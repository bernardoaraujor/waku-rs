use crate::{
    pb::{
        waku_lightpush_pb::{PushRPC, PushRequest, PushResponse},
        waku_message_pb::WakuMessage,
    },
    waku_lightpush::codec::{WakuLightPushCodec, WakuLightPushProtocol},
    waku_relay::network_behaviour::{WakuRelayBehaviour, WakuRelayEvent},
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
use std::{
    iter::once,
    task::{Context, Poll},
};

#[derive(NetworkBehaviour)]
#[behaviour(
    event_process = true,
    out_event = "WakuLightPushEvent",
    poll_method = "poll"
)]
pub struct WakuLightPushBehaviour {
    relay: WakuRelayBehaviour,
    req_res: RequestResponse<WakuLightPushCodec>,
    #[behaviour(ignore)]
    events: Vec<WakuLightPushEvent>,
}

#[derive(Debug)]
pub enum WakuLightPushEvent {
    WakuRelayBehaviour(WakuRelayEvent),
    RequestResponseBehaviour(RequestResponseEvent<PushRPC, PushRPC>),
}

impl From<WakuRelayEvent> for WakuLightPushEvent {
    fn from(event: WakuRelayEvent) -> Self {
        Self::WakuRelayBehaviour(event)
    }
}

impl From<RequestResponseEvent<PushRPC, PushRPC>> for WakuLightPushEvent {
    fn from(event: RequestResponseEvent<PushRPC, PushRPC>) -> Self {
        Self::RequestResponseBehaviour(event)
    }
}

impl NetworkBehaviourEventProcess<WakuRelayEvent> for WakuLightPushBehaviour {
    fn inject_event(&mut self, event: WakuRelayEvent) {
        if let WakuRelayEvent::GossipSub(GossipsubEvent::Message {
            propagation_source,
            message_id,
            message,
        }) = event
        {
            self.events.push(WakuLightPushEvent::WakuRelayBehaviour(
                WakuRelayEvent::GossipSub(GossipsubEvent::Message {
                    propagation_source,
                    message_id,
                    message,
                }),
            ));
        }
    }
}

impl NetworkBehaviourEventProcess<RequestResponseEvent<PushRPC, PushRPC>>
    for WakuLightPushBehaviour
{
    fn inject_event(&mut self, event: RequestResponseEvent<PushRPC, PushRPC>) {
        if let RequestResponseEvent::Message {
            peer: _,
            message:
                RequestResponseMessage::Request {
                    channel, request, ..
                },
        } = event
        {
            // when WakuLightPushBehaviour receives a Request,
            // it forwards the WakuMessage to WakuRelayBehaviour

            let req = request.get_query().clone();
            let req_id = request.get_request_id();
            let req_topic = req.get_pubsub_topic();
            let req_msg = req.get_message().clone();
            let mut res = PushResponse::new();

            info!("WakuLightPush: pushing request: {:?}", req);

            match self.relay.publish(req_topic, req_msg) {
                Ok(_) => {
                    res.set_is_success(true);
                    // todo: res.set_info() ?
                }
                Err(e) => {
                    res.set_is_success(false);
                    res.set_info(e.to_string());
                }
            }

            let mut res_rpc = PushRPC::new();
            res_rpc.set_query(req);
            res_rpc.set_response(res);
            res_rpc.set_request_id(req_id.to_string());

            self.req_res.send_response(channel, res_rpc).unwrap();
        } else if let RequestResponseEvent::Message {
            peer: _,
            message: RequestResponseMessage::Response { response, .. },
        } = event
        {
            if response.get_response().get_is_success() {
                info!(
                    "WakuLightPush: successful response: {:?}",
                    response.get_response()
                );
            } else {
                info!(
                    "WakuLightPush: unsuccessful response: {:?}",
                    response.get_response()
                );
            }
        }
    }
}

impl WakuLightPushBehaviour {
    pub fn new() -> Self {
        Self {
            relay: WakuRelayBehaviour::new(),
            req_res: RequestResponse::new(
                WakuLightPushCodec,
                once((WakuLightPushProtocol(), ProtocolSupport::Full)),
                RequestResponseConfig::default(),
            ),
            events: Vec::new(),
        }
    }

    pub fn publish(&mut self, topic: &str, msg: WakuMessage) -> Result<MessageId, PublishError> {
        self.relay.publish(topic, msg)
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<bool, SubscriptionError> {
        self.relay.subscribe(topic)
    }

    pub fn unsubscribe(&mut self, topic: &str) -> Result<bool, PublishError> {
        self.relay.unsubscribe(topic)
    }

    pub fn add_lightpush_peer(&mut self, peer_id: PeerId, peer_addr: Multiaddr) {
        self.req_res.add_address(&peer_id, peer_addr);
    }

    pub fn send_request(
        &mut self,
        peer_id: PeerId,
        request_id: String,
        pubsub_topic: String,
        msg: WakuMessage,
    ) {
        let mut req = PushRequest::new();
        req.set_pubsub_topic(pubsub_topic);
        req.set_message(msg);

        let mut req_rpc = PushRPC::new();
        req_rpc.set_request_id(request_id);
        req_rpc.set_query(req);
        self.req_res.send_request(&peer_id, req_rpc);
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
