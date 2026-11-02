//! The update store: the bundles an installed app has, the one it boots, and
//! the check that stages the next one.
//!
//! One bake, two outputs, selected at launch; native code by digest identity;
//! signing, anti-rollback, crash recovery, assets by digest, and the check off
//! the boot path; one mechanism, two policies. A stream is `(app, channel,
//! compatibility id)` and the signed head binds it; a stream retires through
//! its sunset card; an unknown record codec selects entry zero.
//!
//! A **bundle** is the static envelope — `exact.json` — plus
//! the files it names by digest: `app.plan` and the assets. The **store** is a
//! directory in the app's container holding the bundles this client has:
//!
//! ```text
//! <dir>/record.json                       the selected bundle, the last good one, the failures
//! <dir>/blobs/<sha256>                    each file's bytes, once
//! <dir>/entries/<canonical envelope sha256>/exact.json
//! <dir>/entries/<canonical envelope sha256>/app.plan          linked to its blob
//! <dir>/entries/<canonical envelope sha256>/assets/<name>     likewise
//! ```
//!
//! Every entry is whole or absent: files land in `entries/.tmp-…` and the
//! directory is renamed into place, an atomic swap on disk. **Entry zero** —
//! the bundle the binary embeds — is never in the
//! store; `selected: null` means it, and it is what a client falls back to.
//!
//! What this crate does and does not do:
//!
//! - It uses `std::fs` for the store directory and **takes the network as a
//!   closure** (`&mut dyn FnMut(&str) -> Result<Vec<u8>, String>`), so a host
//!   runs [`Store::check`] on its own executor thread and this crate opens no
//!   socket and links no TLS. Windows durable storage is unqualified;
//!   [`Store::open`] refuses there before touching any store files.
//!   Pure envelope, signature and baked-binary operations remain available.
//! - It never boots anything: [`Store::prepare_selected`] and
//!   [`Store::prepare_activation`] pin verified bytes; the host accepts
//!   its whole app before [`Store::commit_activation`] changes the record.
//! - The check is off the boot path: selection is a stat and a
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
pub mod gpu;
pub mod store;
pub use gpu::code_digest as gpu_code_digest;

pub use binary::{Activate, Baked};
pub use client::{Client, Outcome};
pub use envelope::{
    canonical_bytes, check_asset_names, sha256_hex, Card, Envelope, FileCard, StreamCard,
};
pub use store::{
    head_url, AssetSet, Check, Embedded, Generation, PreparedSelection, Selection,
    SelectionRefusal, Staged, Status, Store, Trust,
};

/// The store's record codec, the one declaration: the bake
/// hashes it into the compatibility id. The first. A record whose `codec` major is not this one is another binary's
/// and is left alone — the client selects entry zero and writes nothing.
pub const STORE_CODEC: u64 = 2;

/// The envelope's major version: unknown majors are refused,
/// unknown *fields* are ignored.
pub const ENVELOPE_MAJOR: u64 = 1;

/// The most an envelope may weigh, in bytes (the head is a pointer card, not
/// a payload). A bigger body is refused before it is parsed.
pub const MAX_ENVELOPE_BYTES: usize = 64 * 1024;
