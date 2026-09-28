//! A source's cryptography, the same on every executor (LLP 1069.005):
//! what a Rust source calls for what a TypeScript source reaches through
//! `crypto`, under the same read marking.
//!
//! @ref LLP 1069.005 D5 — `digest`, `random_bytes`, `random_uuid`: the
//! right way is the short way (trusted Rust can still call `getrandom`).
//! @ref LLP 1069.005 D2b — under the agent, randomness is a repeatable
//! stream: ChaCha20 (RFC 8439 §2.3) keyed by the launch seed.

use std::sync::{Mutex, OnceLock};

mod ecdsa;
pub use ecdsa::{
    base64url, generate_p256, keep_key, kept_key, sign_es256, unbase64url, EcKey, EcKeyPair, Jwk,
};

use exact_runner::{DataError, Store};

/// The agent's repeatable random stream (LLP 1069.005 D2b): the ChaCha20
/// keystream (RFC 8439 §2.3, 20 rounds) under a key that is the launch seed
/// as eight little-endian bytes followed by 24 zero bytes, with the
/// executor's label (`typescript`, `rust`) as the nonce, zero-padded to 12
/// bytes, and the block counter from 0. Each draw takes the next bytes, in
/// order; none is reused. The browser realms' copy is `agentStream` in
/// `host/web/storage-environment.js`; both are held to the same bytes.
#[derive(Clone, Debug)]
pub struct AgentStream {
    state: [u32; 16],
    block: [u8; 64],
    used: usize,
}

impl AgentStream {
    /// The stream for `seed` (the integer `exactTime().seed` shows) and
    /// the executor `label`, at its start. A label longer than 12 bytes is
    /// cut to 12.
    pub fn new(seed: u64, label: &str) -> AgentStream {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&seed.to_le_bytes());
        let mut nonce = [0u8; 12];
        let label = label.as_bytes();
        let n = label.len().min(12);
        nonce[..n].copy_from_slice(&label[..n]);
        AgentStream::keyed(key, nonce, 0)
    }

    fn keyed(key: [u8; 32], nonce: [u8; 12], counter: u32) -> AgentStream {
        let word = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let mut state = [0u32; 16];
        state[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
        for i in 0..8 {
            state[4 + i] = word(&key[i * 4..]);
        }
        state[12] = counter;
        for i in 0..3 {
            state[13 + i] = word(&nonce[i * 4..]);
        }
        AgentStream {
            state,
            block: [0; 64],
            used: 64,
        }
    }

    /// The seed the agent flag names, or `None` outside the agent: the
    /// process's `EXACT_AGENT=1` and `EXACT_AGENT_SEED` (default 1), which
    /// every native host also reads for `exactTime().seed`. A seed that is
    /// not an integer below 2^53 is 1, as the Apple host takes it.
    pub fn agent_seed() -> Option<u64> {
        if std::env::var("EXACT_AGENT").as_deref() != Ok("1") {
            return None;
        }
        Some(
            std::env::var("EXACT_AGENT_SEED")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .filter(|&s| s < 1 << 53)
                .unwrap_or(1),
        )
    }

    fn next_block(&mut self) {
        let mut x = self.state;
        fn quarter(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
            x[a] = x[a].wrapping_add(x[b]);
            x[d] = (x[d] ^ x[a]).rotate_left(16);
            x[c] = x[c].wrapping_add(x[d]);
            x[b] = (x[b] ^ x[c]).rotate_left(12);
            x[a] = x[a].wrapping_add(x[b]);
            x[d] = (x[d] ^ x[a]).rotate_left(8);
            x[c] = x[c].wrapping_add(x[d]);
            x[b] = (x[b] ^ x[c]).rotate_left(7);
        }
        for _ in 0..10 {
            quarter(&mut x, 0, 4, 8, 12);
            quarter(&mut x, 1, 5, 9, 13);
            quarter(&mut x, 2, 6, 10, 14);
            quarter(&mut x, 3, 7, 11, 15);
            quarter(&mut x, 0, 5, 10, 15);
            quarter(&mut x, 1, 6, 11, 12);
            quarter(&mut x, 2, 7, 8, 13);
            quarter(&mut x, 3, 4, 9, 14);
        }
        for (i, word) in x.iter().enumerate() {
            let out = word.wrapping_add(self.state[i]).to_le_bytes();
            self.block[i * 4..i * 4 + 4].copy_from_slice(&out);
        }
        self.state[12] = self.state[12].wrapping_add(1);
        self.used = 0;
    }

    /// Fill `out` with the stream's next bytes.
    pub fn fill(&mut self, out: &mut [u8]) {
        for byte in out {
            if self.used == 64 {
                self.next_block();
            }
            *byte = self.block[self.used];
            self.used += 1;
        }
    }
}

