use crate::pb::waku_message_pb::WakuMessage;
use libp2p::gossipsub::IdentTopic;
use libp2p::{
    gossipsub::{
        error::{PublishError, SubscriptionError},
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, GossipsubMessage, MessageAuthenticity,
        MessageId, ValidationMode,
    },
    NetworkBehaviour, PeerId,
};
use protobuf::Message;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const DEFAULT_PUBSUB_TOPIC: &str = "/waku/2/default-waku/proto";
const RELAY_PROTOCOL_ID: &str = "/vac/waku/relay/2.0.0";

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WakuRelayEvent")]
pub struct WakuRelayBehaviour {
    gossipsub: Gossipsub,
}

#[derive(Debug)]
pub enum WakuRelayEvent {
    GossipSub(GossipsubEvent),
}

impl From<GossipsubEvent> for WakuRelayEvent {
    fn from(event: GossipsubEvent) -> Self {
        Self::GossipSub(event)
    }
}

impl WakuRelayBehaviour {
    pub fn new() -> Self {
        let message_id_fn = |message: &GossipsubMessage| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = GossipsubConfigBuilder::default()
            .protocol_id_prefix(RELAY_PROTOCOL_ID)
            .validation_mode(ValidationMode::Anonymous) // StrictNoSign
            .message_id_fn(message_id_fn)
            .build()
            .expect("Valid config");

        let gossipsub: Gossipsub = Gossipsub::new(MessageAuthenticity::Anonymous, gossipsub_config)
            .expect("Correct configuration");

        WakuRelayBehaviour { gossipsub }
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<bool, SubscriptionError> {
        let ident_topic = IdentTopic::new(topic);
        self.gossipsub.subscribe(&ident_topic)
    }

    pub fn publish(&mut self, topic: &str, msg: WakuMessage) -> Result<MessageId, PublishError> {
        let ident_topic = IdentTopic::new(topic);
        let msg_bytes = match msg.write_to_bytes() {
            Ok(b) => b,
            Err(_) => panic!("can't write WakuMessage bytes"), // todo: proper error propagation
        };
        self.gossipsub.publish(ident_topic, msg_bytes)
    }

    pub fn add_peer(&mut self, peer_id: &PeerId) {
        self.gossipsub.add_explicit_peer(peer_id);
    }
}
