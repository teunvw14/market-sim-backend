use bytes::{Buf, BytesMut};
use tokio_util::codec::Decoder;

use crate::exchange::CommandBuffer;

/// Decoder for generating frames of Exchange::CommandBuffer encoded with MessagePack from raw bytes.
pub struct MpCommandDecoder {}

// Command buffer maximum number of commands
pub const MAX_CMD_BUF_SIZE: usize = 1024;

// Adapted from the docs at
// https://docs.rs/tokio-util/latest/tokio_util/codec/index.html#example-decoder
impl Decoder for MpCommandDecoder {
    type Item = CommandBuffer;
    type Error = std::io::Error;

    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            // Not enough data to read length marker.
            return Ok(None);
        }
        let mut length_bytes = [0u8; 2];
        length_bytes.copy_from_slice(&src[..2]);
        let length = u16::from_be_bytes(length_bytes) as usize;

        // Reserve more space in the buffer if the whole command hasn't arrived yet
        let bytes_to_receive = 2 + length - src.len();
        if bytes_to_receive > 0 {
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
