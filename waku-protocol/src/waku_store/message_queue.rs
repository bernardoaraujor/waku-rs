use crate::pb::waku_message_pb::WakuMessage;
use crate::pb::waku_store_pb::Index;
use std::collections::{vec_deque::Iter, HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq)]
pub struct IndexedWakuMessage {
    message: WakuMessage,
    index: Index,
    pubsub_topic: String,
}

impl IndexedWakuMessage {
    pub fn new(message: WakuMessage, index: Index, pubsub_topic: String) -> Self {
        IndexedWakuMessage {
            message,
            index,
            pubsub_topic,
        }
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn pubsub_topic(&self) -> &String {
        &self.pubsub_topic
    }

    pub fn content_topic(&self) -> &str {
        &self.message.get_content_topic()
    }

    pub fn message(&self) -> &WakuMessage {
        &self.message
    }
}

pub struct WakuMessageQueue {
    messages: VecDeque<IndexedWakuMessage>,
    queued_digests: HashSet<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WakuMessageQueueErrors {
    Duplicated,
}

impl WakuMessageQueue {
    pub fn new(max_messages: usize) -> Self {
        WakuMessageQueue {
            messages: VecDeque::with_capacity(max_messages),
            queued_digests: HashSet::new(),
        }
    }

    pub fn push(
        &mut self,
        indexed_message: IndexedWakuMessage,
    ) -> Result<(), WakuMessageQueueErrors> {
        if self
            .queued_digests
            .contains(indexed_message.index.get_digest())
        {
            return Err(WakuMessageQueueErrors::Duplicated);
        }

        // todo: avoid future timestamps (?)
        // if indexed_message.index.sender_time - indexed_message.index.receiver_time
        //     > MAX_TIME_VARIANCE
        // {
        //     return Err(WakuMessageQueueErrors::FutureMessageError);
        // }

        self.queued_digests
            .insert(indexed_message.index.get_digest().to_vec());

        if self.messages.len() == self.messages.capacity() {
            // drop oldest from VecDeque
            if let Some(front) = self.messages.pop_front() {
                // drop oldest from HashSet
                self.queued_digests.take(front.index.get_digest());
            }
        }
        self.messages.push_back(indexed_message);

        Ok(())
    }

    pub fn iter(&self) -> Iter<'_, IndexedWakuMessage> {
        self.messages.iter()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn front(&self) -> Option<&IndexedWakuMessage> {
        self.messages.front()
    }

    pub fn back(&self) -> Option<&IndexedWakuMessage> {
        self.messages.back()
    }

    pub fn get(&self, i: usize) -> Option<&IndexedWakuMessage> {
        self.messages.get(i)
    }

    pub fn has_queued_digest(&self, digest: Vec<u8>) -> bool {
        self.queued_digests.contains(&digest)
    }
}

#[cfg(test)]
mod tests {
    use crate::pb::waku_message_pb::WakuMessage;
    use crate::waku_store::message_queue::{
        IndexedWakuMessage, WakuMessageQueue, WakuMessageQueueErrors,
    };
    use crate::waku_store::network_behaviour::compute_index;

    fn create_indexed_message(
        payload: Vec<u8>,
        content_topic: String,
        pubsub_topic: String,
    ) -> IndexedWakuMessage {
        let mut msg = WakuMessage::new();
        msg.set_payload(payload);
        msg.set_content_topic(content_topic);
        IndexedWakuMessage::new(msg.clone(), compute_index(msg.clone()), pubsub_topic)
    }

    #[test]
    fn test_message_queue() {
        let mut msg_queue = WakuMessageQueue::new(3);
        assert_eq!(0, msg_queue.len());

        let indexed_msg1 = create_indexed_message(
            b"1".to_vec(),
            String::from("test_content_topic"),
            String::from("test_pubsub_topic"),
        );

        msg_queue.push(indexed_msg1.clone()).unwrap();
        assert_eq!(1, msg_queue.len());
        assert!(msg_queue.has_queued_digest(indexed_msg1.clone().index.digest));

        let result = msg_queue.push(indexed_msg1.clone());
        assert_eq!(result, Err(WakuMessageQueueErrors::Duplicated));

        let indexed_msg2 = create_indexed_message(
            b"2".to_vec(),
            String::from("test_content_topic"),
            String::from("test_pubsub_topic"),
        );

        msg_queue.push(indexed_msg2.clone()).unwrap();
        assert_eq!(2, msg_queue.len());

        let indexed_msg3 = create_indexed_message(
            b"3".to_vec(),
            String::from("test_content_topic"),
            String::from("test_pubsub_topic"),
        );

        msg_queue.push(indexed_msg3.clone()).unwrap();
        assert_eq!(3, msg_queue.len());

        let indexed_msg4 = create_indexed_message(
            b"4".to_vec(),
            String::from("test_content_topic"),
            String::from("test_pubsub_topic"),
        );

        assert_eq!(&indexed_msg1, msg_queue.front().unwrap());
        assert_eq!(&indexed_msg3, msg_queue.back().unwrap());
        assert!(msg_queue.has_queued_digest(indexed_msg1.clone().index.digest));

        msg_queue.push(indexed_msg4.clone()).unwrap();
        assert_eq!(3, msg_queue.len()); // hit max_messages, len stays the same

        assert_eq!(&indexed_msg2, msg_queue.front().unwrap());
        assert_eq!(&indexed_msg4, msg_queue.back().unwrap());
        assert!(!msg_queue.has_queued_digest(indexed_msg1.index.digest));
    }
}
