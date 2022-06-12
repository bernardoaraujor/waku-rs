use crate::pb::waku_message_pb::WakuMessage;
use libp2p::gossipsub::IdentTopic;
use libp2p::{
    gossipsub::{
        error::{PublishError, SubscriptionError},
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, MessageId,
        ValidationMode,
    },
    NetworkBehaviour, PeerId,
};
use protobuf::Message;

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
        let gossipsub_config = GossipsubConfigBuilder::default()
            .protocol_id_prefix(RELAY_PROTOCOL_ID)
            .validation_mode(ValidationMode::Anonymous) // StrictNoSign
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

#[cfg(test)]
mod tests {
    use crate::pb::waku_message_pb::WakuMessage;
    use crate::waku_relay::network_behaviour::WakuRelayBehaviour;
    use futures::select;
    use futures::StreamExt;
    use libp2p::{identity::Keypair, Multiaddr, PeerId};
    use std::str::FromStr;

    const ADDR_A: &str = "/ip4/127.0.0.1/tcp/58584";
    const ADDR_B: &str = "/ip4/127.0.0.1/tcp/58601";

    #[async_std::test]
    async fn my_test() -> std::io::Result<()> {
        let key_a = Keypair::generate_ed25519();
        let peer_id_a = PeerId::from(key_a.public());
        let address_a = Multiaddr::from_str(&ADDR_A.to_string()).unwrap();

        let key_b = Keypair::generate_ed25519();
        let peer_id_b = PeerId::from(key_b.public());
        let address_b = Multiaddr::from_str(&ADDR_B.to_string()).unwrap();

        let transport_a = libp2p::development_transport(key_a.clone()).await?;
        let transport_b = libp2p::development_transport(key_b.clone()).await?;

        let mut waku_relay_behaviour_a = WakuRelayBehaviour::new();
        let mut waku_relay_behaviour_b = WakuRelayBehaviour::new();

        let topic = "test-topic";

        waku_relay_behaviour_a.subscribe(&topic).unwrap();
        waku_relay_behaviour_b.subscribe(&topic).unwrap();

        waku_relay_behaviour_a.add_peer(&peer_id_b);
        waku_relay_behaviour_b.add_peer(&peer_id_a);

        let mut swarm_a = libp2p::Swarm::new(transport_a, waku_relay_behaviour_a, peer_id_a);
        let mut swarm_b = libp2p::Swarm::new(transport_b, waku_relay_behaviour_b, peer_id_b);

        swarm_a.listen_on(address_a.clone()).unwrap();
        swarm_b.listen_on(address_b.clone()).unwrap();

        match swarm_a.dial(address_b.clone()) {
            Ok(_) => println!("Dialed {:?}", address_b),
            Err(e) => println!("Dial {:?} failed: {:?}", address_b, e),
        }

        loop {
            let msg = WakuMessage::new();
            match swarm_a.behaviour_mut().publish(topic.clone(), msg) {
                Ok(m) => println!("{}", m),
                Err(e) => println!("{}", e),
            }
            select! { event = swarm_a.select_next_some() => {
                println!("a {:?}", event);
            }, event = swarm_b.select_next_some() => {
                println!("b {:?}", event);
            }};
        }
    }
}
