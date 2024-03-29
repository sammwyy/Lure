use aes::cipher::{AsyncStreamCipher, NewCipher};
use anyhow::{bail, ensure};
use bytes::{Buf, BufMut, BytesMut};
use tracing::debug;

use valence_protocol::var_int::{VarInt, VarIntDecodeError};
use valence_protocol::{Decode, DecodePacket, Encode, EncodePacket, Result, MAX_PACKET_SIZE};

type Cipher = cfb8::Cfb8<aes::Aes128>;

#[derive(Default)]
pub struct PacketEncoder {
    pub buf: BytesMut,
    pub compress_buf: Vec<u8>,
    pub compression_threshold: Option<u32>,
    pub cipher: Option<Cipher>,
}

impl PacketEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn append_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes)
    }

    pub fn prepend_packet<P>(&mut self, pkt: &P) -> Result<()>
    where
        P: EncodePacket + ?Sized,
    {
        let start_len = self.buf.len();
        self.append_packet(pkt)?;

        let end_len = self.buf.len();
        let total_packet_len = end_len - start_len;

        // 1) Move everything back by the length of the packet.
        // 2) Move the packet to the new space at the front.
        // 3) Truncate the old packet away.
        self.buf.put_bytes(0, total_packet_len);
        self.buf.copy_within(..end_len, total_packet_len);
        self.buf.copy_within(total_packet_len + start_len.., 0);
        self.buf.truncate(end_len);

        Ok(())
    }

    pub fn append_packet<P>(&mut self, pkt: &P) -> Result<()>
    where
        P: EncodePacket + ?Sized,
    {
        let start_len = self.buf.len();

        pkt.encode_packet((&mut self.buf).writer())?;

        let data_len = self.buf.len() - start_len;

        if let Some(threshold) = self.compression_threshold {
            use std::io::Read;

            use flate2::bufread::ZlibEncoder;
            use flate2::Compression;

            if data_len > threshold as usize {
                let mut z = ZlibEncoder::new(&self.buf[start_len..], Compression::new(4));

                self.compress_buf.clear();

                let data_len_size = VarInt(data_len as i32).written_size();

                let packet_len = data_len_size + z.read_to_end(&mut self.compress_buf)?;

                ensure!(
                    packet_len <= MAX_PACKET_SIZE as usize,
                    "packet exceeds maximum length"
                );

                drop(z);

                self.buf.truncate(start_len);

                let mut writer = (&mut self.buf).writer();

                VarInt(packet_len as i32).encode(&mut writer)?;
                VarInt(data_len as i32).encode(&mut writer)?;
                self.buf.extend_from_slice(&self.compress_buf);
            } else {
                let data_len_size = 1;
                let packet_len = data_len_size + data_len;

                ensure!(
                    packet_len <= MAX_PACKET_SIZE as usize,
                    "packet exceeds maximum length"
                );

                let packet_len_size = VarInt(packet_len as i32).written_size();

                let data_prefix_len = packet_len_size + data_len_size;

                self.buf.put_bytes(0, data_prefix_len);
                self.buf
                    .copy_within(start_len..start_len + data_len, start_len + data_prefix_len);

                let mut front = &mut self.buf[start_len..];

                VarInt(packet_len as i32).encode(&mut front)?;
                // Zero for no compression on this packet.
                VarInt(0).encode(front)?;
            }

            return Ok(());
        }

        let packet_len = data_len;

        ensure!(
            packet_len <= MAX_PACKET_SIZE as usize,
            "packet exceeds maximum length"
        );

        let packet_len_size = VarInt(packet_len as i32).written_size();

        self.buf.put_bytes(0, packet_len_size);
        self.buf
            .copy_within(start_len..start_len + data_len, start_len + packet_len_size);

        let front = &mut self.buf[start_len..];
        VarInt(packet_len as i32).encode(front)?;

        Ok(())
    }

    /// Takes all the packets written so far and encrypts them if encryption is
    /// enabled.
    pub fn take(&mut self) -> BytesMut {
        if let Some(cipher) = &mut self.cipher {
            cipher.encrypt(&mut self.buf);
        }

        self.buf.split()
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn set_compression(&mut self, threshold: Option<u32>) {
        self.compression_threshold = threshold;
    }

    /// Encrypts all future packets **and any packets that have
    /// not been [taken] yet.**
    ///
    /// [taken]: Self::take
    pub fn enable_encryption(&mut self, key: &[u8; 16]) {
        assert!(self.cipher.is_none(), "encryption is already enabled");
        self.cipher = Some(NewCipher::new(key.into(), key.into()));
    }
}

