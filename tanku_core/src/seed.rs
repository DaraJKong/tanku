//! Seeds: the short texts that identify a level.

/// The game version. Seeds and records carry it, because a new version may change generation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTVWXYZ23456789";
const RANDOM_SEED_LENGTH: usize = 8;

/// The number a seed text stands for. Case and surrounding spaces don't matter.
pub fn seed_value(text: &str) -> u64 {
    // FNV-1a.
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    for byte in text.trim().bytes() {
        hash ^= byte.to_ascii_uppercase() as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// A random seed text, built from whatever entropy the caller has.
pub fn random_seed_text(mut entropy: u64) -> String {
    let mut text = String::with_capacity(RANDOM_SEED_LENGTH);
    for _ in 0..RANDOM_SEED_LENGTH {
        entropy = entropy
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let index = (entropy >> 33) % ALPHABET.len() as u64;
        text.push(ALPHABET[index as usize] as char);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_values_ignore_case_and_spaces() {
        assert_eq!(seed_value("tanku"), seed_value("  TANKU "));
        assert_ne!(seed_value("TANKU"), seed_value("TANKV"));
    }

    #[test]
    fn seed_values_are_stable() {
        assert_eq!(seed_value("TANKU"), 4_887_005_400_034_090_100);
    }

    #[test]
    fn random_seeds_use_the_alphabet() {
        let text = random_seed_text(123_456_789);
        assert_eq!(text.len(), RANDOM_SEED_LENGTH);
        assert!(text.bytes().all(|byte| ALPHABET.contains(&byte)));
        assert_ne!(text, random_seed_text(123_456_790));
    }
}
