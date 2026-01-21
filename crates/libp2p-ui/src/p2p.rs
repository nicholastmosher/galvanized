use std::{
    hash::{DefaultHasher, Hash as _, Hasher as _},
    time::Duration,
};

use anyhow::{Context, Error};
use culpa::throws;
use libp2p::{
    Multiaddr, PeerId, Swarm, dcutr,
    futures::StreamExt as _,
    gossipsub, identify,
    kad::{self, store::MemoryStore},
    mdns,
    multiaddr::Protocol,
    noise, ping, relay, tcp, yamux,
};
use libp2p_swarm::{NetworkBehaviour, SwarmEvent};

const BOOTSTRAP: &str = "/dnsaddr/bootstrap.libp2p.io";

const BOOTNODES: [&str; 4] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];

#[derive(NetworkBehaviour)]
pub struct PeerieBehaviour {
    pub dcutr: dcutr::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub kad: kad::Behaviour<MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub ping: ping::Behaviour,
    pub stream: libp2p_stream::Behaviour,
    pub relay_client: relay::client::Behaviour,
    // pub relay_server: relay::Behaviour,
}

impl PeerieBehaviour {
    #[throws]
    pub async fn try_init_swarm() -> Swarm<PeerieBehaviour> {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_quic()
            .with_relay_client(noise::Config::new, yamux::Config::default)?
            .with_behaviour(|key, relay_client| {
                let dcutr = dcutr::Behaviour::new(key.public().to_peer_id());

                // To content-address message, we can take the hash of message and use it as an ID.
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };

                // Set a custom gossipsub configuration
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
                    .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                    .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
                    .build()?;

                // build a gossipsub network behaviour
                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )?;

                let identify = identify::Behaviour::new(identify::Config::new(
                    "/prototype/0.0.1".to_string(),
                    key.public(),
                ));

                let kad = {
                    let mut kad = kad::Behaviour::new(
                        key.public().to_peer_id(),
                        MemoryStore::new(key.public().to_peer_id()),
                    );

                    let bootaddr = BOOTSTRAP.parse::<Multiaddr>()?;
                    for peer in &BOOTNODES {
                        kad.add_address(&peer.parse::<PeerId>()?, bootaddr.clone());
                    }

                    kad
                };

                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )?;

                let ping =
                    ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(3)));

                // let relay_server =
                //     relay::Behaviour::new(key.public().to_peer_id(), Default::default());
                let stream = libp2p_stream::Behaviour::new();

                Ok(PeerieBehaviour {
                    dcutr,
                    gossipsub,
                    identify,
                    kad,
                    mdns,
                    ping,
                    stream,
                    relay_client,
                    // relay_server,
                })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // Create a Gossipsub topic
        // let topic = gossipsub::IdentTopic::new("test-net");
        // subscribes to our topic
        // swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

        // Listen on all interfaces and whatever port the OS assigns
        swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        let local_peer_id = *swarm.local_peer_id();
        let query_id = swarm.behaviour_mut().kad.get_closest_peers(local_peer_id);
        tracing::info!(%local_peer_id, %query_id, "Kademlia query");

        // swarm.listen_on(
        //     BOOTSTRAP
        //         .parse::<Multiaddr>()
        //         .unwrap()
        //         .with(Protocol::P2pCircuit),
        // )?;

        swarm
    }
}
