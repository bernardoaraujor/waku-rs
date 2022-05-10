use crate::pb::{
    waku_lightpush_pb::{PushRPC, PushRequest, PushResponse},
    waku_message_pb::WakuMessage,
};
use crate::waku_message::MAX_MESSAGE_SIZE;
use crate::waku_relay::{WakuRelayBehaviour, WakuRelayEvent};
use async_std::io::Error;
use async_trait::async_trait;
use futures::prelude::*;
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed, ProtocolName},
    identity::Keypair,
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseConfig,
        RequestResponseEvent, RequestResponseMessage,
    },
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId, Swarm,
};
use protobuf::Message;
use std::{io, iter::once};

struct WakuLightPush {
    swarm: Swarm<WakuLightPushBehaviour>,
}

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct WakuLightPushBehaviour {
    relay: WakuRelayBehaviour,
    req_res: RequestResponse<WakuLightPushCodec>,
}

impl NetworkBehaviourEventProcess<WakuRelayEvent> for WakuLightPushBehaviour {
    fn inject_event(&mut self, event: WakuRelayEvent) {
        todo!()
    }
}

impl NetworkBehaviourEventProcess<RequestResponseEvent<PushRPC, PushRPC>>
    for WakuLightPushBehaviour
{
    fn inject_event(&mut self, event: RequestResponseEvent<PushRPC, PushRPC>) {
        if let RequestResponseEvent::Message {
            peer,
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

            self.req_res.send_response(channel, res_rpc);
        } else if let RequestResponseEvent::Message {
            peer,
            message: RequestResponseMessage::Response { response, .. },
        } = event
        {
            if response.get_response().get_is_success() {
                // todo: treat successful response
            } else {
                // todo: treat unsuccessful response
            }
        }
    }
}

impl WakuLightPushBehaviour {
    fn new(key: Keypair) -> Self {
        Self {
            relay: WakuRelayBehaviour::new(key),
            req_res: RequestResponse::new(
                WakuLightPushCodec,
                once((WakuLightPushProtocol(), ProtocolSupport::Full)),
                RequestResponseConfig::default(),
            ),
        }
    }

    pub fn add_req_res_peer(&mut self, peer_id: PeerId, peer_addr: Multiaddr) {
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

#[derive(Clone)]
struct WakuLightPushCodec;

#[derive(Debug, Clone)]
struct WakuLightPushProtocol();

const LIGHTPUSH_PROTOCOL_ID: &str = "/vac/waku/lightpush/2.0.0-beta1";

impl ProtocolName for WakuLightPushProtocol {
    fn protocol_name(&self) -> &[u8] {
        LIGHTPUSH_PROTOCOL_ID.as_bytes()
    }
}
const MAX_LIGHTPUSH_RPC_SIZE: usize = MAX_MESSAGE_SIZE + 64 * 1024; // We add a 64kB safety buffer for protocol overhead

#[async_trait]
impl RequestResponseCodec for WakuLightPushCodec {
    type Protocol = WakuLightPushProtocol;
    type Request = PushRPC;
    type Response = PushRPC;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let rpc_bytes = read_length_prefixed(io, MAX_LIGHTPUSH_RPC_SIZE).await?;
        let rpc: PushRPC = protobuf::Message::parse_from_bytes(&rpc_bytes).unwrap();
        Ok(rpc)
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let rpc_bytes = read_length_prefixed(io, MAX_LIGHTPUSH_RPC_SIZE).await?;
        let rpc: PushRPC = protobuf::Message::parse_from_bytes(&rpc_bytes).unwrap();
        Ok(rpc)
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let req_bytes = req.write_to_bytes()?;
        write_length_prefixed(io, req_bytes).await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let res_bytes = res.write_to_bytes()?;
        write_length_prefixed(io, res_bytes).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::pb::{
        waku_lightpush_pb::{PushRPC, PushRequest},
        waku_message_pb::WakuMessage,
    };
    use crate::waku_lightpush::{
        WakuLightPush, WakuLightPushBehaviour, WakuLightPushCodec, WakuLightPushProtocol,
    };
    use async_std::io::Error;
    use async_std::prelude::FutureExt;
    use futures::join;
    use futures::StreamExt;
    use libp2p::{identity::Keypair, Multiaddr, PeerId};
    use libp2p::{
        request_response::{
            ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
            RequestResponseMessage,
        },
        swarm::{Swarm, SwarmEvent},
    };
    use std::{iter::once, str::FromStr, thread};

    const ADDR_A: &str = "/ip4/127.0.0.1/tcp/58584";
    const ADDR_B: &str = "/ip4/127.0.0.1/tcp/58601";

    const KEY_A: &str = "23jhTbXRXh1RPMwzN2B7GNXZDiDtrkdm943bVBfAQBJFUosggfSDVQzui7pEbuzBFf6x7C5SLWXvUGB1gPaTLTpwRxDYu";
    const KEY_B: &str = "23jhTfVepCSFrkYE8tATMUuxU3SErCYvrShcit6dQfaonM4QxF82wh4k917LJShErtKNNbaUjmqGVDLDQdVB9n7TGieQ1";

    const PEER_ID_A: &str = "12D3KooWLyTCx9j2FMcsHe81RMoDfhXbdyyFgNGQMdcrnhShTvQh";
    const PEER_ID_B: &str = "12D3KooWKBKXsLwbmVBySEmbKayJzfWp3tPCKrnDCsmNy9prwjvy";

    async fn start(mut swarm: Swarm<WakuLightPushBehaviour>) {
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
        let waku_lp_behaviour_a = WakuLightPushBehaviour::new(key_a.clone());
        let mut swarm_a = Swarm::new(transport_a, waku_lp_behaviour_a, peer_id_a);
        swarm_a.listen_on(address_a).unwrap();

        let transport_b = libp2p::development_transport(key_b.clone()).await?;
        let waku_lp_behaviour_b = WakuLightPushBehaviour::new(key_b.clone());
        let mut swarm_b = Swarm::new(transport_b, waku_lp_behaviour_b, peer_id_b);
        swarm_b.listen_on(address_b.clone()).unwrap();

        swarm_a
            .behaviour_mut()
            .add_req_res_peer(peer_id_b, address_b);

        let msg = WakuMessage::new();

        swarm_a.behaviour_mut().send_request(
            peer_id_b,
            "test_request_id".to_string(),
            "test_topic".to_string(),
            msg,
        );

        let future_a = start(swarm_a);
        let future_b = start(swarm_b);

        join!(future_a, future_b);

        Ok(())
    }
}
