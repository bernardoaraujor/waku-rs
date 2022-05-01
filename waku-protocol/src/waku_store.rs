use crate::pb::waku_message_pb::WakuMessage;
use crate::pb::waku_store_pb::Index;

use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_PUBSUB_TOPIC: &str = "/waku/2/default-waku/proto";

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
