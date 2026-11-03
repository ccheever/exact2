# P-256 key generation loops on persistent OS entropy failure

**Status:** Closed
**Severity:** P2
**Systems:** Data cryptography, Native WebCrypto
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

`generate_p256` cannot return its randomness error when the OS entropy
source keeps failing. `Os::fill_bytes` substitutes zeros and records the
error (`data/src/crypto/ecdsa.rs:192-197`), but key generation checks that
error only after `SigningKey::random` returns (`ecdsa.rs:214-219`). The
random constructor uses rejection sampling for a nonzero scalar, so the
zero substitution makes it retry indefinitely. A source calling
`generate_p256`, or native `crypto.subtle.generateKey` through
`js/src/crypto.rs:80`, can occupy its executing thread without reporting
the documented entropy failure.

Reproduce with the unchanged ECDSA source and a failing platform adapter.
The fifth attempted draw deliberately panics to stop the otherwise
unbounded retry:

```rust
mod crypto {
    pub(crate) mod platform {
        use std::sync::atomic::{AtomicUsize, Ordering};
        pub static DRAWS: AtomicUsize = AtomicUsize::new(0);
        pub fn fill(_: &mut [u8]) -> Result<(), String> {
            let draws = DRAWS.fetch_add(1, Ordering::SeqCst) + 1;
            if draws == 5 { panic!("probe stopped repeated entropy attempts"); }
            Err("injected OS entropy failure".into())
        }
    }
    #[path = "/Users/ccheever/projects/exact-release/data/src/crypto/ecdsa.rs"]
    pub mod ecdsa;
}
fn main() {
    let store = exact_runner::Store::new("", []);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
        crypto::ecdsa::generate_p256(&store, false)));
    let draws = crypto::platform::DRAWS.load(std::sync::atomic::Ordering::SeqCst);
    println!("generate_p256 returned={}; entropy attempts={draws}", result.is_ok());
    assert!(result.is_err() && draws == 5);
}
```

The probe is saved as `/tmp/exact-review-p256-entropy.rs`. On the reviewed
checkout, using existing build artifacts:

```sh
rustc --edition=2021 /tmp/exact-review-p256-entropy.rs --crate-name exact_review_p256_entropy -L dependency=target/debug/deps --extern exact_runner=target/debug/deps/libexact_runner-ea99ef2cb22d8243.rlib --extern p256=target/debug/deps/libp256-9b945f04c737cc5f.rlib --extern serde_json=target/debug/deps/libserde_json-96a2b61021fae786.rlib -o /tmp/exact-review-p256-entropy
/tmp/exact-review-p256-entropy
```

Observed: `generate_p256 returned=false; entropy attempts=5`. The panic is
the probe's stop condition; the actual function has no corresponding
limit. `elliptic-curve` 0.13's `NonZeroScalar::random` confirms the
nonzero-scalar rejection loop.

Use a fallible sampling path that returns the entropy error before
submitting fabricated bytes to the constructor. The probe injects an
entropy failure; it does not establish that macOS entropy failed during
ordinary operation, and it does not exercise a native UI call. No duplicate
was found in the current or either related issue corpus, including closed
tickets.

## Resolution

P-256 generation propagates entropy errors before scalar rejection sampling. Only a successful random draw that is not a valid scalar is retried.

## Verification

`key_sampling_returns_entropy_failure_before_retrying` passed in the 115-test data suite. It verifies one draw on entropy failure and a successful retry after an invalid scalar.
