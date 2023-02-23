use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use serde_json::json;

use sha2::{Digest, Sha256};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

use tokio::task::JoinHandle;
use valence::prelude::*;
use valence_protocol::packets::c2s::handshake::{Handshake, HandshakeOwned};
use valence_protocol::packets::c2s::login::LoginStart;
use valence_protocol::packets::c2s::status::{PingRequest, StatusRequest};
use valence_protocol::packets::s2c::login::{LoginSuccess, SetCompression};
use valence_protocol::packets::s2c::status::{PingResponse, StatusResponse};
use valence_protocol::packets::{C2sPlayPacket, S2cPlayPacket};
use valence_protocol::types::HandshakeNextState;
use valence_protocol::VarInt;

use crate::config::LureConfig;
use crate::connection::client_info::ClientInfo;
use crate::connection::codec::{PacketDecoder, PacketEncoder};
use crate::connection::connection::Connection;

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
                if let Err(e) = lure.handle_connection(client, remote_client_addr).await {
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

    pub async fn handle_connection(
        &self,
        client_socket: TcpStream,
        address: SocketAddr,
    ) -> anyhow::Result<()> {
        // Client state
        let (client_read, client_write) = client_socket.into_split();

        let mut connection = Connection {
            address,
            enc: PacketEncoder::new(),
            dec: PacketDecoder::new(),
            read: client_read,
            write: client_write,
            buf: String::new(),
        };

        // Wait for initial handshake.
        let handshake: HandshakeOwned = connection.recv().await?;
        match handshake.next_state {
            HandshakeNextState::Status => self.handle_status(&mut connection, handshake).await,
            HandshakeNextState::Login => match self.handle_login(&mut connection, handshake).await?
            {
                Some(info) => {
                    // let mut client = connection.into_client(info, 2097152, 8388608);
                    self.handle_play(connection, info).await?;
                    Ok(())
                }
                None => Ok(()),
            },
        }
    }

    pub async fn handle_status(
        &self,
        client: &mut Connection,
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
        &self,
        client: &mut Connection,
        handshake: HandshakeOwned,
    ) -> anyhow::Result<Option<ClientInfo>> {
        let proxy_config = self.config.proxy.to_owned();
        let online_mode = proxy_config.online_mode;
        let compression = proxy_config.compression_threshold;

        let LoginStart {
            username,
            profile_id: _,
        } = client.recv::<LoginStart>().await?;

        let username = username.to_owned_username();
        let mut info = if online_mode {
            self.login_online(client, username).await?
        } else {
            self.login_offline(client, username).await?
        };

        info.protocol_version = handshake.protocol_version.0;

        if compression > 0 {
            client
                .send(&SetCompression {
                    threshold: VarInt(compression as i32),
                })
                .await?;
            client.set_compression(compression).await?;
        }

        client
            .send(&LoginSuccess {
                uuid: info.uuid,
                username: info.username.as_str_username(),
                properties: Default::default(),
            })
            .await?;

        Ok(Some(info))
    }

    async fn login_online(
        &self,
        client: &mut Connection,
        username: Username<String>,
    ) -> anyhow::Result<ClientInfo> {
        Ok(ClientInfo {
            uuid: Uuid::from_slice(&Sha256::digest(username.as_str())[..16])?,
            username,
            properties: vec![],
            ip: client.address.ip(),
            protocol_version: 0,
        })
    }

    pub async fn login_offline(
        &self,
        client: &mut Connection,
        username: Username<String>,
    ) -> anyhow::Result<ClientInfo> {
        Ok(ClientInfo {
            uuid: Uuid::from_slice(&Sha256::digest(username.as_str())[..16])?,
            username,
            properties: vec![],
            ip: client.address.ip(),
            protocol_version: 0,
        })
    }

    pub async fn handle_play(&self, client: Connection, info: ClientInfo) -> anyhow::Result<()> {
        let server_address: SocketAddr = "127.0.0.1:25566".parse().to_owned()?;
        let server_stream = TcpStream::connect(server_address).await?;

        if let Err(e) = server_stream.set_nodelay(true) {
            eprintln!("Failed to set TCP_NODELAY: {e}");
        }

        let (server_read, server_write) = server_stream.into_split();

        let mut server = Connection {
            address: server_address,
            enc: PacketEncoder::new(),
            dec: PacketDecoder::new(),
            read: server_read,
            write: server_write,
            buf: String::new(),
        };

        server
            .send(&Handshake {
                next_state: HandshakeNextState::Login,
                protocol_version: VarInt::from(info.protocol_version),
                server_address: &server_address.ip().to_string(),
                server_port: server_address.port(),
            })
            .await?;

        server
            .send(&LoginStart {
                profile_id: Some(info.uuid),
                username: info.username.as_str_username(),
            })
            .await?;

        let compression_result = server.recv::<SetCompression>().await?;
        server
            .set_compression(compression_result.threshold.0 as u32)
            .await?;
        server.recv::<LoginSuccess>().await?;

        let mut client_to_server = Connection {
            address: client.address.clone(),
            buf: client.buf.clone(),
            dec: client.dec,
            enc: server.enc,
            read: client.read,
            write: server.write,
        };

        let mut server_to_client = Connection {
            address: server_address.clone(),
            dec: server.dec,
            enc: client.enc,
            read: server.read,
            write: client.write,
            buf: String::new(),
        };

        let c2s_fut: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            loop {
                client_to_server.pipe::<C2sPlayPacket>().await?;
            }
        });

        let s2c_fut = async move {
            loop {
                server_to_client.pipe::<S2cPlayPacket>().await?;
            }
        };

        tokio::select! {
            c2s = c2s_fut => Ok(c2s??),
            s2c = s2c_fut => s2c,
        }
    }
}
