use crate::{
    pb::{
        waku_message_pb::WakuMessage,
        waku_store_pb::{
            ContentFilter, HistoryQuery, HistoryRPC, HistoryResponse, HistoryResponse_Error, Index,
            PagingInfo, PagingInfo_Direction,
        },
    },
    waku_relay::network_behaviour::{WakuRelayBehaviour, WakuRelayEvent},
    waku_store::{
        codec::{WakuStoreCodec, WakuStoreProtocol},
        message_queue::{IndexedWakuMessage, WakuMessageQueue},
    },
};
use libp2p::{
    gossipsub::{
        error::{PublishError, SubscriptionError},
        GossipsubEvent, MessageId,
    },
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
        RequestResponseMessage,
    },
    swarm::{
        NetworkBehaviour, NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters,
    },
    Multiaddr, NetworkBehaviour, PeerId,
};
use log::info;
use protobuf::{Message, RepeatedField};
use sha2::{Digest, Sha256};
use std::{
    iter::once,
    task::{Context, Poll},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(NetworkBehaviour)]
#[behaviour(
    event_process = true,
    out_event = "WakuStoreEvent",
    poll_method = "poll"
)]
pub struct WakuStoreBehaviour {
    req_res: RequestResponse<WakuStoreCodec>,
    relay: WakuRelayBehaviour, // todo: Either filter
    #[behaviour(ignore)]
    message_queue: WakuMessageQueue,
    #[behaviour(ignore)]
    events: Vec<WakuStoreEvent>,
}

#[derive(Debug)]
pub enum WakuStoreEvent {
    WakuRelayBehaviour(WakuRelayEvent),
    RequestResponseBehaviour(RequestResponseEvent<HistoryRPC, HistoryRPC>),
}

impl From<WakuRelayEvent> for WakuStoreEvent {
    fn from(event: WakuRelayEvent) -> Self {
        Self::WakuRelayBehaviour(event)
    }
}

impl From<RequestResponseEvent<HistoryRPC, HistoryRPC>> for WakuStoreEvent {
    fn from(event: RequestResponseEvent<HistoryRPC, HistoryRPC>) -> Self {
        Self::RequestResponseBehaviour(event)
    }
}

impl NetworkBehaviourEventProcess<WakuRelayEvent> for WakuStoreBehaviour {
    fn inject_event(&mut self, event: WakuRelayEvent) {
        if let WakuRelayEvent::GossipSub(GossipsubEvent::Message {
            propagation_source,
            message_id,
            message,
        }) = event
        {
            let topic = message.topic.to_string();
            let mut waku_message = WakuMessage::new();
            waku_message.merge_from_bytes(&message.data).unwrap();
            let indexed_message = IndexedWakuMessage::new(
                waku_message.clone(),
                compute_index(waku_message.clone()),
                topic.clone(),
            );
            info!(
                "WakuStore: message received via WakuRelay: {:?}",
                indexed_message
            );
            match self.message_queue.push(indexed_message) {
                Ok(_) => info!("WakuStore: successfully queued message"),
                Err(e) => info!("WakuStore: not queueing message: {:?}", e),
            };
            self.events.push(WakuStoreEvent::WakuRelayBehaviour(
                WakuRelayEvent::GossipSub(GossipsubEvent::Message {
                    propagation_source,
                    message_id,
                    message,
                }),
            ));
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
                response.set_error(HistoryResponse_Error::INVALID_CURSOR);
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
            match response.get_response().get_error() {
                HistoryResponse_Error::INVALID_CURSOR => info!("WakuStore: failed query."),
                HistoryResponse_Error::NONE => {
                    info!("WakuStore: received response. {:?}", response)
                }
            }
        }
    }
}

impl WakuStoreBehaviour {
    pub fn new(max_messages: usize) -> Self {
        Self {
            message_queue: WakuMessageQueue::new(max_messages),
            events: Vec::new(),
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

    pub fn add_relay_peer(&mut self, peer_id: &PeerId) {
        self.relay.add_peer(peer_id);
    }

    pub fn publish(&mut self, topic: &str, msg: WakuMessage) -> Result<MessageId, PublishError> {
        self.relay.publish(topic, msg)
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<bool, SubscriptionError> {
        self.relay.subscribe(topic)
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

    fn poll(
        &mut self,
        _: &mut Context,
        _: &mut impl PollParameters,
    ) -> Poll<
        NetworkBehaviourAction<
            <Self as NetworkBehaviour>::OutEvent,
            <Self as NetworkBehaviour>::ConnectionHandler,
        >,
    > {
        if !self.events.is_empty() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(self.events.remove(0)));
        }
        Poll::Pending
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
