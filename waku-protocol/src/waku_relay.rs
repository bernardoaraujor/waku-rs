use crate::pb::waku_message_pb::WakuMessage;
use libp2p::gossipsub::error::{PublishError, SubscriptionError};
use libp2p::gossipsub::{
    Gossipsub, GossipsubEvent, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity,
    MessageId, ValidationMode,
};
use libp2p::NetworkBehaviour;
use libp2p::{gossipsub, identity};
use std::time::Duration;

const RELAY_PROTOCOL_ID: &str = "/vac/waku/relay/2.0.0";

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WakuRelayEvent")]
pub struct WakuRelayBehaviour {
    gossipsub: Gossipsub,
}

pub enum WakuRelayEvent {
    GossipSub(GossipsubEvent),
}

impl From<GossipsubEvent> for WakuRelayEvent {
    fn from(event: GossipsubEvent) -> Self {
        Self::GossipSub(event)
    }
}

impl WakuRelayBehaviour {
    pub fn new(key: identity::Keypair) -> Self {
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .protocol_id_prefix(RELAY_PROTOCOL_ID)
            .validation_mode(ValidationMode::Anonymous) // StrictNoSign
            .build()
            .expect("Valid config");

        let mut gossipsub: gossipsub::Gossipsub =
            gossipsub::Gossipsub::new(MessageAuthenticity::Anonymous, gossipsub_config)
                .expect("Correct configuration");

        WakuRelayBehaviour { gossipsub }
    }
}
