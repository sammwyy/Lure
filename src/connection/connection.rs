use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::time::timeout;
use valence_protocol::packets::s2c::play::DisconnectPlay;
use valence_protocol::{DecodePacket, EncodePacket, Text};

use super::codec::{PacketDecoder, PacketEncoder};

const READ_BUF_SIZE: usize = 4096;

pub struct Connection {
    pub address: SocketAddr,
    pub enc: PacketEncoder,
    pub dec: PacketDecoder,
    pub read: OwnedReadHalf,
    pub write: OwnedWriteHalf,
    pub buf: String,
}

impl Connection {
    pub fn enable_encryption(&mut self, key: &[u8; 16]) {
        self.enc.enable_encryption(key);
        self.dec.enable_encryption(key);
    }

    pub async fn disconnect(&mut self, reason: Text) -> anyhow::Result<()> {
        let kick = DisconnectPlay {
            reason: reason.into(),
        };
        self.send(&kick).await?;
        Ok(())
    }

    pub async fn set_compression(&mut self, threshold: u32) -> anyhow::Result<()> {
        self.dec.set_compression(true);
        self.enc.set_compression(Some(threshold));
        Ok(())
    }

    pub async fn recv<'a, P>(&'a mut self) -> anyhow::Result<P>
    where
        P: DecodePacket<'a> + EncodePacket,
    {
        while !self.dec.has_next_packet()? {
            self.dec.reserve(READ_BUF_SIZE);
            let mut buf = self.dec.take_capacity();

            if self.read.read_buf(&mut buf).await? == 0 {
                return Err(io::Error::from(ErrorKind::UnexpectedEof).into());
            }

            self.dec.queue_bytes(buf);
        }

        Ok(self
            .dec
            .try_next_packet()?
            .expect("decoder said it had another packet"))
    }

    pub async fn send<P>(&mut self, pkt: &P) -> anyhow::Result<()>
    where
        P: EncodePacket + ?Sized,
    {
        self.enc.append_packet(pkt)?;
        let bytes = self.enc.take();
        timeout(Duration::from_millis(5000), self.write.write_all(&bytes)).await??;
        Ok(())
    }

    pub async fn pipe<'a, P>(&'a mut self) -> anyhow::Result<()>
    where
        P: DecodePacket<'a> + EncodePacket,
    {
        while !self.dec.has_next_packet()? {
            self.dec.reserve(4096);
            let mut buf = self.dec.take_capacity();

            if self.read.read_buf(&mut buf).await? == 0 {
                return Err(io::Error::from(ErrorKind::UnexpectedEof).into());
            }

            self.dec.queue_bytes(buf);
        }

        let pkt: P = self.dec.try_next_packet()?.expect("Packet was None");
        self.enc.append_packet(&pkt)?;

        let bytes = self.enc.take();
        self.write.write_all(&bytes).await?;

        self.buf.clear();

        Ok(())
    }
}
