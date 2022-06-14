// this file is being used for development purposes!
// cargo run --bin relay -- x_pubsub_topic_x
// cargo run --bin relay -- x_pubsub_topic_x /ip4/127.0.0.1/tcp/xxxxx

use libp2p::futures::StreamExt;
use libp2p::{identity::Keypair, swarm::Swarm, Multiaddr, PeerId};
use log::info;
use std::error::Error;
use waku_protocol::waku_relay::network_behaviour::WakuRelayBehaviour;

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

    loop {
        let event = swarm.select_next_some().await;
        info!("{:?}", event);
    }
}
