use std::net::IpAddr;

use valence_protocol::{types::Property, Username, Uuid};

pub struct ClientInfo {
    /// The username of the new client.
    pub username: Username<String>,
    /// The UUID of the new client.
    pub uuid: Uuid,
    /// The remote address of the new client.
    pub ip: IpAddr,
    /// The client's properties from the game profile. Typically contains a
    /// `textures` property with the skin and cape of the player.
    pub properties: Vec<Property>,

    pub protocol_version: i32,
    pub hostname: String,
}
