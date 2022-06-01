use crate::pb::waku_message_pb::WakuMessage;
use crate::pb::waku_store_pb::{
    ContentFilter, HistoryQuery, HistoryRPC, HistoryResponse, HistoryResponse_Error, Index,
    PagingInfo, PagingInfo_Direction,
};
use crate::waku_relay::network_behaviour::{WakuRelayBehaviour, WakuRelayEvent};
use crate::waku_store::message_queue::IndexedWakuMessage;
use crate::waku_store::{
    codec::{WakuStoreCodec, WakuStoreProtocol},
    message_queue::WakuMessageQueue,
};
use libp2p::{
    gossipsub::GossipsubEvent,
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
        RequestResponseMessage,
    },
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use log::info;
use protobuf::{Message, RepeatedField};
use sha2::{Digest, Sha256};
use std::iter::once;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_PUBSUB_TOPIC: &str = "/waku/2/default-waku/proto";

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct WakuStoreBehaviour {
    #[behaviour(ignore)]
    message_queue: WakuMessageQueue,
    req_res: RequestResponse<WakuStoreCodec>,
    relay: WakuRelayBehaviour, // todo: Either filter
}

impl NetworkBehaviourEventProcess<WakuRelayEvent> for WakuStoreBehaviour {
    fn inject_event(&mut self, event: WakuRelayEvent) {
        if let WakuRelayEvent::GossipSub(GossipsubEvent::Message {
            propagation_source: peer_id,
            message_id: id,
            message,
        }) = event
        {
            let topic = message.topic.to_string();
            let mut waku_message = WakuMessage::new();
            waku_message.merge_from_bytes(&message.data).unwrap();
            let indexed_message =
                IndexedWakuMessage::new(waku_message.clone(), compute_index(waku_message), topic);
            info!(
                "WakuStore: queueing message received via WakuRelay: {:?}",
                indexed_message
            );
            self.message_queue.push(indexed_message).unwrap();
        }
    }
}

impl NetworkBehaviourEventProcess<RequestResponseEvent<HistoryRPC, HistoryRPC>>
    for WakuStoreBehaviour
{
    fn inject_event(&mut self, event: RequestResponseEvent<HistoryRPC, HistoryRPC>) {
        if let RequestResponseEvent::Message {
            peer: _,
            message:
                RequestResponseMessage::Request {
                    channel, request, ..
                },
        } = event
        {
            let request_id = request.get_request_id();
            let query = request.get_query();
            info!(
                "WakuStore: received request. Request ID: {}, Query: {:?}",
                request_id, query
            );

            let query_pubsub_topic = query.get_pubsub_topic();
            let query_content_filters = query.get_content_filters();
            let query_paging_info = query.get_paging_info();
            let query_page_size = query_paging_info.get_page_size();
            let query_direction = query_paging_info.get_direction();
            let query_cursor = query.get_paging_info().get_cursor();
            let query_digest = query_cursor.get_digest().to_vec();

            let mut response = HistoryResponse::new();

            if !self.message_queue.has_queued_digest(query_digest) {
                info!("WakuStore: query not found");
                response.set_error(HistoryResponse_Error::NONE);
            } else {
                let mut res_messages = Vec::new();
                let mut j = 0;
                for (i, indexed_message) in self.message_queue.iter().enumerate() {
                    if indexed_message.index() == query_cursor {
                        j = i;
                    }
                }

                let mut page_count = 0;
                let dir: i32 = match query_direction {
                    PagingInfo_Direction::FORWARD => 1,
                    PagingInfo_Direction::BACKWARD => -1,
                };
                for k in 0..self.message_queue.len() {
                    let i = (j as i32 + k as i32 * dir) % self.message_queue.len() as i32;
                    if let Some(indexed_message) = self.message_queue.get(i as usize) {
                        let mut cf = ContentFilter::new();
                        cf.set_contentTopic(indexed_message.content_topic().to_string());
                        if indexed_message.pubsub_topic() == query_pubsub_topic
                            && query_content_filters.contains(&cf)
                        {
                            res_messages.push(indexed_message.message().clone());
                            page_count = page_count + 1;

                            if page_count == query_page_size {
                                break;
                            }
                        }
                    };
                }

                response.set_messages(RepeatedField::from_vec(res_messages));
            }
            response.set_paging_info(query_paging_info.clone());

            let mut res_rpc = HistoryRPC::new();
            res_rpc.set_request_id(request_id.to_string());
            res_rpc.set_query(query.clone());
            res_rpc.set_response(response.clone());

            info!("WakuStore: sending query response: {:?}", response);
            self.req_res.send_response(channel, res_rpc).unwrap();
        } else if let RequestResponseEvent::Message {
            peer: _,
            message: RequestResponseMessage::Response { response, .. },
        } = event
        {
            info!("WakuStore: received response. {:?}", response);
            //todo: parse response
        }
    }
}