pub fn encode_packet<P>(buf: &mut Vec<u8>, pkt: &P) -> Result<()>
where
    P: EncodePacket + ?Sized,
{
    let start_len = buf.len();

    pkt.encode_packet(&mut *buf)?;

    let packet_len = buf.len() - start_len;

    ensure!(
        packet_len <= MAX_PACKET_SIZE as usize,
        "packet exceeds maximum length"
    );

    let packet_len_size = VarInt(packet_len as i32).written_size();

    buf.put_bytes(0, packet_len_size);
    buf.copy_within(
        start_len..start_len + packet_len,
        start_len + packet_len_size,
    );

    let front = &mut buf[start_len..];
    VarInt(packet_len as i32).encode(front)?;

    Ok(())
}

pub fn encode_packet_compressed<P>(
    buf: &mut Vec<u8>,
    pkt: &P,
    threshold: u32,
    scratch: &mut Vec<u8>,
) -> Result<()>
where
    P: EncodePacket + ?Sized,
{
    use std::io::Read;

    use flate2::bufread::ZlibEncoder;
    use flate2::Compression;

    let start_len = buf.len();

    pkt.encode_packet(&mut *buf)?;

    let data_len = buf.len() - start_len;

    if data_len > threshold as usize {
        let mut z = ZlibEncoder::new(&buf[start_len..], Compression::new(4));

        scratch.clear();

        let data_len_size = VarInt(data_len as i32).written_size();

        let packet_len = data_len_size + z.read_to_end(scratch)?;

        ensure!(
            packet_len <= MAX_PACKET_SIZE as usize,
            "packet exceeds maximum length"
        );

        drop(z);

        buf.truncate(start_len);

        VarInt(packet_len as i32).encode(&mut *buf)?;
        VarInt(data_len as i32).encode(&mut *buf)?;
        buf.extend_from_slice(scratch);
    } else {
        let data_len_size = 1;
        let packet_len = data_len_size + data_len;

        ensure!(
            packet_len <= MAX_PACKET_SIZE as usize,
            "packet exceeds maximum length"
        );

        let packet_len_size = VarInt(packet_len as i32).written_size();

        let data_prefix_len = packet_len_size + data_len_size;

        buf.put_bytes(0, data_prefix_len);
        buf.copy_within(start_len..start_len + data_len, start_len + data_prefix_len);

        let mut front = &mut buf[start_len..];

        VarInt(packet_len as i32).encode(&mut front)?;
        // Zero for no compression on this packet.
        VarInt(0).encode(front)?;
    }

    Ok(())
}

#[derive(Default)]
pub struct PacketDecoder {
    pub buf: BytesMut,
    pub cursor: usize,
    pub decompress_buf: Vec<u8>,
    pub compression_enabled: bool,
    pub cipher: Option<Cipher>,
}

