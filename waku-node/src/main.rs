use clap::Parser;
use libp2p::futures::StreamExt;
use libp2p::{identity::Keypair, swarm::Swarm, Multiaddr, PeerId};
use log::info;
use network_behaviour::WakuNodeBehaviour;
use std::error::Error;
use waku_protocol::waku_relay::network_behaviour::DEFAULT_PUBSUB_TOPIC;

pub mod network_behaviour;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Enable relay protocol
    #[clap(long)]
    relay: bool,

    /// Multiaddr of peer to directly connect with. Option may be repeated
    #[clap(long)]
    static_node: Option<Vec<Multiaddr>>,

    /// List of topics to listen
    #[clap(long)]
    topics: Option<Vec<String>>,

    /// Enable store protocol
    #[clap(long)]
    store: bool,

    /// Maximum number of messages to store
    #[clap(long, default_value = "50000")]
    store_capacity: usize,

    /// Enable lightpush protocol
    #[clap(long)]
    lightpush: bool,
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Cli::parse();

    let local_key = Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local peer id: {:?}", local_peer_id);

    let transport = libp2p::development_transport(local_key.clone()).await?;

    let mut waku_node_behaviour =
        WakuNodeBehaviour::new(args.relay, args.store, args.store_capacity, args.lightpush);

    match args.topics {
        Some(topics) => {
            for t in topics {
                waku_node_behaviour.subscribe(&t).unwrap();
            }
        }
        None => waku_node_behaviour.subscribe(DEFAULT_PUBSUB_TOPIC).unwrap(),
    }

    let mut swarm = Swarm::new(transport, waku_node_behaviour, local_peer_id);
    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    match args.static_node {
        Some(addresses) => {
            for a in addresses {
                match swarm.dial(a.clone()) {
                    Ok(_) => info!("Dialed {:?}", a),
                    Err(e) => panic!("Failed to dial address: {:?} {:?}", a, e),
                }
            }
        }
        None => {}
    }

    loop {
        let event = swarm.select_next_some().await;
        info!("{:?}", event);
    }

    // Ok(())
}
