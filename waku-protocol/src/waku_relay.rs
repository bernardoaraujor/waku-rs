use crate::pb::waku_message_pb::WakuMessage;
use libp2p::gossipsub::error::{PublishError, SubscriptionError};
use libp2p::gossipsub::{
    GossipsubEvent, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity, MessageId,
    ValidationMode,
};
use libp2p::{gossipsub, identity};
use std::time::Duration;

const RELAY_PROTOCOL_ID: &str = "/vac/waku/relay/2.0.0";

struct WakuRelay {
    gossipsub: gossipsub::Gossipsub,
}

impl WakuRelay {
    pub fn new(key: identity::Keypair) -> Self {
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .protocol_id_prefix(RELAY_PROTOCOL_ID)
            .validation_mode(ValidationMode::Anonymous) // StrictNoSign
            .build()
            .expect("Valid config");

        let mut gossipsub: gossipsub::Gossipsub =
            gossipsub::Gossipsub::new(MessageAuthenticity::Signed(key), gossipsub_config)
                .expect("Correct configuration");

        WakuRelay { gossipsub }
    }

    fn subscribe(&mut self, topic: Topic) -> Result<bool, SubscriptionError> {
        self.gossipsub.subscribe(&topic)
    }

    fn unsubscribe(&mut self, topic: Topic) -> Result<bool, PublishError> {
        self.gossipsub.unsubscribe(&topic)
    }

    fn publish(&mut self, topic: Topic, data: Vec<u8>) -> Result<MessageId, PublishError> {
        self.gossipsub.publish(topic, data)
    }
}
