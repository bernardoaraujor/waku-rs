use crate::pb::waku_message_pb::WakuMessage;
use libp2p::{
    gossipsub::{
        error::{PublishError, SubscriptionError},
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, MessageId,
        Sha256Topic, ValidationMode,
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

    fn subscribe(&mut self, topic: &str) -> Result<bool, SubscriptionError> {
        let sha256topic = Sha256Topic::new(topic);
        self.gossipsub.subscribe(&sha256topic)
    }

    pub fn publish(&mut self, topic: &str, msg: WakuMessage) -> Result<MessageId, PublishError> {
        let sha256topic = Sha256Topic::new(topic);
        let msg_bytes = match msg.write_to_bytes() {
            Ok(b) => b,
            Err(_) => panic!("can't write WakuMessage bytes"), // todo: proper error propagation
        };
        self.gossipsub.publish(sha256topic, msg_bytes)
    }

    fn add_peer(&mut self, peer_id: &PeerId) {
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

    const KEY_A: &str = "23jhTbXRXh1RPMwzN2B7GNXZDiDtrkdm943bVBfAQBJFUosggfSDVQzui7pEbuzBFf6x7C5SLWXvUGB1gPaTLTpwRxDYu";
    const KEY_B: &str = "23jhTfVepCSFrkYE8tATMUuxU3SErCYvrShcit6dQfaonM4QxF82wh4k917LJShErtKNNbaUjmqGVDLDQdVB9n7TGieQ1";

    const PEER_ID_A: &str = "12D3KooWLyTCx9j2FMcsHe81RMoDfhXbdyyFgNGQMdcrnhShTvQh";
    const PEER_ID_B: &str = "12D3KooWKBKXsLwbmVBySEmbKayJzfWp3tPCKrnDCsmNy9prwjvy";

    #[async_std::test]
    async fn my_test() -> std::io::Result<()> {
        let decoded_key_a = bs58::decode(&KEY_A.to_string()).into_vec().unwrap();
        let key_a = Keypair::from_protobuf_encoding(&decoded_key_a).unwrap();
        let address_a = Multiaddr::from_str(&ADDR_A.to_string()).unwrap();
        let peer_id_a = PeerId::from_str(PEER_ID_A).unwrap();
        let transport_a = libp2p::development_transport(key_a.clone()).await?;

        let decoded_key_b = bs58::decode(&KEY_B.to_string()).into_vec().unwrap();
        let key_b = Keypair::from_protobuf_encoding(&decoded_key_b).unwrap();
        let address_b = Multiaddr::from_str(&ADDR_B.to_string()).unwrap();
        let peer_id_b = PeerId::from_str(PEER_ID_B).unwrap();
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

        swarm_a.dial(address_b).unwrap();

        loop {
            let msg = WakuMessage::new();
            swarm_a.behaviour_mut().publish(topic.clone(), msg).unwrap();
            select! { event = swarm_a.select_next_some() => {
                println!("a {:?}", event);
            }, event = swarm_b.select_next_some() => {
                println!("b {:?}", event);
            }};
        }
    }
}
