use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::NetworkBehaviour;
use waku_protocol::waku_relay::network_behaviour::{WakuRelayBehaviour, WakuRelayEvent};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WakuNodeEvent")]
pub struct WakuNodeBehaviour {
    relay: Toggle<WakuRelayBehaviour>,
}

#[derive(Debug)]
pub enum WakuNodeEvent {
    WakuRelayBehaviour(WakuRelayEvent),
}

impl From<WakuRelayEvent> for WakuNodeEvent {
    fn from(event: WakuRelayEvent) -> Self {
        Self::WakuRelayBehaviour(event)
    }
}

#[derive(Debug)]
pub enum WakuNodeError {
    RelayNotEnabled,
    RelaySubscriptionErr,
}

impl WakuNodeBehaviour {
    pub fn new(relay_enabled: bool) -> Self {
        let relay = match relay_enabled {
            true => Toggle::from(Some(WakuRelayBehaviour::new())),
            false => Toggle::from(None),
        };

        WakuNodeBehaviour { relay }
    }

    pub fn subscribe(&mut self, topic: &str) -> Result<(), WakuNodeError> {
        match self.relay.as_mut() {
            Some(r) => match r.subscribe(topic) {
                Ok(_) => return Ok(()),
                Err(_) => return Err(WakuNodeError::RelaySubscriptionErr),
            },
            None => return Err(WakuNodeError::RelayNotEnabled),
        }
    }
}
