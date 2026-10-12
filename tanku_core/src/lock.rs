//! The password of a hosted game.
//!
//! A game that has a password lets in only those who prove that they know it, and the
//! password itself never goes over the network. The host draws a salt for its game and keeps
//! a key that the password and the salt give, not the password. To a guest who knocks, it
//! sends the salt and a number that it asks of nobody else. The guest answers with a proof
//! that only the key and that number give. A proof answers one challenge: whoever listens to
//! a guest who joins can't join with what they heard.
//!
//! This keeps out those who were not given the password. It hides nothing: what a game sends
//! is not secret. And whoever records a proof can try passwords against it at leisure, which
//! the key only slows down: a short word that others could guess keeps out the honest alone.
//!
//! The key is PBKDF2 with HMAC-SHA-256 (RFC 8018), and a proof is an HMAC-SHA-256 (RFC 2104)
//! under that key. SHA-256 is written here as its standard gives it (FIPS 180-4). The tests
//! hold all three to the examples of their standards.

use serde::{Deserialize, Serialize};

use crate::protocol::lettered;

/// The longest password of a game, in letters.
pub const PASSWORD_LENGTH: usize = 24;
/// How many times the key of a password is worked over. It makes each guess at a password
/// cost that much more to whoever recorded a proof, and each join a few milliseconds.
const ROUNDS: u32 = 10_000;

/// A password as it is kept while it is typed and as it is turned into a key: only letters
/// that the interface draws, no space at its ends, and no more letters than a password has.
/// The host and the guest go by the same rule, so what both typed gives the same key.
pub fn worded(text: &str) -> String {
    let written: String = text.chars().filter(|letter| lettered(*letter)).collect();
    let password: String = written.trim_start().chars().take(PASSWORD_LENGTH).collect();
    password.trim_end().to_string()
}

/// What a host asks of a guest who knocks at a game that has a password: the salt of the
/// game, and a number that it asks of this guest alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Challenge {
    pub salt: u64,
    pub nonce: u64,
}

/// What proves that a guest knows the password of a game.
pub type Proof = [u8; 32];

/// What a host keeps of the password of its game: a key, and never the password.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Lock {
    salt: u64,
    key: [u8; 32],
}

impl Lock {
    /// The lock of a game whose host typed this password, with a salt that the game drew for
    /// itself. A game without a password has none.
    pub fn new(password: &str, salt: u64) -> Option<Self> {
        let password = worded(password);
        (!password.is_empty()).then(|| Self {
            salt,
            key: key(&password, salt),
        })
    }

    /// What a guest is asked, with a number that this guest alone is given.
    pub fn challenge(&self, nonce: u64) -> Challenge {
        Challenge {
            salt: self.salt,
            nonce,
        }
    }

    /// Whether a proof answers the challenge that the guest of this number was given.
    pub fn opens(&self, nonce: u64, guest: u64, proof: &Proof) -> bool {
        let wanted = signed(&self.key, nonce, guest);
        // Every byte is looked at, whatever the first ones are: how long the look takes tells
        // nothing of where a wrong proof goes wrong.
        let differing = wanted.iter().zip(proof).map(|(a, b)| a ^ b);
        differing.fold(0, |all, byte| all | byte) == 0
    }
}

/// The proof that answers a challenge, for the guest of this number whose player typed this
/// password.
pub fn prove(challenge: Challenge, guest: u64, password: &str) -> Proof {
    let key = key(&worded(password), challenge.salt);
    signed(&key, challenge.nonce, guest)
}

/// The key that a password gives in a game of this salt.
fn key(password: &str, salt: u64) -> [u8; 32] {
    let salt = [b"tanku".as_slice(), &salt.to_le_bytes()].concat();
    stretched(password.as_bytes(), &salt, ROUNDS)
}

/// What a key says of a challenge and of the guest who was given it.
fn signed(key: &[u8; 32], nonce: u64, guest: u64) -> Proof {
    let asked = [nonce.to_le_bytes(), guest.to_le_bytes()].concat();
    hmac(key, &asked)
}

/// The first 32 bytes of PBKDF2 with HMAC-SHA-256: a key from a password and a salt, worked
/// over a number of times.
fn stretched(password: &[u8], salt: &[u8], rounds: u32) -> [u8; 32] {
    let first = [salt, &1u32.to_be_bytes()].concat();
    let mut worked = hmac(password, &first);
    let mut key = worked;
    for _ in 1..rounds {
        worked = hmac(password, &worked);
        for (byte, with) in key.iter_mut().zip(worked) {
            *byte ^= with;
        }
    }
    key
}

/// HMAC-SHA-256: what a key says of a message.
fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    // A key that is longer than a block goes by its hash.
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&sha256(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let inner: Vec<u8> = block.iter().map(|byte| byte ^ 0x36).collect();
    let outer: Vec<u8> = block.iter().map(|byte| byte ^ 0x5C).collect();
    let within = sha256(&[inner.as_slice(), message].concat());
    sha256(&[outer.as_slice(), &within].concat())
}