impl WakuStoreBehaviour {
    fn new(max_messages: usize) -> Self {
        Self {
            message_queue: WakuMessageQueue::new(max_messages),
            req_res: RequestResponse::new(
                WakuStoreCodec,
                once((WakuStoreProtocol(), ProtocolSupport::Full)),
                RequestResponseConfig::default(),
            ),
            relay: WakuRelayBehaviour::new(),
        }
    }

    pub fn add_store_peer(&mut self, peer_id: PeerId, peer_addr: Multiaddr) {
        self.req_res.add_address(&peer_id, peer_addr);
    }

    pub fn send_query(
        &mut self,
        peer_id: PeerId,
        request_id: String, // todo: should this be an input parameter?
        cursor: Index,
        page_size: u64,
        direction: bool,
        pubsub_topic: String,
        content_topic: Vec<String>,
    ) {
        let mut query = HistoryQuery::new();
        query.set_pubsub_topic(pubsub_topic);

        let mut paging_info = PagingInfo::new();
        paging_info.set_page_size(page_size);
        paging_info.set_cursor(cursor);
        match direction {
            true => paging_info.set_direction(PagingInfo_Direction::FORWARD),
            false => paging_info.set_direction(PagingInfo_Direction::BACKWARD),
        }
        query.set_paging_info(paging_info);

        let mut content_filters = RepeatedField::new();
        for t in content_topic {
            let mut c = ContentFilter::new();
            c.set_contentTopic(t);
            content_filters.push(c);
        }
        query.set_content_filters(content_filters);
        info!("WakuStore: sending query: {:?}", query);

        let mut query_rpc = HistoryRPC::new();
        query_rpc.set_request_id(request_id);
        query_rpc.set_query(query);

        self.req_res.send_request(&peer_id, query_rpc);
    }
}

pub type WakuMessageDigest = Vec<u8>;

// Takes a WakuMessage and returns its Index.
pub fn compute_index(msg: WakuMessage) -> Index {
    let mut hasher = Sha256::new();

    hasher.update(msg.payload);
    hasher.update(msg.content_topic.as_bytes());

    let digest: WakuMessageDigest = hasher.finalize().as_slice().to_vec();

    let mut index = Index::new();
    index.set_digest(digest);
    index.set_receiver_time(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos() as i64,
    );
    index.set_sender_time(msg.timestamp);
    index.set_pubsub_topic(msg.content_topic);

    index
}

#[cfg(test)]
mod tests {
    use crate::pb::waku_message_pb::WakuMessage;
    use crate::pb::waku_store_pb::Index;
    use crate::waku_store::message_queue::IndexedWakuMessage;
    use crate::waku_store::network_behaviour::WakuStoreBehaviour;
    use crate::waku_store::network_behaviour::{compute_index, DEFAULT_PUBSUB_TOPIC};
    use futures::join;
    use futures::StreamExt;
    use libp2p::{identity::Keypair, swarm::Swarm, Multiaddr, PeerId};
    use sha2::{Digest, Sha256};
    use std::str::FromStr;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_compute_index() {
        let test_payload = "test_payload".as_bytes().to_vec();
        let test_topic = DEFAULT_PUBSUB_TOPIC.to_string();
        let test_version = 1;
        let test_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos() as i64;

        let mut msg = WakuMessage::new();
        msg.set_payload(test_payload.clone());
        msg.set_content_topic(test_topic.clone());
        msg.set_version(test_version.clone());
        msg.set_timestamp(test_timestamp.clone());

        let index = compute_index(msg);

        let mut hasher = Sha256::new();

        hasher.update(test_payload);
        hasher.update(test_topic.clone().as_bytes());

        let digest = hasher.finalize().as_slice().to_vec();

        assert_eq!(index.digest, digest);
        assert_eq!(index.pubsub_topic, test_topic);
        assert_eq!(index.sender_time, test_timestamp);
    }

