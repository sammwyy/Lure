use std::error::Error;
use std::{net::SocketAddr, sync::Arc};

use serde_json::json;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use valence::prelude::*;
use valence_protocol::packets::c2s::handshake::HandshakeOwned;
use valence_protocol::packets::c2s::status::{PingRequest, StatusRequest};
use valence_protocol::packets::s2c::status::{PingResponse, StatusResponse};
use valence_protocol::types::HandshakeNextState;
use valence_protocol::{PacketDecoder, PacketEncoder};

use crate::config::LureConfig;
use crate::state::State;

#[derive(Clone, Debug)]
pub struct Lure {
    config: LureConfig,
}

impl Lure {
    pub fn new(config: LureConfig) -> Lure {
        Lure { config }
    }

    pub async fn start(self) -> Result<(), Box<dyn Error>> {
        // Listener Config
        let listener_cfg = self.config.listener.to_owned();
        println!("Preparing socket {}", listener_cfg.bind);
        let address: SocketAddr = listener_cfg.bind.parse().unwrap();
        let max_connections = listener_cfg.max_connections;

        // Start server.
        let listener = TcpListener::bind(address).await?;
        let semaphore = Arc::new(Semaphore::new(max_connections));

        while let Ok(permit) = semaphore.clone().acquire_owned().await {
            let (client, remote_client_addr) = listener.accept().await?;
            eprintln!("Accepted connection to {remote_client_addr}");

            if let Err(e) = client.set_nodelay(true) {
                eprintln!("Failed to set TCP_NODELAY: {e}");
            }

            let lure = self.clone();
            tokio::spawn(async move {
                if let Err(e) = lure.handle_connection(client).await {
                    eprintln!("Connection to {remote_client_addr} ended with: {e:#}");
                } else {
                    eprintln!("Connection to {remote_client_addr} ended.");
                }
                drop(permit);
            });
        }

        println!("Starting Lure server.");
        Ok(())
    }

    pub async fn handle_connection(self, client_socket: TcpStream) -> anyhow::Result<()> {
        // Client state
        let (client_read, client_write) = client_socket.into_split();

        let mut client = State {
            enc: PacketEncoder::new(),
            dec: PacketDecoder::new(),
            read: client_read,
            write: client_write,
            buf: String::new(),
        };

        // Wait for initial handshake.
        let handshake: HandshakeOwned = client.recv().await?;
        match handshake.next_state {
            HandshakeNextState::Status => {
                self.handle_status(client, handshake).await?;
            }
            HandshakeNextState::Login => {
                self.handle_login(client, handshake).await?;
            }
        }

        Ok(())
    }

    pub async fn handle_status(
        self,
        mut client: State,
        handshake: HandshakeOwned,
    ) -> anyhow::Result<()> {
        client.recv::<StatusRequest>().await?;

        let proxy = self.config.proxy.to_owned();
        let max_players = proxy.max_players;
        let motd: Text = proxy.motd.into();
        let protocol = handshake.protocol_version.0;

        let json = json!({
            "version": {
                "name": "Lure",
                "protocol": protocol
            },
            "players": {
                "online": 0,
                "max": max_players,
                "sample": vec![PlayerSampleEntry {
                    name: "foobar".into(),
                    id: Uuid::from_u128(12345),
                }],
            },
            "description": motd,
            "favicon": ""
        });

        client
            .send(&StatusResponse {
                json: &json.to_string(),
            })
            .await?;

        let PingRequest { payload } = client.recv::<PingRequest>().await?;
        client.send(&PingResponse { payload }).await?;
        Ok(())
    }

    pub async fn handle_login(
        self,
        client: State,
        handshake: HandshakeOwned,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
