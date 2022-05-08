use crate::pb::waku_lightpush_pb::{PushRPC, PushRequest, PushResponse};
use crate::pb::waku_message_pb::WakuMessage;
use crate::waku_message::MAX_MESSAGE_SIZE;
use crate::waku_relay::{WakuRelayBehaviour, WakuRelayEvent};
use async_trait::async_trait;
use futures::prelude::*;
use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed, ProtocolName};
use libp2p::identity;
use libp2p::request_response::{
    ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseConfig,
    RequestResponseEvent, RequestResponseMessage,
};
use libp2p::{swarm::NetworkBehaviourEventProcess, NetworkBehaviour};
use protobuf::Message;
use std::io;
use std::iter::once;

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
            // todo: relay
            let res_rpc = request.clone();
            // todo: res_rpc.set_response();
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
    fn new(key: identity::Keypair) -> Self {
        Self {
            relay: WakuRelayBehaviour::new(key),
            req_res: RequestResponse::new(
                WakuLightPushCodec,
                once((WakuLightPushProtocol(), ProtocolSupport::Full)),
                RequestResponseConfig::default(),
            ),
        }
    }

    fn new_request_rpc(request_id: String, pubsub_topic: String, msg: WakuMessage) -> PushRPC {
        let mut req = PushRequest::new();
        req.set_pubsub_topic(pubsub_topic);
        req.set_message(msg);

        let mut req_rpc = PushRPC::new();
        req_rpc.set_request_id(request_id);
        req_rpc.set_query(req);
        req_rpc
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
    use crate::pb::waku_lightpush_pb::{PushRPC, PushRequest};
    use crate::pb::waku_message_pb::WakuMessage;
    use crate::waku_lightpush::WakuLightPushBehaviour;
    use crate::waku_lightpush::{
        // WakuLightPushBehaviour,
        WakuLightPushCodec,
        WakuLightPushProtocol,
    };
    use async_std::io::Error;
    use async_std::prelude::FutureExt;
    use futures::join;
    use futures::StreamExt;
    use libp2p::request_response::{
        ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
        RequestResponseMessage,
    };
    use libp2p::swarm::{Swarm, SwarmEvent};
    use libp2p::{identity, Multiaddr, PeerId};
    use std::iter::once;
    use std::str::FromStr;
    use std::thread;

    async fn setup_node(
        key: String,
        address: String,
        bootstrap_nodes: [[&str; 2]; 2],
    ) -> Result<Swarm<WakuLightPushBehaviour>, Error> {
        let decoded_key = bs58::decode(&key).into_vec().unwrap();
        let local_key = identity::Keypair::from_protobuf_encoding(&decoded_key).unwrap();

        let local_address = Multiaddr::from_str(&address).unwrap();

        // Setting up the request/response protocol.
        let protocols = once((WakuLightPushProtocol(), ProtocolSupport::Full));
        let cfg = RequestResponseConfig::default();
        let req_proto = WakuLightPushBehaviour::new(local_key.clone());

        // Setting up the transport and swarm.
        let local_peer_id = PeerId::from(local_key.public());
        let transport = libp2p::development_transport(local_key).await?;
        let mut swarm = Swarm::new(transport, req_proto, local_peer_id);
        swarm.listen_on(local_address).unwrap();

        // We want to connect to all bootstrap nodes.
        for info in bootstrap_nodes.iter() {
            let addr = Multiaddr::from_str(info[0]).unwrap();
            let peer_id = PeerId::from_str(info[1]).unwrap();

            if peer_id == local_peer_id {
                continue;
            }

            let msg = WakuMessage::new();
            let rpc = WakuLightPushBehaviour::new_request_rpc(
                "test_request_id".to_string(),
                "test_topic".to_string(),
                msg,
            );

            swarm
                .behaviour_mut()
                .req_res
                .add_address(&peer_id, addr.clone());
            swarm.behaviour_mut().req_res.send_request(&peer_id, rpc);
        }

        Ok(swarm)
    }

    async fn start_loop(swarm: &mut Swarm<WakuLightPushBehaviour>) {
        println!("");
        loop {
            swarm.select_next_some().await;
        }
    }

    const BOOTSTRAP_PEERS: [[&str; 2]; 2] = [
        [
            "/ip4/127.0.0.1/tcp/58584",
            "12D3KooWLyTCx9j2FMcsHe81RMoDfhXbdyyFgNGQMdcrnhShTvQh",
        ],
        [
            "/ip4/127.0.0.1/tcp/58601",
            "12D3KooWKBKXsLwbmVBySEmbKayJzfWp3tPCKrnDCsmNy9prwjvy",
        ],
    ];

    const ADDR_A: &str = "/ip4/127.0.0.1/tcp/58584";
    const ADDR_B: &str = "/ip4/127.0.0.1/tcp/58601";

    const KEY_A: &str = "23jhTbXRXh1RPMwzN2B7GNXZDiDtrkdm943bVBfAQBJFUosggfSDVQzui7pEbuzBFf6x7C5SLWXvUGB1gPaTLTpwRxDYu";
    const KEY_B: &str = "23jhTfVepCSFrkYE8tATMUuxU3SErCYvrShcit6dQfaonM4QxF82wh4k917LJShErtKNNbaUjmqGVDLDQdVB9n7TGieQ1";

    #[async_std::test]
    async fn my_test() -> std::io::Result<()> {
        let mut swarm_a =
            setup_node(KEY_A.to_string(), ADDR_A.to_string(), BOOTSTRAP_PEERS).await?;

        let mut swarm_b =
            setup_node(KEY_B.to_string(), ADDR_B.to_string(), BOOTSTRAP_PEERS).await?;

        let future_a = start_loop(&mut swarm_a);
        let future_b = start_loop(&mut swarm_b);

        join!(future_a, future_b);

        Ok(())
    }
}
