use crate::pb::waku_message_pb::WakuMessage;
use crate::pb::waku_store_pb::{HistoryQuery, HistoryResponse, Index};
use async_trait::async_trait;
use futures::prelude::*;
use libp2p::core::upgrade::ProtocolName;
use libp2p::request_response::{RequestResponse, RequestResponseCodec, RequestResponseEvent};
use libp2p::NetworkBehaviour;
use sha2::{Digest, Sha256};
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

const STORE_PROTOCOL_ID: &str = "/vac/waku/store/2.0.0-beta4";
const DEFAULT_PUBSUB_TOPIC: &str = "/waku/2/default-waku/proto";

#[derive(Clone)]
pub struct WakuStoreProtocol();
#[derive(Clone)]
pub struct WakuStoreCodec();

impl ProtocolName for WakuStoreProtocol {
    fn protocol_name(&self) -> &[u8] {
        STORE_PROTOCOL_ID.as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for WakuStoreCodec {
    type Protocol = WakuStoreProtocol;
    type Request = HistoryQuery;
    type Response = HistoryResponse;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        // todo
        let hq = HistoryQuery::new();
        Ok(hq)
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        // todo
        let hr = HistoryResponse::new();
        Ok(hr)
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        _: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // todo
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
        // todo
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WakuStoreEvent")]
struct WakuStore {
    request_response: RequestResponse<WakuStoreCodec>,
}

#[derive(Debug)]
pub enum WakuStoreEvent {
    RequestResponse(RequestResponseEvent<HistoryQuery, HistoryResponse>),
}

impl From<RequestResponseEvent<HistoryQuery, HistoryResponse>> for WakuStoreEvent {
    fn from(event: RequestResponseEvent<HistoryQuery, HistoryResponse>) -> Self {
        WakuStoreEvent::RequestResponse(event)
    }
}

// Takes a WakuMessage and returns its Index.
fn compute_index(msg: WakuMessage) -> Index {
    let mut hasher = Sha256::new();

    hasher.update(msg.payload);
    hasher.update(msg.content_topic.as_bytes());

    let digest = hasher.finalize().as_slice().to_vec();

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
    use crate::waku_store::{compute_index, DEFAULT_PUBSUB_TOPIC};
    use sha2::{Digest, Sha256};
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
}
