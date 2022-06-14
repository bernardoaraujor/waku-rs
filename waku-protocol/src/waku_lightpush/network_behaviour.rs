use crate::pb::{
    waku_lightpush_pb::{PushRPC, PushRequest, PushResponse},
    waku_message_pb::WakuMessage,
};
use crate::waku_lightpush::codec::{WakuLightPushCodec, WakuLightPushProtocol};
use crate::waku_relay::network_behaviour::{WakuRelayBehaviour, WakuRelayEvent};
use libp2p::{
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
        RequestResponseMessage,
    },
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use log::info;
use std::iter::once;

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub struct WakuLightPushBehaviour {
    relay: WakuRelayBehaviour,
    req_res: RequestResponse<WakuLightPushCodec>,
}

impl NetworkBehaviourEventProcess<WakuRelayEvent> for WakuLightPushBehaviour {
    fn inject_event(&mut self, _: WakuRelayEvent) {}
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
        }
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
}
