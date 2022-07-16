use crate::{pb::waku_store_pb::HistoryRPC, waku_message::MAX_MESSAGE_SIZE};
use async_trait::async_trait;
use futures::prelude::*;
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed, ProtocolName},
    request_response::RequestResponseCodec,
};
use protobuf::Message;
use std::io;

const MAX_PAGE_SIZE: usize = 100; // Maximum number of waku messages in each page
const STORE_PROTOCOL_ID: &str = "/vac/waku/store/2.0.0-beta4";
const MAX_STORE_RPC_SIZE: usize = MAX_PAGE_SIZE * MAX_MESSAGE_SIZE + 64 * 1024; // We add a 64kB safety buffer for protocol overhead

#[derive(Clone)]
pub struct WakuStoreProtocol();
#[derive(Clone)]
pub struct WakuStoreCodec;

impl ProtocolName for WakuStoreProtocol {
    fn protocol_name(&self) -> &[u8] {
        STORE_PROTOCOL_ID.as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for WakuStoreCodec {
    type Protocol = WakuStoreProtocol;
    type Request = HistoryRPC;
    type Response = HistoryRPC;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let rpc_bytes = read_length_prefixed(io, MAX_STORE_RPC_SIZE).await?;
        let rpc: HistoryRPC = protobuf::Message::parse_from_bytes(&rpc_bytes).unwrap();
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
        let rpc_bytes = read_length_prefixed(io, MAX_STORE_RPC_SIZE).await?;
        let rpc: HistoryRPC = protobuf::Message::parse_from_bytes(&rpc_bytes).unwrap();
        Ok(rpc)
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        query: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let query_bytes = query.write_to_bytes()?;
        write_length_prefixed(io, query_bytes).await?;
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
