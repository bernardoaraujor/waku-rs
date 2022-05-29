use crate::pb::waku_message_pb::WakuMessage;
use crate::pb::waku_store_pb::Index;
use std::collections::{vec_deque::Iter, VecDeque};

#[derive(Clone, Debug)]
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
}

pub struct WakuMessageQueue {
    max_messages: usize,
    messages: VecDeque<IndexedWakuMessage>,
}

impl WakuMessageQueue {
    pub fn new(max_messages: usize) -> Self {
        WakuMessageQueue {
            max_messages,
            messages: VecDeque::with_capacity(max_messages),
        }
    }

    pub fn push(&mut self, indexed_message: IndexedWakuMessage) {
        if self.messages.len() == self.messages.capacity() {
            self.messages.pop_front();
        }
        self.messages.push_back(indexed_message);
    }

    pub fn pop(&mut self) -> Option<IndexedWakuMessage> {
        self.messages.pop_front()
    }

    pub fn iter(&self) -> Iter<'_, IndexedWakuMessage> {
        self.messages.iter()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::pb::waku_message_pb::WakuMessage;
    use crate::waku_store::message_queue::{IndexedWakuMessage, WakuMessageQueue};
    use crate::waku_store::network_behaviour::compute_index;

    #[test]
    fn test_message_queue() {
        let msg = WakuMessage::new();
        let indexed_msg = IndexedWakuMessage::new(
            msg.clone(),
            compute_index(msg.clone()),
            "test_pubsub_topic".to_string(),
        );

        let mut msg_queue = WakuMessageQueue::new(3);
        assert_eq!(0, msg_queue.len());

        msg_queue.push(indexed_msg.clone());
        assert_eq!(1, msg_queue.len());

        msg_queue.push(indexed_msg.clone());
        assert_eq!(2, msg_queue.len());

        msg_queue.push(indexed_msg.clone());
        assert_eq!(3, msg_queue.len());

        msg_queue.push(indexed_msg.clone());
        assert_eq!(3, msg_queue.len());

        for i in msg_queue.iter() {
            println!("{:?}", i);
        }
    }
}