/// A SHA-2 size for [`digest`] (LLP 1069.005 D1's three).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sha {
    /// SHA-256: 32 bytes.
    Sha256,
    /// SHA-384: 48 bytes.
    Sha384,
    /// SHA-512: 64 bytes.
    Sha512,
}

/// The digest of `data`: pure, so no read and allowed anywhere, as
/// `crypto.subtle.digest` is on every executor (LLP 1069.005 D1, D5).
pub fn digest(sha: Sha, data: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    match sha {
        Sha::Sha256 => sha2::Sha256::digest(data).to_vec(),
        Sha::Sha384 => sha2::Sha384::digest(data).to_vec(),
        Sha::Sha512 => sha2::Sha512::digest(data).to_vec(),
    }
}

/// The most one draw fills, as WebCrypto's `getRandomValues` allows.
pub const MAX_RANDOM_BYTES: usize = 65_536;

/// Fill `out` with secure random bytes, and mark the answer being made as
/// having drawn them (LLP 1069.005 D2, D5): a device read, so bake compiles
/// no value that drew one and the device asks it, as for a TypeScript
/// source's `crypto.getRandomValues`. The OS's entropy; under the agent the
/// repeatable `rust` stream (D2b). More than [`MAX_RANDOM_BYTES`] refuses,
/// with nothing written; an entropy failure clears `out` and refuses.
pub fn random_bytes(store: &Store, out: &mut [u8]) -> Result<(), DataError> {
    if out.len() > MAX_RANDOM_BYTES {
        return Err(DataError::BadArguments(format!(
            "random_bytes: at most {MAX_RANDOM_BYTES} bytes a draw, not {}",
            out.len()
        )));
    }
    let agent = AGENT
        .get_or_init(|| platform::agent_seed().map(|s| Mutex::new(AgentStream::new(s, "rust"))));
    match agent {
        Some(stream) => stream.lock().unwrap_or_else(|e| e.into_inner()).fill(out),
        None => platform::fill(out).map_err(|error| {
            out.fill(0);
            DataError::Unavailable(format!("random_bytes: OS randomness unavailable: {error}"))
        })?,
    }
    store.observe_entropy();
    Ok(())
}

/// A random UUID v4, as `crypto.randomUUID()` returns: a draw of 16 bytes
/// by [`random_bytes`], with its read.
pub fn random_uuid(store: &Store) -> Result<String, DataError> {
    let mut bytes = [0u8; 16];
    random_bytes(store, &mut bytes)?;
    Ok(format_uuid(bytes))
}

/// The process's `rust` stream under the agent, from its start at the first
/// draw; `None` outside the agent.
static AGENT: OnceLock<Option<Mutex<AgentStream>>> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod platform {
    pub fn fill(out: &mut [u8]) -> Result<(), String> {
        getrandom::fill(out).map_err(|e| e.to_string())
    }
    pub fn agent_seed() -> Option<u64> {
        super::AgentStream::agent_seed()
    }
}

/// In the page's wasm, entropy and the drive's seed are the page's (the
/// `exact_data` imports `host/web/glue.js` supplies). A Wasm logic module
/// (LLP 1029.000) has no such import yet: one that draws is refused at load.
#[cfg(target_arch = "wasm32")]
pub(crate) mod platform {
    #[link(wasm_import_module = "exact_data")]
    extern "C" {
        fn random(ptr: *mut u8, len: usize);
        #[link_name = "agent_seed"]
        fn page_seed() -> f64;
    }
    pub fn fill(out: &mut [u8]) -> Result<(), String> {
        // SAFETY: the page writes exactly `len` bytes at `ptr`, which is
        // this owned buffer, and calls nothing back.
        unsafe { random(out.as_mut_ptr(), out.len()) };
        Ok(())
    }
    pub fn agent_seed() -> Option<u64> {
        // SAFETY: a pure read of the page's launch facts.
        let seed = unsafe { page_seed() };
        (seed >= 0.0).then_some(seed as u64)
    }
}

