//! The update store: the bundles an installed app has, the one it boots, and
//! the check that stages the next one.
//!
//! @ref LLP 1026 D9 (one bake, two outputs; the store; selection at launch) /
//! D10 (native by digest identity) / D11 (signing, anti-rollback, crash
//! recovery, assets by digest, the check off the boot path) / D12 (one
//! mechanism, two policies)
//! @ref LLP 1030 D3a (the compatibility id; a stream is `(app, channel,
//! compatibility id)` and the signed head binds it) / D5 (the sunset card) /
//! D9 (an unknown record codec selects entry zero)
//! @ref LLP 1030.000 §4 stage 4 (bundle publishing)
//!
//! A **bundle** is the static envelope of LLP 1023 D2 — `exact.json` — plus
//! the files it names by digest: `app.plan` and the assets. The **store** is a
//! directory in the app's container holding the bundles this client has:
//!
//! ```text
//! <dir>/record.json                       the selected bundle, the last good one, the failures
//! <dir>/entries/<canonical envelope sha256>/exact.json
//! <dir>/entries/<canonical envelope sha256>/app.plan
//! <dir>/entries/<canonical envelope sha256>/assets/<name>
//! ```
//!
//! Every entry is whole or absent: files land in `entries/.tmp-…` and the
//! directory is renamed into place, the atomic-swap rule of LLP 1023 D3 on
//! disk. **Entry zero** — the bundle the binary embeds — is never in the
//! store; `selected: null` means it, and it is what a client falls back to.
//!
//! What this crate does and does not do:
//!
//! - It uses `std::fs` for the store directory and **takes the network as a
//!   closure** (`&mut dyn FnMut(&str) -> Result<Vec<u8>, String>`), so a host
//!   runs [`Store::check`] on its own executor thread and this crate opens no
//!   socket, links no TLS, and knows no platform.
//! - It never boots anything: [`Store::prepare_selected`] and
//!   [`Store::prepare_activation`] pin verified bytes; the host accepts
//!   its whole app before [`Store::commit_activation`] changes the record.
//! - The check is off the boot path (LLP 1026 D11): selection is a stat and a
//!   read; nothing here touches the network until the host asks.
//!
//! - [`envelope`] — the envelope, its canonical bytes, and the signature.
//! - [`store`] — the store directory, selection, the crash counter, the check.
//! - [`binary`] — what the bake wrote into the binary (`compat.json`), read
//!   back into what the store is opened with and where it checks.
//! - [`client`] — the store as a host runs it: open from the baked facts,
//!   select, the boot marks, one check as an outcome line, activate.

#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod binary;
pub mod client;
pub mod envelope;
pub mod store;

pub use binary::{Activate, Baked};
pub use client::{Client, Outcome};
pub use envelope::{
    canonical_bytes, check_asset_names, sha256_hex, Card, Envelope, FileCard, StreamCard,
};
pub use store::{
    head_url, AssetSet, Check, Embedded, Generation, PreparedSelection, Selection,
    SelectionRefusal, Staged, Status, Store, Trust,
};

/// The store's record codec (LLP 1030 D1, D9), the one declaration: the bake
/// hashes it into the compatibility id. The first. A record whose `codec` major is not this one is another binary's
/// and is left alone — the client selects entry zero and writes nothing.
pub const STORE_CODEC: u64 = 2;

/// The envelope's major version (LLP 1023 D2): unknown majors are refused,
/// unknown *fields* are ignored.
pub const ENVELOPE_MAJOR: u64 = 1;

/// The most an envelope may weigh, in bytes (LLP 1026 D11: the head is a
/// pointer card, not a payload). A bigger body is refused before it is parsed.
pub const MAX_ENVELOPE_BYTES: usize = 64 * 1024;
