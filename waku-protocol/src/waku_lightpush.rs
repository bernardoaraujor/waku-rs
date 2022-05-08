use crate::pb::waku_lightpush_pb::{PushRPC, PushRequest, PushResponse};
use crate::pb::waku_message_pb::WakuMessage;
use crate::waku_message::MAX_MESSAGE_SIZE;
use async_trait::async_trait;
use futures::prelude::*;
use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed, ProtocolName};
use libp2p::request_response::{
    RequestResponse, RequestResponseCodec, RequestResponseEvent, RequestResponseMessage,
};
use libp2p::NetworkBehaviour;
use protobuf::Message;
use std::io;

const MAX_LIGHTPUSH_RPC_SIZE: usize = MAX_MESSAGE_SIZE + 64 * 1024; // We add a 64kB safety buffer for protocol overhead
const LIGHTPUSH_PROTOCOL_ID: &str = "/vac/waku/lightpush/2.0.0-beta1";

#[derive(Clone)]
pub struct WakuLightPushCodec;

#[derive(Debug, Clone)]
pub struct WakuLightPushProtocol();

impl WakuLightPushProtocol {
    pub fn new_request_rpc(request_id: String, pubsub_topic: String, msg: WakuMessage) -> PushRPC {
        let mut req = PushRequest::new();
        req.set_pubsub_topic(pubsub_topic);
        req.set_message(msg);

        let mut req_rpc = PushRPC::new();
        req_rpc.set_request_id(request_id);
        req_rpc.set_query(req);
        req_rpc
    }

    pub fn handle_request_response_event(
        req_res: &mut RequestResponse<WakuLightPushCodec>,
        event: RequestResponseEvent<PushRPC, PushRPC>,
    ) {
        match event {
            RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Request {
                        channel, request, ..
                    },
            } => {
                // log::info!("WakuLightPushProtocol: Received request from {:?}", peer);
                // todo: relay
                let res_rpc = request.clone();
                // todo: res_rpc.set_response();
                req_res.send_response(channel, res_rpc);
            }
            RequestResponseEvent::Message {
                peer,
                message: RequestResponseMessage::Response { response, .. },
            } => {
                // log::info!("WakuLightPushProtocol: Received response from {:?}", peer);
                // log::info!("WakuLightPushProtocol: Response info: {:?}", response.get_response().get_info());
                if response.get_response().get_is_success() {
                    // todo: treat successful response
                } else {
                    // todo: treat unsuccessful response
                }
            }
            RequestResponseEvent::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                // log::info!(
                //     "WakuLightPushProtocol: Failed to handle request ({:?}) from {:?}: {:?}",
                //     request_id,
                //     peer,
                //     error
                // );
            }
            RequestResponseEvent::OutboundFailure {
                peer,
                request_id,
                error,
            } => {
                // log::info!(
                //     "WakuLightPushProtocol: Failed to send request ({:?}) to {:?}: {:?}",
                //     request_id,
                //     peer,
                //     error
                // );
            }
            RequestResponseEvent::ResponseSent { peer, request_id } => {
                // log::info!("WakuLightPushProtocol: Sent response to {:?} ({:?})", peer, request_id);
            }
        }
    }
}

impl ProtocolName for WakuLightPushProtocol {
    fn protocol_name(&self) -> &[u8] {
        LIGHTPUSH_PROTOCOL_ID.as_bytes()
    }
}

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

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WakuLightPushEvent")]
struct WakuLightPush {
    request_response: RequestResponse<WakuLightPushCodec>,
}

#[derive(Debug)]
pub enum WakuLightPushEvent {
    RequestResponse(RequestResponseEvent<PushRPC, PushRPC>),
}

impl From<RequestResponseEvent<PushRPC, PushRPC>> for WakuLightPushEvent {
    fn from(event: RequestResponseEvent<PushRPC, PushRPC>) -> Self {
        WakuLightPushEvent::RequestResponse(event)
    }
}

