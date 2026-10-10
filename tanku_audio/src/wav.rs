//! Writes music to a sound file, to listen to it without the game.

use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use crate::{CHANNELS, SAMPLE_RATE};

/// Writes frames of music as a WAV file of 16 bits.
pub fn write(path: impl AsRef<Path>, frames: &[f32]) -> io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    file.write_all(&encode(frames))?;
    file.flush()
}

/// The bytes of a WAV file of 16 bits.
pub fn encode(frames: &[f32]) -> Vec<u8> {
    const BYTES: u32 = 2;
    let size = frames.len() as u32 * BYTES;
    let mut bytes = Vec::with_capacity(44 + size as usize);
    bytes.extend(b"RIFF");
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes()); // PCM
    bytes.extend(CHANNELS.to_le_bytes());
    bytes.extend(SAMPLE_RATE.to_le_bytes());
    bytes.extend((SAMPLE_RATE * CHANNELS as u32 * BYTES).to_le_bytes());
    bytes.extend((CHANNELS * BYTES as u16).to_le_bytes());
    bytes.extend((8 * BYTES as u16).to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for &sample in frames {
        let level = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        bytes.extend(level.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_says_what_it_holds() {
        let bytes = encode(&[0.0, 1.0, -1.0, 0.5]);
        assert_eq!(bytes.len(), 44 + 8);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(bytes[4..8], 44u32.to_le_bytes());
        assert_eq!(bytes[22..24], 2u16.to_le_bytes());
        assert_eq!(bytes[24..28], 44_100u32.to_le_bytes());
        assert_eq!(bytes[28..32], 176_400u32.to_le_bytes());
        assert_eq!(bytes[40..44], 8u32.to_le_bytes());
        let (pairs, _) = bytes[44..].as_chunks::<2>();
        let samples: Vec<i16> = pairs.iter().map(|&pair| i16::from_le_bytes(pair)).collect();
        assert_eq!(samples, [0, 32_767, -32_767, 16_384]);
    }
}
