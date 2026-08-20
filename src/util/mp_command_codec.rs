use bytes::{Buf, BytesMut};
use serde::Serialize;
use tokio_util::codec::{Decoder, Encoder};

use crate::exchange::CommandBuffer;

/// Codec for generating frames of Exchange::CommandBuffer encoded with MessagePack from raw bytes.
pub struct MpCommandCodec {}

impl MpCommandCodec {
    pub fn new() -> Self {
        Self {}
    }
}

// Encoder, only for testing.
impl<Item: Serialize> Encoder<Item> for MpCommandCodec {
    type Error = std::io::Error;
    fn encode(&mut self, item: Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mp_encoded = rmp_serde::to_vec(&item)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if mp_encoded.len() > u16::MAX as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Message too long!",
            ));
        }
        let len_bytes = (mp_encoded.len() as u16).to_be_bytes();
        dst.extend_from_slice(&len_bytes);
        dst.extend_from_slice(&mp_encoded);

        Ok(())
    }
}

// Adapted from the docs at
// https://docs.rs/tokio-util/latest/tokio_util/codec/index.html#example-decoder
impl Decoder for MpCommandCodec {
    type Item = CommandBuffer;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            // Not enough data to read length marker.
            return Ok(None);
        }
        let mut length_bytes = [0u8; 2];
        length_bytes.copy_from_slice(&src[..2]);
        let length = u16::from_be_bytes(length_bytes) as usize;

        // Reserve more space in the buffer if the whole command hasn't arrived yet
        if src.len() < 2 + length {
            let bytes_to_receive = 2 + length - src.len();
            src.reserve(bytes_to_receive);

            // Frame isn't fully available yet, so return Ok(None) as required by spec.
            return Ok(None);
        }

        // Read bytes for next frame
        let command_buf: CommandBuffer = rmp_serde::from_slice(&src[2..2 + length])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        src.advance(2 + length);

        Ok(Some(command_buf))
    }
}
