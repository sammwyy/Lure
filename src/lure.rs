use std::net::SocketAddr;

use valence::prelude::*;

use crate::config::LureConfig;

pub struct Lure {
    config: LureConfig,
}

#[async_trait]
impl AsyncCallbacks for Lure {
    async fn server_list_ping(
        &self,
        shared: &SharedServer,
        remote_addr: SocketAddr,
        protocol: i32,
    ) -> ServerListPing {
        self.server_list_ping(shared, remote_addr, protocol).await
    }

    async fn login(&self, _shared: &SharedServer, _info: &NewClientInfo) -> Result<(), Text> {
        Err("You are not meant to join this example".color(Color::RED))
    }
}

impl Lure {
    pub fn new(config: LureConfig) -> Lure {
        Lure { config }
    }

    async fn server_list_ping(
        &self,
        _shared: &SharedServer,
        remote_addr: SocketAddr,
        protocol: i32,
    ) -> ServerListPing {
        let proxy = self.config.proxy.to_owned();
        let max_players = proxy.max_players;
        let motd = proxy.motd;

        println!("[{}] Pinged with protocol {}", remote_addr, protocol);

        ServerListPing::Respond {
            online_players: 0,
            max_players,
            player_sample: vec![PlayerSampleEntry {
                name: "foobar".into(),
                id: Uuid::from_u128(12345),
            }],
            description: motd.into(),
            favicon_png: include_bytes!("../assets/default_icon.png"),
        }
    }

    pub fn start(self) {
        // Listener
        let listener = self.config.listener.to_owned();
        println!("Preparing socket {}", listener.bind);
        let address: SocketAddr = listener.bind.parse().unwrap();
        let max_connections = listener.max_connections;

        // Proxy
        let proxy = self.config.proxy.to_owned();
        let compression_threshold = proxy.compression_threshold;
        println!("Setting compression to {}", compression_threshold);
        let connection_mode = if proxy.online_mode {
            println!("Settings proxy in online mode");
            ConnectionMode::Online {
                prevent_proxy_connections: proxy.prevent_proxy_connections,
            }
        } else {
            println!("Settings proxy in offline mode");
            println!("WARNING! Server exposed to attacks.");
            ConnectionMode::Offline
        };

        let server = ServerPlugin::new(self)
            .with_address(address)
            .with_compression_threshold(Some(compression_threshold))
            .with_connection_mode(connection_mode)
            .with_max_connections(max_connections);

        println!("Starting Lure server.");
        App::new().add_plugin(server).run();
    }
}