/// The first 32 bits of the fractional parts of the cube roots of the first 64 primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 of a message.
fn sha256(message: &[u8]) -> [u8; 32] {
    // The first 32 bits of the fractional parts of the square roots of the first 8 primes.
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    // The message is followed by one set bit, by zeros up to eight bytes short of a whole
    // block, and by its own length in bits.
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend((message.len() as u64 * 8).to_be_bytes());

    for block in padded.as_chunks::<64>().0 {
        let mut words = [0u32; 64];
        for (word, bytes) in words.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for i in 16..64 {
            let (early, late) = (words[i - 15], words[i - 2]);
            let s0 = early.rotate_right(7) ^ early.rotate_right(18) ^ (early >> 3);
            let s1 = late.rotate_right(17) ^ late.rotate_right(19) ^ (late >> 10);
            words[i] = words[i - 16]
                .wrapping_add(s0)
                .wrapping_add(words[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let first = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[i])
                .wrapping_add(words[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = s0.wrapping_add(majority);
            (h, g, f, e) = (g, f, e, d.wrapping_add(first));
            (d, c, b, a) = (c, b, a, first.wrapping_add(second));
        }
        for (kept, worked) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *kept = kept.wrapping_add(worked);
        }
    }
    let mut hash = [0u8; 32];
    for (bytes, word) in hash.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        *bytes = word.to_be_bytes();
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn sha_256_gives_what_its_standard_says() {
        let examples: [(&[u8], &str); 3] = [
            (
                b"",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            // A message that fills a block but for the room that its length takes.
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ];
        for (message, wanted) in examples {
            assert_eq!(hex(&sha256(message)), wanted);
        }
        // A message of many blocks.
        assert_eq!(
            hex(&sha256(&vec![b'a'; 1_000_000])),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn hmac_gives_what_its_standard_says() {
        // The first, second and sixth examples of RFC 4231.
        assert_eq!(
            hex(&hmac(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            hex(&hmac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // A key that is longer than a block.
        let message = b"Test Using Larger Than Block-Size Key - Hash Key First";
        assert_eq!(
            hex(&hmac(&[0xaa; 131], message)),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn a_key_is_worked_over_as_its_standard_says() {
        // The examples of RFC 7914, and one that every library gives.
        assert_eq!(
            hex(&stretched(b"passwd", b"salt", 1)),
            "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc"
        );
        assert_eq!(
            hex(&stretched(b"password", b"salt", 4096)),
            "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
        );
        assert_eq!(
            hex(&stretched(b"Password", b"NaCl", 80_000)),
            "4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab56"
        );
    }

    #[test]
    fn the_password_opens_the_game_and_nothing_else_does() {
        let lock = Lock::new("open sesame", 7).unwrap();
        let challenge = lock.challenge(1234);
        assert_eq!(
            challenge,
            Challenge {
                salt: 7,
                nonce: 1234
            }
        );
        let (guest, other) = (42, 43);
        let proof = prove(challenge, guest, "open sesame");
        assert!(lock.opens(1234, guest, &proof));
        // Another password doesn't, however close.
        for wrong in ["open sesam", "Open sesame", "open  sesame", ""] {
            assert!(!lock.opens(1234, guest, &prove(challenge, guest, wrong)));
        }
        // A proof answers one challenge, of one guest: what was heard opens nothing later.
        assert!(!lock.opens(1235, guest, &proof));
        assert!(!lock.opens(1234, other, &proof));
        // Nor does it open another game with the same password.
        let elsewhere = Lock::new("open sesame", 8).unwrap();
        assert!(!elsewhere.opens(1234, guest, &proof));
        // A proof that is off by one bit, wherever, is no proof.
        for byte in 0..32 {
            let mut near = proof;
            near[byte] ^= 1;
            assert!(!lock.opens(1234, guest, &near), "byte {byte}");
        }
    }

    #[test]
    fn a_password_goes_by_what_was_typed_and_shows() {
        assert_eq!(worded("  open sesame \n"), "open sesame");
        assert_eq!(worded("s\u{e9}same\u{1F600}"), "s\u{e9}same");
        assert_eq!(
            worded(&"p".repeat(PASSWORD_LENGTH + 9)).len(),
            PASSWORD_LENGTH
        );
        // The host and the guest clean a password alike, so both get the same key.
        let lock = Lock::new(" open sesame ", 7).unwrap();
        let proof = prove(lock.challenge(1), 2, "open sesame\t");
        assert!(lock.opens(1, 2, &proof));
        // A game whose host typed nothing has no password.
        assert!(Lock::new("", 7).is_none());
        assert!(Lock::new("  \t", 7).is_none());
    }

    #[test]
    fn a_host_keeps_no_password() {
        // What is kept of two passwords differs, and holds neither of them.
        let (one, two) = (Lock::new("sesame", 7), Lock::new("sesamf", 7));
        assert!(one != two);
        let kept = one.unwrap().key;
        let password = b"sesame";
        assert!(!kept.windows(password.len()).any(|part| part == password));
        // The same password is kept otherwise in another game.
        assert!(Lock::new("sesame", 8) != one);
    }
}