#[cfg(test)]
mod tests {
    use crate::pb::waku_lightpush_pb::{PushRPC, PushRequest};
    use crate::pb::waku_message_pb::WakuMessage;
    use crate::waku_lightpush::{WakuLightPushCodec, WakuLightPushProtocol};
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
        key: Option<String>,
        address: Option<String>,
        default_address: &str,
        bootstrap_nodes: [[&str; 2]; 2],
    ) -> Result<Swarm<RequestResponse<WakuLightPushCodec>>, Error> {
        // Taking the keypair from the command line or generating a new one.
        let local_key = if let Some(key) = key {
            let decoded_key = bs58::decode(&key).into_vec().unwrap();
            identity::Keypair::from_protobuf_encoding(&decoded_key).unwrap()
        } else {
            identity::Keypair::generate_ed25519()
        };

        // Taking the address from the command line arguments or the default one.
        let local_address = if let Some(addr) = address {
            Multiaddr::from_str(&addr).unwrap()
        } else {
            Multiaddr::from_str(&default_address).unwrap()
        };

        // Setting up the request/response protocol.
        let protocols = once((WakuLightPushProtocol(), ProtocolSupport::Full));
        let cfg = RequestResponseConfig::default();
        let req_proto = RequestResponse::new(WakuLightPushCodec, protocols.clone(), cfg.clone());

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
            let rpc = WakuLightPushProtocol::new_request_rpc(
                "test_request_id".to_string(),
                "test_topic".to_string(),
                msg,
            );

            swarm.behaviour_mut().add_address(&peer_id, addr.clone());
            swarm.behaviour_mut().send_request(&peer_id, rpc);
        }

        Ok(swarm)
    }

    async fn start_loop(swarm: &mut Swarm<RequestResponse<WakuLightPushCodec>>) {
        println!("");
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {:?}", address)
                }
                SwarmEvent::Behaviour(event) => {
                    WakuLightPushProtocol::handle_request_response_event(
                        swarm.behaviour_mut(),
                        event,
                    )
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("Connection established with {:?}", peer_id);
                }
                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    println!("Connection closed with {:?} ({:?})", peer_id, cause);
                }
                SwarmEvent::IncomingConnection { send_back_addr, .. } => {
                    println!("Incoming connection to {:?}", send_back_addr);
                }
                SwarmEvent::IncomingConnectionError {
                    send_back_addr,
                    error,
                    ..
                } => {
                    println!(
                        "Incoming connection error to {:?} ({:?})",
                        send_back_addr, error
                    );
                }
                SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                    println!("Outgoing connection error with {:?} ({:?})", peer_id, error);
                }
                SwarmEvent::BannedPeer { peer_id, endpoint } => {
                    println!("Banned peer {:?} ({:?})", peer_id, endpoint);
                }
                SwarmEvent::ExpiredListenAddr {
                    listener_id,
                    address,
                } => {
                    println!("Expired listen addr {:?} ({:?})", listener_id, address);
                }
                SwarmEvent::ListenerClosed {
                    listener_id,
                    addresses,
                    reason,
                } => {
                    println!(
                        "Listener closed {:?} ({:?}) ({:?})",
                        listener_id, addresses, reason
                    );
                }
                SwarmEvent::ListenerError { listener_id, error } => {
                    println!("Listener error {:?} ({:?})", listener_id, error);
                }
                SwarmEvent::Dialing(peer_id) => {
                    println!("Dialing {:?}", peer_id);
                }
            }
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
    const DEFAULT_ADDRESS: &str = "/ip4/0.0.0.0/tcp/0";

    const KEY_A: &str = "23jhTbXRXh1RPMwzN2B7GNXZDiDtrkdm943bVBfAQBJFUosggfSDVQzui7pEbuzBFf6x7C5SLWXvUGB1gPaTLTpwRxDYu";
    const KEY_B: &str = "23jhTfVepCSFrkYE8tATMUuxU3SErCYvrShcit6dQfaonM4QxF82wh4k917LJShErtKNNbaUjmqGVDLDQdVB9n7TGieQ1";

    #[async_std::test]
    async fn my_test() -> std::io::Result<()> {
        let mut swarm_a = setup_node(
            Some(KEY_A.to_string()),
            Some(ADDR_A.to_string()),
            DEFAULT_ADDRESS,
            BOOTSTRAP_PEERS,
        )
        .await?;

        let mut swarm_b = setup_node(
            Some(KEY_B.to_string()),
            Some(ADDR_B.to_string()),
            DEFAULT_ADDRESS,
            BOOTSTRAP_PEERS,
        )
        .await?;

        let future_a = start_loop(&mut swarm_a);
        let future_b = start_loop(&mut swarm_b);

        join!(future_a, future_b);

        Ok(())
    }
}
