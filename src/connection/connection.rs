use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::timeout;

use valence_protocol::packets::s2c::login::SetCompression;
use valence_protocol::{DecodePacket, EncodePacket, PacketDecoder, PacketEncoder, VarInt};

const READ_BUF_SIZE: usize = 4096;

pub struct Connection {
    pub address: SocketAddr,
    pub enc: PacketEncoder,
    pub dec: PacketDecoder,
    pub read: OwnedReadHalf,
    pub write: OwnedWriteHalf,
    pub buf: String,
    pub permit: OwnedSemaphorePermit,
}

impl Connection {
    /*
    pub fn enable_encryption(&mut self, key: &[u8; 16]) {
        self.enc.enable_encryption(key);
        self.dec.enable_encryption(key);
    }
    */

    pub async fn set_compression(&mut self, threshold: u32) -> anyhow::Result<()> {
        self.send(&SetCompression {
            threshold: VarInt(threshold as i32),
        })
        .await?;

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
}