impl PacketDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_next_packet<'a, P>(&'a mut self) -> Result<Option<P>>
    where
        P: DecodePacket<'a>,
    {
        self.buf.advance(self.cursor);
        self.cursor = 0;

        let mut r = &self.buf[..];

        let packet_len = match VarInt::decode_partial(&mut r) {
            Ok(len) => len,
            Err(VarIntDecodeError::Incomplete) => return Ok(None),
            Err(VarIntDecodeError::TooLarge) => bail!("malformed packet length VarInt"),
        };

        ensure!(
            (0..=MAX_PACKET_SIZE).contains(&packet_len),
            "packet length of {packet_len} is out of bounds"
        );

        if r.len() < packet_len as usize {
            return Ok(None);
        }

        r = &r[..packet_len as usize];

        let packet = if self.compression_enabled {
            use std::io::Read;

            use anyhow::Context;
            use flate2::bufread::ZlibDecoder;

            let data_len = VarInt::decode(&mut r)?.0;

            ensure!(
                (0..MAX_PACKET_SIZE).contains(&data_len),
                "decompressed packet length of {data_len} is out of bounds"
            );

            if data_len != 0 {
                self.decompress_buf.clear();
                self.decompress_buf.reserve_exact(data_len as usize);
                let mut z = ZlibDecoder::new(r).take(data_len as u64);

                z.read_to_end(&mut self.decompress_buf)
                    .context("decompressing packet")?;

                r = &self.decompress_buf;
                P::decode_packet(&mut r)?
            } else {
                P::decode_packet(&mut r)?
            }
        } else {
            P::decode_packet(&mut r)?
        };

        if !r.is_empty() {
            let remaining = r.len();

            debug!("packet after partial decode ({remaining} bytes remain): {packet:?}");

            bail!("packet contents were not read completely ({remaining} bytes remain)");
        }

        let total_packet_len = VarInt(packet_len).written_size() + packet_len as usize;
        self.cursor = total_packet_len;

        Ok(Some(packet))
    }

    /// Repeatedly decodes a packet type until all packets in the decoder are
    /// consumed or an error occurs. The decoded packets are returned in a vec.
    ///
    /// Intended for testing purposes with encryption and compression disabled.
    #[track_caller]
    pub fn collect_into_vec<'a, P>(&'a mut self) -> Result<Vec<P>>
    where
        P: DecodePacket<'a>,
    {
        assert!(
            self.cipher.is_none(),
            "encryption must be disabled to use this method"
        );

        assert!(
            !self.compression_enabled,
            "compression must be disabled to use this method"
        );

        self.buf.advance(self.cursor);
        self.cursor = 0;

        let mut res = vec![];

        loop {
            let mut r = &self.buf[self.cursor..];

            let packet_len = match VarInt::decode_partial(&mut r) {
                Ok(len) => len,
                Err(VarIntDecodeError::Incomplete) => return Ok(res),
                Err(VarIntDecodeError::TooLarge) => bail!("malformed packet length VarInt"),
            };

            ensure!(
                (0..=MAX_PACKET_SIZE).contains(&packet_len),
                "packet length of {packet_len} is out of bounds"
            );

            if r.len() < packet_len as usize {
                return Ok(res);
            }

            r = &r[..packet_len as usize];

            let packet = P::decode_packet(&mut r)?;

            if !r.is_empty() {
                let remaining = r.len();

                debug!("packet after partial decode ({remaining} bytes remain): {packet:?}");

                bail!("packet contents were not read completely ({remaining} bytes remain)");
            }

            let total_packet_len = VarInt(packet_len).written_size() + packet_len as usize;
            self.cursor += total_packet_len;

            res.push(packet);
        }
    }

    pub fn has_next_packet(&self) -> Result<bool> {
        let mut r = &self.buf[self.cursor..];

        match VarInt::decode_partial(&mut r) {
            Ok(packet_len) => {
                ensure!(
                    (0..=MAX_PACKET_SIZE).contains(&packet_len),
                    "packet length of {packet_len} is out of bounds"
                );

                Ok(r.len() >= packet_len as usize)
            }
            Err(VarIntDecodeError::Incomplete) => Ok(false),
            Err(VarIntDecodeError::TooLarge) => bail!("malformed packet length VarInt"),
        }
    }

    pub fn set_compression(&mut self, enabled: bool) {
        self.compression_enabled = enabled;
    }

    pub fn enable_encryption(&mut self, key: &[u8; 16]) {
        assert!(self.cipher.is_none(), "encryption is already enabled");

        let mut cipher = Cipher::new(key.into(), key.into());
        // Don't forget to decrypt the data we already have.
        cipher.decrypt(&mut self.buf[self.cursor..]);
        self.cipher = Some(cipher);
    }

    pub fn queue_bytes(&mut self, mut bytes: BytesMut) {
        #![allow(unused_mut)]

        if let Some(cipher) = &mut self.cipher {
            cipher.decrypt(&mut bytes);
        }

        self.buf.unsplit(bytes);
    }

    pub fn queue_slice(&mut self, bytes: &[u8]) {
        let len = self.buf.len();

        self.buf.extend_from_slice(bytes);

        if let Some(cipher) = &mut self.cipher {
            cipher.decrypt(&mut self.buf[len..]);
        }
    }

    pub fn queued_bytes(&self) -> &[u8] {
        self.buf.as_ref()
    }

    pub fn take_capacity(&mut self) -> BytesMut {
        self.buf.split_off(self.buf.len())
    }

    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional);
    }
}
