use crate::pb::waku_message_pb::WakuMessage;
use crate::pb::waku_store_pb::Index;

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
    messages: Vec<IndexedWakuMessage>, // todo: ring buffer, which crate? VecDeque?
                                       // https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=14bbad4d2c074f1632122c3fb98ef8cf
}

#[derive(Debug, Clone)]
pub struct MaxQueueSize;

impl WakuMessageQueue {
    pub fn new(max_messages: usize) -> Self {
        WakuMessageQueue {
            max_messages,
            messages: Vec::new(),
        }
    }

    pub fn push(&mut self, indexed_message: IndexedWakuMessage) -> Result<(), MaxQueueSize> {
        if self.messages.len() + 1 == self.max_messages {
            return Err(MaxQueueSize);
        }
        self.messages.push(indexed_message);

        Ok(())
    }
}
