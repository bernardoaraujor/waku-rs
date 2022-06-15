// this file is being used for development purposes!
// cargo run --bin relay -- x_pubsub_topic_x
// cargo run --bin relay -- x_pubsub_topic_x /ip4/127.0.0.1/tcp/xxxxx

use async_std::io;
use async_std::io::prelude::BufReadExt;
use async_std::io::stdin;
use futures::select;
use libp2p::futures::StreamExt;
use libp2p::gossipsub::GossipsubEvent;
use libp2p::{
    gossipsub::{Gossipsub, GossipsubEvent::Subscribed},
    identity::Keypair,
    swarm::{Swarm, SwarmEvent},
    Multiaddr, PeerId,
};
use log::info;
use std::error::Error;
use std::str::FromStr;
use waku_protocol::waku_message::WakuMessage;
use waku_protocol::waku_relay::network_behaviour::WakuRelayBehaviour;
use waku_protocol::waku_relay::network_behaviour::WakuRelayEvent::GossipSub;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let local_key = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local peer id: {:?}", local_peer_id);

    let transport = libp2p::development_transport(local_key.clone()).await?;

    let mut waku_relay_behaviour = WakuRelayBehaviour::new();

    let pubsub_topic = match std::env::args().nth(1) {
        Some(t) => t,
        None => panic!("No pubsub topic provided!"),
    };

    waku_relay_behaviour.subscribe(&pubsub_topic).unwrap();

    let mut swarm = Swarm::new(transport, waku_relay_behaviour, local_peer_id);
    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    if let Some(to_dial) = std::env::args().nth(2) {
        let address: Multiaddr = to_dial.parse().expect("User to provide valid address.");
        match swarm.dial(address.clone()) {
            Ok(_) => info!("Dialed {:?}", address),
            Err(e) => panic!("failed to dial address: {:?} {:?}", address, e),
        }
    }

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();

    loop {
        select! {
            line = stdin.select_next_some() => {
                let mut msg = WakuMessage::new();
                let content_topic = "test_content_topic";
                msg.set_payload(line.expect("Stdin not to close").as_bytes().to_vec());
                msg.set_content_topic(content_topic.to_string());
                match swarm.behaviour_mut().publish(&pubsub_topic, msg) {
                    Ok(m) => info!("Published message: {}", m),
                    Err(e) => info!("Error publishing message: {}", e),
                };
            },
            event = swarm.select_next_some() => {
                info!("{:?}", event);
            }
        }
    }

    Ok(())
}