/// A lowercase UUID v4 from 16 bytes: only the version and variant bits
/// are set, as `crypto.randomUUID()` forms one on every executor.
pub fn format_uuid(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut text = String::with_capacity(36);
    for (i, byte) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn the_block_function_is_rfc_8439s() {
        // RFC 8439 §2.3.2's test vector: key 00..1f, nonce 00:00:00:09
        // 00:00:00:4a 00:00:00:00, block count 1.
        let key: [u8; 32] = std::array::from_fn(|i| i as u8);
        let nonce = [0, 0, 0, 9, 0, 0, 0, 0x4a, 0, 0, 0, 0];
        let mut stream = AgentStream::keyed(key, nonce, 1);
        let mut block = [0u8; 64];
        stream.fill(&mut block);
        assert_eq!(
            hex(&block),
            "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4e\
             d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e"
        );
    }

    #[test]
    fn a_seed_and_a_label_name_one_stream_drawn_in_order() {
        let mut a = AgentStream::new(1, "typescript");
        let mut whole = [0u8; 80];
        a.fill(&mut whole);
        // Any split of the draws gives the same bytes, across a block.
        let mut b = AgentStream::new(1, "typescript");
        let (mut first, mut second) = ([0u8; 60], [0u8; 20]);
        b.fill(&mut first);
        b.fill(&mut second);
        assert_eq!(&whole[..60], &first);
        assert_eq!(&whole[60..], &second);
        // The pinned start the web realms and Hermes are held to.
        assert_eq!(
            format_uuid(whole[..16].try_into().unwrap()),
            TYPESCRIPT_SEED_1_FIRST_UUID
        );
        let mut rust = [0u8; 16];
        AgentStream::new(1, "rust").fill(&mut rust);
        let mut other = [0u8; 16];
        AgentStream::new(2, "typescript").fill(&mut other);
        assert!(rust != whole[..16] && other != whole[..16]);
    }

    const TYPESCRIPT_SEED_1_FIRST_UUID: &str = "deb201fb-035c-4c32-bbbf-3da08991a485";

    #[test]
    fn digests_are_the_sha2_vectors_and_no_read() {
        assert_eq!(
            hex(&digest(Sha::Sha256, b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&digest(Sha::Sha384, b"abc")),
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed\
             8086072ba1e7cc2358baeca134c825a7"
        );
        assert_eq!(
            hex(&digest(Sha::Sha512, b"")),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    #[test]
    fn a_draw_is_a_counted_read_and_the_quota_refuses_unmarked() {
        let store = Store::new("", []);
        let (a, b) = (random_uuid(&store).unwrap(), random_uuid(&store).unwrap());
        assert!(a != b && a.len() == 36 && &a[14..15] == "4", "{a} {b}");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"));
        let mut bytes = [0u8; 32];
        random_bytes(&store, &mut bytes).unwrap();
        assert_ne!(bytes, [0; 32]);
        assert_eq!((store.reads(), store.entropy_draws()), (3, 3));
        let mut too_many = vec![7u8; MAX_RANDOM_BYTES + 1];
        assert!(matches!(
            random_bytes(&store, &mut too_many),
            Err(DataError::BadArguments(_))
        ));
        assert!(too_many.iter().all(|&b| b == 7), "nothing written");
        assert_eq!(store.entropy_draws(), 3, "a refusal draws nothing");
    }

    /// Two processes under the agent with seed 1 mint the `rust` stream's
    /// UUID; one outside it does not (D2b). A child process each, since the
    /// flag is the process's and read once.
    #[test]
    fn under_the_agent_two_runs_of_a_rust_source_mint_the_same_uuid() {
        const NAME: &str =
            "crypto::tests::under_the_agent_two_runs_of_a_rust_source_mint_the_same_uuid";
        if std::env::var_os("EXACT_DATA_CRYPTO_CHILD").is_some() {
            println!("uuid={}", random_uuid(&Store::new("", [])).unwrap());
            return;
        }
        let run = |agent: bool| {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
                .env("EXACT_DATA_CRYPTO_CHILD", "1")
                .env("EXACT_AGENT_SEED", "1")
                .env_remove("EXACT_AGENT");
            if agent {
                command.env("EXACT_AGENT", "1");
            }
            let out = String::from_utf8(command.output().unwrap().stdout).unwrap();
            // The harness prints the test's name on the same line.
            out.split("uuid=")
                .nth(1)
                .and_then(|rest| rest.get(..36))
                .unwrap_or_else(|| panic!("no uuid: {out}"))
                .to_string()
        };
        let mut first = [0u8; 16];
        AgentStream::new(1, "rust").fill(&mut first);
        assert_eq!(run(true), format_uuid(first));
        assert_eq!(run(true), format_uuid(first));
        assert_ne!(run(false), format_uuid(first));
    }

    #[test]
    fn uuid_sets_only_version_and_variant_bits() {
        assert_eq!(format_uuid([0; 16]), "00000000-0000-4000-8000-000000000000");
        assert_eq!(
            format_uuid(std::array::from_fn(|i| i as u8)),
            "00010203-0405-4607-8809-0a0b0c0d0e0f"
        );
    }
}
