use crate::Peer;
use anyhow::Result;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// peers to connect to on startup
    #[serde(default = "default_peers")]
    pub peers: Vec<Peer>,

    // TODO: only for development
    #[serde(default = "default_secret_key_seed")]
    pub secret_key_seed: u8,

    /// specify whether or not you will be a relay node for others.
    /// By default, is true
    #[serde(default = "default_is_relay")]
    pub is_relay: bool,

    /// The port used to listen on all interfaces
    #[serde(default = "default_port")]
    pub port: u16,

    /// The number of nodes that gossipsub sends full messages to
    #[serde(default = "default_gossipsub_connections")]
    pub num_gossipsub_connections: GossipsubConnections,
}

fn default_peers() -> Vec<Peer> {
    Vec::new()
}

fn default_port() -> u16 {
    4021
}

fn default_is_relay() -> bool {
    true
}

fn default_secret_key_seed() -> u8 {
    fastrand::u8(0..u8::MAX)
}

/// The number of nodes that gossipsub sends full messages to.
/// The number of gossipsub connections (also called the network degree, D, or peering degree) controls the trade-off
/// between speed, reliability, resilience and efficiency of the network. A higher peering degree helps messages get
/// delivered faster, with a better chance of reaching all subscribers and with less chance of any peer disrupting the
/// network by leaving. However, a high peering degree also causes additional redundant copies of each message to be
/// sent throughout the network, increasing the bandwidth required to participate in the network.
#[derive(Debug, Deserialize, Clone)]
pub struct GossipsubConnections {
    /// target number of connections.  Gossipsub will try to form this many connections, but will
    /// accept `lower_tolerance` less connections or
    /// `upper_tolerance` more connections
    #[serde(default = "default_gossipsub_target_num")]
    target_num: usize,

    /// how many connections under `target_num` is acceptable
    #[serde(default = "default_gossipsub_connections_lower_tolerance")]
    lower_tolerance: usize,

    /// how many connections above `target_num` is acceptable
    #[serde(default = "default_gossipsub_connections_upper_tolerance")]
    upper_tolerance: usize,
}

// called in main in the gossipsub builder.
// The gossipsub builder refers to the peering degrees as "mesh_n", "mesh_n_high", "mesh_n_low"
impl GossipsubConnections {
    pub fn mesh_n(&self) -> usize {
        self.target_num
    }

    // return the actual minimum number of connections, not the relative number
    pub fn mesh_n_low(&self) -> usize {
        let target_num = self.target_num;
        let lower_tolerance = self.lower_tolerance;

        // TODO: should this bottom out at 0 or 1?
        if lower_tolerance > target_num {
            0
        } else {
            target_num - lower_tolerance
        }
    }

    // return the actual maximum number of connections, not the relative number
    pub fn mesh_n_high(&self) -> usize {
        self.target_num + self.upper_tolerance
    }
}

impl Default for GossipsubConnections {
    fn default() -> Self {
        Self {
            target_num: default_gossipsub_target_num(),
            lower_tolerance: default_gossipsub_connections_lower_tolerance(),
            upper_tolerance: default_gossipsub_connections_upper_tolerance(),
        }
    }
}

fn default_gossipsub_connections() -> GossipsubConnections {
    GossipsubConnections::default()
}

// try to connect with 6 nodes to send full messages to.
fn default_gossipsub_target_num() -> usize {
    6
}

// default of 1 means 1 less connection than target is acceptable
fn default_gossipsub_connections_lower_tolerance() -> usize {
    1
}

// default of 6 means 6 more connections than target is acceptable
fn default_gossipsub_connections_upper_tolerance() -> usize {
    6
}

impl Config {
    pub fn parse(config_file_path: &str) -> Result<Self> {
        let config_content = fs::read_to_string(config_file_path)?;
        let config: Config = toml::from_str(&config_content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use libp2p::{Multiaddr, PeerId};

    #[test]
    fn test_parse() {
        let cfg = Config::parse("example_priory.toml").unwrap();

        assert_eq!(cfg.port, 0);
        assert!(cfg.is_relay);
        assert_eq!(cfg.secret_key_seed, 1);
        assert_eq!(
            cfg.peers,
            vec![Peer {
                multiaddr: Multiaddr::from_str("/ip4/142.93.53.125/tcp/4001").unwrap(),
                peer_id: PeerId::from_str("12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN")
                    .unwrap()
            }]
        );
        assert_eq!(cfg.num_gossipsub_connections.mesh_n(), 7);
        assert_eq!(cfg.num_gossipsub_connections.mesh_n_low(), 5);
        assert_eq!(cfg.num_gossipsub_connections.mesh_n_high(), 12);
    }

    #[test]
    fn test_gossipsub_connection_lower_bound_overflow() {
        let toml_str = r#"
            peers = []

            [num_gossipsub_connections]
            target_num = 1 
            lower_tolerance = 2
            upper_tolerance = 4
        "#;

        let cfg: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(cfg.num_gossipsub_connections.mesh_n_low(), 0);
    }
}