    const ADDR_A: &str = "/ip4/127.0.0.1/tcp/58584";
    const ADDR_B: &str = "/ip4/127.0.0.1/tcp/58601";

    const KEY_A: &str = "23jhTbXRXh1RPMwzN2B7GNXZDiDtrkdm943bVBfAQBJFUosggfSDVQzui7pEbuzBFf6x7C5SLWXvUGB1gPaTLTpwRxDYu";
    const KEY_B: &str = "23jhTfVepCSFrkYE8tATMUuxU3SErCYvrShcit6dQfaonM4QxF82wh4k917LJShErtKNNbaUjmqGVDLDQdVB9n7TGieQ1";

    const PEER_ID_A: &str = "12D3KooWLyTCx9j2FMcsHe81RMoDfhXbdyyFgNGQMdcrnhShTvQh";
    const PEER_ID_B: &str = "12D3KooWKBKXsLwbmVBySEmbKayJzfWp3tPCKrnDCsmNy9prwjvy";

    async fn start(mut swarm: Swarm<WakuStoreBehaviour>) {
        loop {
            swarm.select_next_some().await;
        }
    }

    #[async_std::test]
    async fn my_test() -> std::io::Result<()> {
        env_logger::init_from_env(
            env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
        );
        let decoded_key_a = bs58::decode(&KEY_A.to_string()).into_vec().unwrap();
        let key_a = Keypair::from_protobuf_encoding(&decoded_key_a).unwrap();
        let address_a = Multiaddr::from_str(&ADDR_A.to_string()).unwrap();
        let peer_id_a = PeerId::from_str(PEER_ID_A).unwrap();

        let decoded_key_b = bs58::decode(&KEY_B.to_string()).into_vec().unwrap();
        let key_b = Keypair::from_protobuf_encoding(&decoded_key_b).unwrap();
        let address_b = Multiaddr::from_str(&ADDR_B.to_string()).unwrap();
        let peer_id_b = PeerId::from_str(PEER_ID_B).unwrap();

        let transport_a = libp2p::development_transport(key_a.clone()).await?;
        let waku_lp_behaviour_a = WakuStoreBehaviour::new(10);
        let mut swarm_a = Swarm::new(transport_a, waku_lp_behaviour_a, peer_id_a);
        swarm_a.listen_on(address_a).unwrap();

        let transport_b = libp2p::development_transport(key_b.clone()).await?;
        let waku_lp_behaviour_b = WakuStoreBehaviour::new(10);
        let mut swarm_b = Swarm::new(transport_b, waku_lp_behaviour_b, peer_id_b);
        swarm_b.listen_on(address_b.clone()).unwrap();

        swarm_a.behaviour_mut().add_store_peer(peer_id_b, address_b);

        let mut msg = WakuMessage::new();
        msg.set_payload(b"test_payload".to_vec());
        msg.set_content_topic("test_content_topic".to_string());

        let indexed_message = IndexedWakuMessage::new(
            msg.clone(),
            compute_index(msg.clone()),
            "test_pubsub_topic".to_string(),
        );
        swarm_b
            .behaviour_mut()
            .message_queue
            .push(indexed_message)
            .unwrap();

        let cursor = compute_index(msg);

        let mut content_topics = Vec::new();
        content_topics.push("test_content_topic".to_string());
        swarm_a.behaviour_mut().send_query(
            peer_id_b,
            "test_request_id".to_string(),
            cursor,
            1,
            true,
            "test_pubsub_topic".to_string(),
            content_topics,
        );

        let future_a = start(swarm_a);
        let future_b = start(swarm_b);

        join!(future_a, future_b);

        Ok(())
    }
}
