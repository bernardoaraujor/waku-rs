use crate::pb::waku_message_pb::WakuMessage;
use crate::pb::waku_store_pb::{ContentFilter, HistoryQuery, HistoryRPC, HistoryResponse, Index};
use crate::waku_store::{
    codec::{WakuStoreCodec, WakuStoreProtocol},
    message_queue::WakuMessageQueue,
};
use libp2p::{
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
        RequestResponseMessage,
    },
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use protobuf::RepeatedField;
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

            // todo: search message queue and sort results

            // todo: create an actual response
            let response = HistoryResponse::new();

            let mut res_rpc = HistoryRPC::new();
            res_rpc.set_request_id(request_id.to_string());
            res_rpc.set_query(query.clone());
            res_rpc.set_response(response);

            self.req_res.send_response(channel, res_rpc);
        } else if let RequestResponseEvent::Message {
            peer: _,
            message: RequestResponseMessage::Response { response, .. },
        } = event
        {
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
        }
    }

    pub fn add_store_peer(&mut self, peer_id: PeerId, peer_addr: Multiaddr) {
        self.req_res.add_address(&peer_id, peer_addr);
    }

    pub fn send_query(
        &mut self,
        peer_id: PeerId,
        request_id: String,
        pubsub_topic: String,
        content_topic: Vec<String>,
        start_time: i64,
        end_time: i64,
    ) {
        let mut query = HistoryQuery::new();
        query.set_pubsub_topic(pubsub_topic);
        let mut content_filters = RepeatedField::new();
        for t in content_topic {
            let mut c = ContentFilter::new();
            c.set_contentTopic(t);
            content_filters.push(c);
        }
        query.set_content_filters(content_filters);
        query.set_start_time(start_time);
        query.set_end_time(end_time);

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
        msg.set_payload(b"".to_vec());
        msg.set_content_topic("test_content_topic".to_string());

        let indexed_message = IndexedWakuMessage::new(
            msg.clone(),
            compute_index(msg),
            "test_pubsub_topic".to_string(),
        );
        swarm_a.behaviour_mut().message_queue.push(indexed_message);

        let mut content_topics = Vec::new();
        content_topics.push("test_content_topic".to_string());
        swarm_a.behaviour_mut().send_query(
            peer_id_b,
            "test_request_id".to_string(),
            "test_pubsub_topic".to_string(),
            content_topics,
            0,
            10,
        );

        let future_a = start(swarm_a);
        let future_b = start(swarm_b);

        join!(future_a, future_b);

        Ok(())
    }
}
