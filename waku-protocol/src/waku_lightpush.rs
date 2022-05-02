use crate::pb::waku_lightpush_pb::PushRPC;
use crate::waku_message::MAX_MESSAGE_SIZE;
use async_trait::async_trait;
use futures::prelude::*;
use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed, ProtocolName};
use libp2p::request_response::{RequestResponse, RequestResponseCodec, RequestResponseEvent};
use libp2p::NetworkBehaviour;
use protobuf::Message;
use std::io;

const MAX_LIGHTPUSH_RPC_SIZE: usize = MAX_MESSAGE_SIZE + 64 * 1024; // We add a 64kB safety buffer for protocol overhead
const LIGHTPUSH_PROTOCOL_ID: &str = "/vac/waku/lightpush/2.0.0-beta1";

#[derive(Clone)]
pub struct WakuLightPushProtocol();
#[derive(Clone)]
pub struct WakuLightPushCodec();

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
