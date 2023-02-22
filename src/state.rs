use std::io;
use std::io::ErrorKind;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::time::timeout;
use valence_protocol::{DecodePacket, EncodePacket, PacketDecoder, PacketEncoder};

pub struct State {
    pub enc: PacketEncoder,
    pub dec: PacketDecoder,
    pub read: OwnedReadHalf,
    pub write: OwnedWriteHalf,
    pub buf: String,
}

impl State {
    pub async fn send<P>(&mut self, pkt: &P) -> anyhow::Result<()>
    where
        P: EncodePacket + ?Sized,
    {
        self.enc.append_packet(pkt)?;
        let bytes = self.enc.take();
        timeout(Duration::from_millis(5000), self.write.write_all(&bytes)).await??;
        Ok(())
    }

    pub async fn recv<'a, P>(&'a mut self) -> anyhow::Result<P>
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

        Ok(self
            .dec
            .try_next_packet()?
            .expect("decoder said it had another packet"))
    }
}
