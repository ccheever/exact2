//! The store directory: what this client has, what it boots, and the check
//! that stages the next bundle.
//!
//! @ref LLP 1026 D9 (the store; selection at launch; entry zero) / D11
//! (signing, anti-rollback, crash recovery, assets by digest, the check off the
//! boot path) / LLP 1030 D3a (the stream's own path on the origin) / D5 (the
//! sunset card; check and activate are two acts) / D9 (an unknown record codec
//! selects entry zero and is left alone)
//!
//! The shape on disk:
//!
//! ```text
//! record.json                              { codec, selected, lastGood, failures, stream, bad }
//! entries/<canonical envelope sha256>/exact.json  the head, as it arrived
//! entries/<envelope sha256>/app.plan        verified against the envelope
//! entries/<envelope sha256>/assets/<name>   likewise
//! entries/.tmp-<something>/                 an entry being built; never selected
//! ```
//!
//! Three rules hold the whole thing up:
//!
//! 1. **An entry is whole or absent.** Every file is written under a temporary
//!    name and the directory is renamed into place, so a client that dies
//!    mid-download leaves nothing selectable behind (LLP 1023 D3's atomic swap,
//!    on disk).
//! 2. **Entry zero is never in the store and never deleted.** `selected: null`
//!    means the bundle the binary embeds; it is what a bad update falls back
//!    to, and it is the whole of LLP 1030 D9's promise.
//! 3. **The counter must not share fate with what it recovers.** The failure
//!    count is written before the boot it counts and cleared after first pixel,
//!    so a bundle that never reaches first pixel is demoted at the launch after
//!    next (LLP 1026 D11; LLP 0421 invariant 11).

use crate::envelope::{resolve_url, safe_name, sha256_hex, Card, Envelope, FileCard};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type AssetResult = Result<Arc<[u8]>, String>;
type AssetCache = Arc<Mutex<BTreeMap<String, AssetResult>>>;

/// The trust policy explicitly baked into a host (LLP 1026 D11/D12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// Every head must verify against a baked public key.
    Production,
    /// Unsigned heads are allowed for the local development loop.
    Development,
}

/// What the binary itself carries: entry zero's identity, the cohort it
/// belongs to, and the keys it trusts (LLP 1026 D9/D11; LLP 1030 D3a). The bake
/// writes every field; nothing here is hand-declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embedded {
    /// The app id (LLP 1023 D5). A bundle for another app is refused.
    pub app_id: String,
    /// The cohort this binary is (LLP 1030 D3a). It names the stream's path on
    /// the origin, and a bundle that names another one is refused.
    pub compatibility_id: String,
    /// The `seq` of the bundle the binary embeds: the anti-rollback floor for a
    /// client that has taken no update.
    pub seq: u64,
    /// The channel this binary bakes in (LLP 1030.000 D4): the stream it
    /// checks is `(channel, compatibility id)`, and a head for another
    /// channel — signed for it, replayed here — is refused.
    pub channel: String,
    /// The developer's verification keys, by key id — raw Ed25519 public keys.
    /// Empty never grants unsigned admission. Rotation is a new binary.
    pub verification_keys: Vec<(String, [u8; 32])>,
    /// Whether unsigned development heads are permitted by this artifact.
    pub trust: Trust,
    /// The SHA-256 of the plan the binary embeds — what a binary can actually
    /// know about entry zero (LLP 1026 D9: "the client knows, by digest, what
    /// it already has"). The published head is a document the binary never
    /// carried, so its digest could not say "current"; the plan's can. With
    /// it, a fresh install whose origin still serves the plan it shipped with
    /// — and assets it also embeds, by digest — answers [`Check::Current`] and
    /// downloads nothing; without it, that client stages one copy of what it
    /// already has, once.
    pub embedded_plan_sha256: Option<String>,
    /// Complete baked asset roster by name: digest and byte count. Missing
    /// metadata is unknown, never an empty generation.
    pub embedded_assets: Option<BTreeMap<String, (String, u64)>>,
    /// The canonical envelope this binary embeds, when published.
    pub entry_digest: Option<String>,
}

impl Embedded {
    /// Admit an unsigned head only under an explicit development policy.
    /// Supplied signatures are always verified, including in development.
    pub fn verify(&self, envelope: &Envelope) -> Result<(), String> {
        if self.trust == Trust::Development && envelope.signature.is_none() {
            Ok(())
        } else {
            envelope.verify(&self.verification_keys)
        }
    }
}

/// What to boot (LLP 1026 D9). `plan` is `None` for entry zero — the bundle in
/// the binary — which is also what a client falls back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The plan file to decode, or `None` for the embedded bytes.
    pub plan: Option<PathBuf>,
    /// Where this entry's assets are, or `None` for the embedded ones.
    pub assets_dir: Option<PathBuf>,
    /// The entry's name — its envelope digest — or `None` for entry zero.
    pub entry: Option<String>,
    /// The `seq` this selection carries: the entry's, or the embedded one's.
    pub seq: u64,
}

/// The immutable identity of one prepared bundle generation. It lets a host
/// keep asset reads tied to the plan it prepared and guards boot-time refusal
/// from clearing a newer selection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation {
    /// The signed entry's canonical digest, or `None` for entry zero.
    pub entry: Option<String>,
    /// The stream sequence carried by this generation.
    pub seq: u64,
}

/// A generation-scoped complete asset roster. A name absent here is absent
/// from the generation — callers must not fall through to embedded files.
/// Named assets are read and verified only at their first resolution, then
/// retained as immutable bytes for every clone pinned to this generation.
#[derive(Debug, Clone)]
pub struct AssetSet {
    generation: Generation,
    root: PathBuf,
    cards: Arc<BTreeMap<String, FileCard>>,
    resolved: AssetCache,
}

impl AssetSet {
    fn stored(generation: Generation, root: PathBuf, cards: Vec<FileCard>) -> AssetSet {
        AssetSet {
            generation,
            root,
            cards: Arc::new(
                cards
                    .into_iter()
                    .map(|card| (card.name.clone(), card))
                    .collect(),
            ),
            resolved: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// The generation this roster belongs to.
    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    /// Every asset name in this generation, sorted. This complete roster is
    /// also the tombstone boundary for embedded or predecessor assets.
    pub fn names(&self) -> Vec<String> {
        self.cards.keys().cloned().collect()
    }

    /// Resolve one name. `Ok(None)` means the complete roster omits it;
    /// `Err` means the signed card exists but the stored bytes no longer match.
    /// A result, including a refusal, is cached across clones after one read.
    pub fn resolve(&self, name: &str) -> Result<Option<Arc<[u8]>>, String> {
        safe_name(name)?;
        let Some(card) = self.cards.get(name) else {
            return Ok(None);
        };
        let mut resolved = self.resolved.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(result) = resolved.get(name) {
            return result.clone().map(Some);
        }
        let path = self.root.join(name);
        let result = read_card(&path, card).map(Arc::from);
        resolved.insert(name.to_string(), result.clone());
        result.map(Some)
    }
}

/// A selected update after its mandatory plan read has re-proved the signed
/// card. Its asset set remains lazy and is safe to pin per host session.
#[derive(Debug, Clone)]
pub struct PreparedSelection {
    /// The selected update's generation token.
    pub generation: Generation,
    /// The verified plan bytes to decode.
    pub plan: Arc<[u8]>,
    /// The generation's complete, lazily verified asset roster.
    pub assets: AssetSet,
}

/// Why a selected update could not be prepared at launch. The store has
/// already moved this process and its durable selection to entry zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRefusal {
    /// The entry that failed its signed card.
    pub entry: String,
    /// The integrity or store-write refusal.
    pub reason: String,
    /// Refreshed delivery status for a host cache before it boots entry zero.
    pub status: Box<Status>,
}

impl std::fmt::Display for SelectionRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "entry {} refused at launch: {}", self.entry, self.reason)
    }
}

impl std::error::Error for SelectionRefusal {}

/// A bundle selected for the next launch that is not the one running (LLP 1030
/// D5: check and activate are two acts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    /// The entry's name: its envelope digest.
    pub entry: String,
    /// The stream `seq` it carries.
    pub seq: u64,
    /// Its plan file.
    pub plan: PathBuf,
    /// Its assets.
    pub assets_dir: PathBuf,
}

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// The head is the bundle this client already selected. Nothing was
    /// downloaded and nothing was written.
    Current {
        /// The stream's retirement notice, when it has one (LLP 1030 D5).
        sunset: Option<Card>,
    },
    /// A new bundle is on disk, whole, and selected for the next launch.
    Staged {
        /// The new entry's name: its envelope digest.
        entry: String,
        /// The stream `seq` it carries.
        seq: u64,
        /// The stream's retirement notice, when it has one (LLP 1030 D5).
        sunset: Option<Card>,
    },
}

/// What the app and the agent are told (LLP 1030 D7): the `delivery` resource's
/// content, and what `state.delivery` mirrors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// `"<channel>/<compatibilityId>"` for the entry this process runs, or
    /// `"embedded"` while it runs entry zero. A selection staged for another
    /// launch does not change this generation identity.
    pub stream: String,
    /// The `seq` of what is selected for the next launch.
    pub selected_seq: u64,
    /// The `seq` of the entry this process booted — the embedded one's for
    /// entry zero.
    pub running_seq: u64,
    /// The `seq` of the bundle in the binary.
    pub embedded_seq: u64,
    /// Whether a bundle is staged that the running one is not.
    pub staged: bool,
    /// The accepted stream's retirement notice, when the selected bundle
    /// carries one. It remains advisory while another generation runs.
    pub sunset: Option<Card>,
    /// The entry this process booted, or `None` for entry zero.
    pub entry: Option<String>,
}

/// The store: bundles on disk, the record that selects one, and the check.
#[derive(Debug)]
pub struct Store {
    dir: PathBuf,
    embedded: Embedded,
    record: Record,
    /// A record written by a binary with a newer store codec: this client
    /// selects entry zero and **writes nothing at all** (LLP 1030 D9).
    frozen: bool,
    /// `app-decides` (LLP 1030.000 D4): a checked bundle is held as
    /// `pending` rather than selected, until [`Store::commit_activation`].
    hold: bool,
    /// The entry this process booted, remembered from the selection at open.
    running: Option<String>,
    /// That entry's `seq` (the embedded one's for entry zero).
    running_seq: u64,
    /// The validated view of `record.selected`, or `None` for entry zero.
    view: Option<EntryView>,
    /// A selected entry rejected while opening, retained until the host asks
    /// for the launch candidate so it can publish the fallback note/status.
    launch_refusal: Option<(String, String)>,
}

/// The selected entry, validated: it exists, it parses, and it is this app's
/// and this cohort's.
#[derive(Debug, Clone)]
struct EntryView {
    sha: String,
    seq: u64,
    plan: FileCard,
    assets: Vec<FileCard>,
    sunset: Option<Card>,
}

/// The record, in memory. Its JSON is `{ "codec", "selected", "pending",
/// "lastGood", "failures", "stream": { "channel", "compatibilityId",
/// "seq" }, "bad" }`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    selected: Option<String>,
    /// An entry on disk whole that no launch boots until the app activates
    /// it — the `app-decides` policy (LLP 1030.000 D4): `check` puts it
    /// here instead of `selected`, and `activate` promotes it.
    pending: Option<String>,
    last_good: Option<String>,
    failures: u64,
    compatibility_id: String,
    channel: String,
    /// The highest stream sequence this client has admitted, including an
    /// embedded generation or a head found current while entry zero ran.
    seq: u64,
    digest: Option<String>,
    /// Entries that failed to reach first pixel twice. A head that names one is
    /// refused rather than staged again, so a bad bundle cannot loop a client
    /// through the same download every launch.
    bad: Vec<String>,
}

/// How many bad entries to remember. Older ones fall off: the list exists to
/// break a loop, not to keep history.
const BAD_REMEMBERED: usize = 8;

/// Two failed boots in a row demote a bundle (LLP 1026 D11).
const FAILURES_ALLOWED: u64 = 2;

impl Record {
    fn fresh(embedded: &Embedded) -> Record {
        Record {
            selected: None,
            pending: None,
            last_good: None,
            failures: 0,
            compatibility_id: embedded.compatibility_id.clone(),
            channel: embedded.channel.clone(),
            seq: embedded.seq,
            digest: embedded.entry_digest.clone(),
            bad: Vec::new(),
        }
    }

    fn to_json(&self) -> String {
        let quoted = |v: &Option<String>| match v {
            Some(s) => serde_json::Value::String(s.clone()),
            None => serde_json::Value::Null,
        };
        let value = serde_json::json!({
            "codec": crate::STORE_CODEC,
            "selected": quoted(&self.selected),
            "pending": quoted(&self.pending),
            "lastGood": quoted(&self.last_good),
            "failures": self.failures,
            "stream": { "channel": self.channel, "compatibilityId": self.compatibility_id, "seq": self.seq, "digest": self.digest },
            "bad": self.bad,
        });
        format!("{value}\n")
    }
}

/// What was on disk where the record lives.
enum RecordFile {
    /// No record, or one this client cannot read as JSON at all: start fresh.
    Fresh,
    /// An older codec from this pre-1.0 crate. Its selections are not read;
    /// open replaces it with this binary's clean entry-zero record.
    Obsolete,
    /// A record written by a binary with another store codec (LLP 1030 D9).
    Foreign,
    /// This codec's record.
    Ours(Record),
}

fn read_record(path: &Path) -> RecordFile {
    let Ok(text) = std::fs::read_to_string(path) else {
        return RecordFile::Fresh;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return RecordFile::Fresh;
    };
    let Some(object) = value.as_object() else {
        return RecordFile::Fresh;
    };
    match object.get("codec").and_then(|v| v.as_u64()) {
        Some(codec) if codec == crate::STORE_CODEC => {}
        Some(codec) if codec < crate::STORE_CODEC => return RecordFile::Obsolete,
        _ => return RecordFile::Foreign,
    }
    let stream = object.get("stream").and_then(|v| v.as_object());
    RecordFile::Ours(Record {
        selected: object
            .get("selected")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        pending: object
            .get("pending")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        last_good: object
            .get("lastGood")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        failures: object.get("failures").and_then(|v| v.as_u64()).unwrap_or(0),
        compatibility_id: stream
            .and_then(|s| s.get("compatibilityId"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        channel: stream
            .and_then(|s| s.get("channel"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        seq: stream
            .and_then(|s| s.get("seq"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        digest: stream
            .and_then(|s| s.get("digest"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        bad: object
            .get("bad")
            .and_then(|v| v.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

impl Store {
    /// Open the store at `dir`, creating it, and settle what this launch boots.
    ///
    /// This is the launch boundary and the only place the record is repaired:
    /// a selected entry that is gone, half-written, another app's, or another
    /// cohort's is dropped; a selected entry that failed to reach first pixel
    /// twice in a row is demoted to the last good one and then to entry zero;
    /// a record from another cohort is reset to this one's;
    /// a record from another store codec freezes the store (LLP 1030 D9) and
    /// nothing here writes.
    pub fn open(dir: &Path, embedded: Embedded) -> Result<Store, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
        let entries = dir.join("entries");
        std::fs::create_dir_all(&entries)
            .map_err(|e| format!("cannot make {}: {e}", entries.display()))?;
        let record = Record::fresh(&embedded);
        let mut store = Store {
            dir: dir.to_path_buf(),
            running_seq: embedded.seq,
            embedded,
            record,
            frozen: false,
            hold: false,
            running: None,
            view: None,
            launch_refusal: None,
        };
        let mut rewrite = false;
        match read_record(&store.record_path()) {
            RecordFile::Foreign => {
                store.frozen = true;
                store.record = Record::fresh(&store.embedded);
                return Ok(store);
            }
            RecordFile::Obsolete => {
                store.record = Record::fresh(&store.embedded);
                rewrite = true;
            }
            RecordFile::Fresh => {
                store.record = Record::fresh(&store.embedded);
            }
            RecordFile::Ours(record) => store.record = record,
        }
        // A launch has no other client of this store: a stage or a record write
        // that died left a temporary behind, and nothing points at it.
        sweep(dir);
        sweep(&entries);
        let before = store.record.clone();
        if store.record.compatibility_id != store.embedded.compatibility_id
            || store.record.channel != store.embedded.channel
        {
            // A new binary over an old store: the old cohort's entries stay on
            // disk (their files are still reusable by digest) but none of them
            // is selectable here, so this cohort starts at entry zero.
            store.record = Record::fresh(&store.embedded);
        }
        if let Some(selected) = store.record.selected.clone() {
            match store.read_view(&selected) {
                Ok(view) => store.view = Some(view),
                Err(reason) => {
                    store.launch_refusal = Some((selected.clone(), reason));
                    store.clear_entry_references(&selected);
                }
            }
        }
        if store.record.failures >= FAILURES_ALLOWED {
            if let Some(bad) = store.record.selected.take() {
                store.record.bad.retain(|s| *s != bad);
                store.record.bad.push(bad);
                let extra = store.record.bad.len().saturating_sub(BAD_REMEMBERED);
                store.record.bad.drain(..extra);
                store.record.failures = 0;
                // The last good bundle, unless it is the one just demoted — that
                // way lies a client that falls back onto its own bad bundle.
                store.record.selected = store
                    .record
                    .last_good
                    .clone()
                    .filter(|sha| !store.record.bad.contains(sha));
                store.view = store.validate(store.record.selected.clone());
                if store.view.is_none() {
                    store.record.selected = None;
                }
            }
        }
        let last_good_is_selected = store
            .view
            .as_ref()
            .zip(store.record.last_good.as_ref())
            .is_some_and(|(view, last_good)| view.sha == *last_good);
        if !last_good_is_selected && store.validate(store.record.last_good.clone()).is_none() {
            store.record.last_good = None;
        }
        if store.record.pending == store.record.selected
            || store
                .record
                .pending
                .as_ref()
                .is_some_and(|p| store.record.bad.contains(p))
            || store.validate(store.record.pending.clone()).is_none()
        {
            store.record.pending = None;
        }
        store.running = store.record.selected.clone();
        store.running_seq = store.select().seq;
        if rewrite || store.record != before {
            store.write_record()?;
        }
        Ok(store)
    }

    /// What this launch boots (LLP 1026 D9): the record's selection when its
    /// entry is whole and this cohort's, else entry zero. No network is
    /// touched, ever, on this path — selection is a stat and a read.
    ///
    /// During a session the selection moves when [`Store::check`] stages a
    /// bundle: after that this reports what the **next** launch takes, and
    /// [`Store::staged`] is how the running process learns they differ.
    pub fn select(&self) -> Selection {
        match &self.view {
            Some(view) => {
                let dir = self.entry_dir(&view.sha);
                Selection {
                    plan: Some(dir.join("app.plan")),
                    assets_dir: Some(dir.join("assets")),
                    entry: Some(view.sha.clone()),
                    seq: view.seq,
                }
            }
            None => Selection {
                plan: None,
                assets_dir: None,
                entry: None,
                seq: self.embedded.seq,
            },
        }
    }

    /// Prepare the update generation this process selected at open. The
    /// authenticated envelope was read once during [`Store::open`]; this
    /// mandatory plan read now re-proves its signed byte count and digest.
    /// Assets stay lazy behind the returned complete [`AssetSet`].
    ///
    /// A corrupt candidate is removed from every durable role, is not counted
    /// as a boot failure, and cannot later be blessed by this process. The
    /// refusal includes status after the running generation moved to entry
    /// zero, so a host can refresh its cache before it falls back.
    pub fn prepare_selected(&mut self) -> Result<Option<PreparedSelection>, SelectionRefusal> {
        if let Some((entry, reason)) = self.launch_refusal.clone() {
            return Err(SelectionRefusal {
                entry,
                reason,
                status: Box::new(self.status()),
            });
        }
        let Some(entry) = self.running.clone() else {
            return Ok(None);
        };
        let view = match self.view.as_ref().filter(|view| view.sha == entry).cloned() {
            Some(view) => view,
            None => match self.read_view(&entry) {
                Ok(view) => view,
                Err(reason) => return Err(self.refuse_running(entry, reason)),
            },
        };
        let plan_path = self.entry_dir(&entry).join("app.plan");
        let plan = match read_card(&plan_path, &view.plan) {
            Ok(bytes) => Arc::from(bytes),
            Err(reason) => return Err(self.refuse_running(entry, reason)),
        };
        let generation = Generation {
            entry: Some(entry.clone()),
            seq: view.seq,
        };
        Ok(Some(PreparedSelection {
            generation: generation.clone(),
            plan,
            assets: AssetSet::stored(
                generation,
                self.entry_dir(&entry).join("assets"),
                view.assets,
            ),
        }))
    }

    /// Refuse a prepared generation before it becomes live after an asset
    /// resolution proves its stored bytes corrupt. Only the matching running
    /// generation moves to entry zero; a stale token cannot clear a newer
    /// selection. Any boot count for the corrupt entry is removed, and it
    /// cannot be last-good. A host with multiple live sessions must not use
    /// this as a per-session refusal mechanism.
    pub fn refuse_prepared(
        &mut self,
        generation: &Generation,
        reason: impl Into<String>,
    ) -> SelectionRefusal {
        let reason = reason.into();
        let Some(entry) = generation.entry.clone() else {
            return SelectionRefusal {
                entry: "embedded".to_string(),
                reason: format!("entry zero is not a stored generation: {reason}"),
                status: Box::new(self.status()),
            };
        };
        if self.running.as_deref() != Some(entry.as_str()) || self.running_seq != generation.seq {
            return SelectionRefusal {
                entry,
                reason: format!("the generation is no longer running: {reason}"),
                status: Box::new(self.status()),
            };
        }
        self.refuse_running(entry, reason)
    }

    /// Count this boot before it happens (LLP 1026 D11). A no-op when entry
    /// zero booted — the embedded bundle is the thing being recovered *to*, so
    /// counting its failures would have nothing to fall back on.
    pub fn boot_started(&mut self) -> Result<(), String> {
        if self.frozen || self.running.is_none() {
            return Ok(());
        }
        self.record.failures += 1;
        self.write_record()
    }

    /// First pixel: the bundle that booted is good (LLP 1026 D11). Clears the
    /// counter and makes what is **running** — not what a check has since
    /// staged — the fallback for the next bad update.
    pub fn boot_succeeded(&mut self, generation: &Generation) -> Result<(), String> {
        if self.frozen || *generation != self.generation() {
            return Ok(());
        }
        self.record.failures = 0;
        self.record.last_good.clone_from(&self.running);
        self.write_record()
    }

    /// The selected entry was refused before it could run and this process
    /// booted entry zero instead. The durable failure count and selection stay
    /// intact for crash recovery; only this process's running identity moves
    /// to the embedded generation.
    pub fn entry_refused(&mut self) {
        self.running = None;
        self.running_seq = self.embedded.seq;
    }

    /// Ask the origin for this stream's head, and stage what it names.
    ///
    /// `head_url` is the stream's own path on the origin (LLP 1030 D3a;
    /// 1030.000 D4) — [`head_url`] builds it:
    /// `<origin>/.exact/<channel>/<compatibilityId>/exact.json`, so a static
    /// host serves each cohort of each channel its own manifest with no
    /// negotiation, and the head's file cards resolve against it. `fetch` is
    /// the host's network — this crate opens no socket — and is called once
    /// for the head, then once for each file the store does not already
    /// have. `embedded_asset` says what the binary itself carries: the digest
    /// of the embedded asset by that name, or `None`; it is asked only when
    /// the head's plan is the embedded one, so a host may hash lazily.
    ///
    /// Refused, before anything is written: a head over
    /// [`MAX_ENVELOPE_BYTES`](crate::MAX_ENVELOPE_BYTES), an `exact` major this
    /// binary does not read, another app's id, another cohort's compatibility
    /// id, another channel's stream, a `seq` below what is selected
    /// (anti-rollback), an unsigned or wrongly signed head when the binary
    /// carries keys, a bundle already demoted for failing to boot, a file
    /// whose digest or byte count is not what the head declared, and a url
    /// that is not http or https.
    ///
    /// On success the entry is on disk whole and selected for the **next**
    /// launch; nothing about the running app changes until the host calls
    /// [`Store::commit_activation`] (LLP 1026 D11; LLP 1030 D5: two acts).
    pub fn check(
        &mut self,
        head_url: &str,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
        embedded_asset: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Result<Check, String> {
        let envelope = self.download(head_url, fetch, embedded_asset)?;
        self.finish_check(envelope, head_url, embedded_asset)
    }

    /// The immutable facts a download uses while its host releases the lock.
    /// This private snapshot may write whole entries, never the live record.
    pub(crate) fn check_snapshot(&self) -> Store {
        Store {
            dir: self.dir.clone(),
            embedded: self.embedded.clone(),
            record: self.record.clone(),
            frozen: self.frozen,
            hold: self.hold,
            running: self.running.clone(),
            running_seq: self.running_seq,
            view: self.view.clone(),
            launch_refusal: self.launch_refusal.clone(),
        }
    }

    pub(crate) fn download(
        &self,
        head_url: &str,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
        embedded_asset: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Result<Envelope, String> {
        if self.frozen {
            return Err(
                "the update record was written by a newer binary; this one changes nothing".into(),
            );
        }
        let envelope = Envelope::parse(&fetch(head_url)?)?;
        if !self.admit_check(&envelope, head_url, embedded_asset)? {
            self.write_entry(&envelope, head_url, fetch)?;
        }
        Ok(envelope)
    }

    /// Recheck against the live record after downloading: activation, boot
    /// marks, and a newer accepted head must never be overwritten by a copy
    /// of the record taken before network I/O.
    pub(crate) fn finish_check(
        &mut self,
        envelope: Envelope,
        head_url: &str,
        embedded_asset: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Result<Check, String> {
        if self.admit_check(&envelope, head_url, embedded_asset)? {
            if self.record.seq != envelope.stream.seq
                || self.record.digest.as_ref() != Some(&envelope.digest)
            {
                self.record.seq = envelope.stream.seq;
                self.record.digest = Some(envelope.digest.clone());
                self.write_record()?;
            }
            return Ok(Check::Current {
                sunset: envelope.sunset,
            });
        }
        let view = self
            .validate(Some(envelope.digest.clone()))
            .ok_or_else(|| {
                format!(
                    "entry {} was written but does not validate",
                    envelope.digest
                )
            })?;
        if self.hold {
            // `app-decides`: whole on disk and known, booted by no launch
            // until the app says so.
            self.record.pending = Some(envelope.digest.clone());
        } else {
            self.record.selected = Some(envelope.digest.clone());
            self.record.pending = None;
            self.record.failures = 0;
            self.view = Some(view);
        }
        self.record.seq = envelope.stream.seq;
        self.record.digest = Some(envelope.digest.clone());
        self.write_record()?;
        Ok(Check::Staged {
            entry: envelope.digest,
            seq: envelope.stream.seq,
            sunset: envelope.sunset,
        })
    }

    fn admit_check(
        &self,
        envelope: &Envelope,
        head_url: &str,
        embedded_asset: &mut dyn FnMut(&str) -> Option<String>,
    ) -> Result<bool, String> {
        if self.frozen {
            return Err(
                "the update record was written by a newer binary; this one changes nothing".into(),
            );
        }
        if envelope.app_id != self.embedded.app_id {
            return Err(format!(
                "the head is for {}; this binary is {}",
                envelope.app_id, self.embedded.app_id
            ));
        }
        if let Some(app) = &envelope.stream.app {
            if *app != self.embedded.app_id {
                return Err(format!(
                    "the head's stream is for {app}; this binary is {}",
                    self.embedded.app_id
                ));
            }
        }
        if envelope.stream.compatibility_id != self.embedded.compatibility_id {
            return Err(format!(
                "the head is for cohort {}; this binary is {}",
                envelope.stream.compatibility_id, self.embedded.compatibility_id
            ));
        }
        if envelope.stream.channel != self.embedded.channel {
            return Err(format!(
                "the head is for channel {}; this binary is {}",
                envelope.stream.channel, self.embedded.channel
            ));
        }
        self.embedded.verify(envelope)?;
        // URL admission is part of every authenticated card, including a
        // card whose bytes are already embedded or present in a whole entry.
        // Check the entire roster before Current can advance the observed
        // floor and before either whole-entry or digest reuse can return.
        validate_card_urls(head_url, envelope)?;
        let floor = self
            .embedded
            .seq
            .max(self.select().seq)
            .max(self.record.seq);
        if envelope.stream.seq < floor {
            return Err(format!(
                "the head is seq {}, below the accepted seq {floor}",
                envelope.stream.seq
            ));
        }
        if envelope.stream.seq == floor {
            let accepted = if self.record.seq == floor {
                self.record.digest.as_ref()
            } else {
                None
            }
            .or_else(|| {
                (self.embedded.seq == floor)
                    .then_some(self.embedded.entry_digest.as_ref())
                    .flatten()
            });
            if accepted.is_some_and(|digest| *digest != envelope.digest) {
                return Err(format!("the head is seq {floor} but names another bundle; a used sequence cannot equivocate"));
            }
        }
        let current = match &self.view {
            _ if self.record.pending.as_deref() == Some(envelope.digest.as_str()) => true,
            Some(view) => view.sha == envelope.digest,
            // Entry zero is current when the head names the plan the binary
            // embeds and exactly the same complete asset roster — removal
            // and an unknown roster are both updates.
            None => {
                self.embedded.embedded_plan_sha256.as_deref() == Some(envelope.plan.sha256.as_str())
                    && self
                        .embedded
                        .embedded_assets
                        .as_ref()
                        .is_some_and(|assets| {
                            assets.len() == envelope.assets.len()
                                && envelope.assets.iter().all(|a| {
                                    assets.get(&a.name).is_some_and(|(sha, bytes)| {
                                        sha == &a.sha256 && *bytes == a.bytes
                                    })
                                })
                        })
                    && envelope
                        .assets
                        .iter()
                        .all(|a| embedded_asset(&a.name).as_deref() == Some(a.sha256.as_str()))
            }
        };
        if current {
            return Ok(true);
        }
        if self.record.bad.contains(&envelope.digest) {
            return Err(format!(
                "bundle {} failed to reach first pixel twice; it is not staged again",
                envelope.digest
            ));
        }
        if envelope.stream.seq == floor {
            return Err(format!(
                "the head is seq {floor} but names another bundle; a used sequence cannot equivocate"
            ));
        }
        Ok(false)
    }

    /// Hold what a check stages as pending — whole on disk, booted by no
    /// launch — until [`Store::commit_activation`]: the `app-decides` policy (LLP
    /// 1030.000 D4). Off, a staged bundle is selected for the next launch.
    pub fn hold_staged(&mut self, hold: bool) {
        self.hold = hold;
    }

    /// The bundle waiting for this process: held pending, or selected for
    /// the next launch and not the one this process booted (LLP 1030 D5/D7).
    pub fn staged(&self) -> Option<Staged> {
        let view = match self.validate(self.record.pending.clone()) {
            Some(pending) => pending,
            None => {
                let view = self.view.as_ref()?;
                if self.running.as_deref() == Some(view.sha.as_str()) {
                    return None;
                }
                view.clone()
            }
        };
        let dir = self.entry_dir(&view.sha);
        Some(Staged {
            entry: view.sha,
            seq: view.seq,
            plan: dir.join("app.plan"),
            assets_dir: dir.join("assets"),
        })
    }

    /// Verify and pin a staged candidate without changing the record or
    /// running generation. Every clone shares its lazy, immutable asset bytes.
    pub fn prepare_activation(&self) -> Result<Option<PreparedSelection>, String> {
        let Some(staged) = self.staged() else {
            return Ok(None);
        };
        let view = self.read_view(&staged.entry)?;
        let generation = Generation {
            entry: Some(view.sha.clone()),
            seq: view.seq,
        };
        Ok(Some(PreparedSelection {
            generation: generation.clone(),
            plan: Arc::from(read_card(&staged.plan, &view.plan)?),
            assets: AssetSet::stored(generation, staged.assets_dir, view.assets),
        }))
    }

    /// Commit only the generation the host prepared and accepted. A check
    /// may have advanced staging while the host prepared: its candidate then
    /// fails without overwriting that newer record. A write failure likewise
    /// changes neither the in-memory selection nor the running generation.
    pub fn commit_activation(&mut self, generation: &Generation) -> Result<(), String> {
        let staged = self.staged().ok_or("nothing is staged")?;
        if generation.entry.as_deref() != Some(&staged.entry) || generation.seq != staged.seq {
            return Err("the staged generation changed during preparation".into());
        }
        let view = self.read_view(&staged.entry)?;
        let mut record = self.record.clone();
        record.pending = None;
        record.selected = Some(staged.entry.clone());
        record.failures = 0;
        write_atomic(&self.record_path(), record.to_json().as_bytes())?;
        self.record = record;
        self.view = Some(view);
        self.running = Some(staged.entry);
        self.running_seq = staged.seq;
        Ok(())
    }

    /// Identity of the app generation running in this process.
    pub fn generation(&self) -> Generation {
        Generation {
            entry: self.running.clone(),
            seq: self.running_seq,
        }
    }

    /// What the app and the agent are told (LLP 1030 D7).
    pub fn status(&self) -> Status {
        Status {
            // `view` is the durable selection for a future launch and may
            // move after this process booted. The stream, like `entry` and
            // `running_seq`, describes the generation actually running.
            stream: if self.running.is_some() {
                format!(
                    "{}/{}",
                    self.embedded.channel, self.embedded.compatibility_id
                )
            } else {
                "embedded".to_string()
            },
            selected_seq: self.select().seq,
            running_seq: self.running_seq,
            embedded_seq: self.embedded.seq,
            staged: self.staged().is_some(),
            // Sunset is an advisory for the accepted delivery stream, not
            // generation identity. Keep it visible while an older or
            // embedded generation runs, including after a boot refusal.
            sunset: self.view.as_ref().and_then(|v| v.sunset.clone()),
            entry: self.running.clone(),
        }
    }

    /// The store's directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// What the binary carries, as this store was opened with.
    pub fn embedded(&self) -> &Embedded {
        &self.embedded
    }

    /// Whether an unknown store codec froze this store (LLP 1030 D9): the
    /// client boots entry zero and writes nothing.
    pub fn frozen(&self) -> bool {
        self.frozen
    }

    fn record_path(&self) -> PathBuf {
        self.dir.join("record.json")
    }

    fn entries_dir(&self) -> PathBuf {
        self.dir.join("entries")
    }

    fn entry_dir(&self, sha: &str) -> PathBuf {
        self.entries_dir().join(sha)
    }

    /// The envelope of an entry that is whole, this app's, and this cohort's.
    fn validate(&self, sha: Option<String>) -> Option<EntryView> {
        let sha = sha?;
        self.read_view(&sha).ok()
    }

    /// Read an entry's authenticated metadata without reading payload bytes.
    /// Launch stats and reads this envelope once; the mandatory plan read
    /// verifies its card, and each asset read verifies its own lazily.
    fn read_view(&self, sha: &str) -> Result<EntryView, String> {
        let dir = self.entry_dir(sha);
        let plan_path = dir.join("app.plan");
        if !regular_file(&plan_path) {
            return Err(format!(
                "{} is not a regular plan file",
                plan_path.display()
            ));
        }
        let envelope_path = dir.join("exact.json");
        if !regular_file(&envelope_path) {
            return Err(format!(
                "{} is not a regular envelope file",
                envelope_path.display()
            ));
        }
        let raw = std::fs::read(&envelope_path)
            .map_err(|e| format!("cannot read {}: {e}", envelope_path.display()))?;
        let envelope = Envelope::parse(&raw)?;
        if envelope.digest != sha
            || envelope.app_id != self.embedded.app_id
            || envelope.stream.compatibility_id != self.embedded.compatibility_id
            || envelope.stream.channel != self.embedded.channel
        {
            return Err(format!("entry {sha} does not belong to this update stream"));
        }
        if envelope
            .stream
            .app
            .as_ref()
            .is_some_and(|app| *app != self.embedded.app_id)
        {
            return Err(format!("entry {sha} names another app in its stream"));
        }
        if envelope.stream.seq < self.embedded.seq {
            return Err(format!(
                "entry {sha} is seq {}, below embedded seq {}",
                envelope.stream.seq, self.embedded.seq
            ));
        }
        self.embedded.verify(&envelope)?;
        Ok(EntryView {
            sha: sha.to_string(),
            seq: envelope.stream.seq,
            plan: envelope.plan,
            assets: envelope.assets,
            sunset: envelope.sunset,
        })
    }

    /// Remove a corrupt entry from every role that could select or bless it.
    /// The accepted sequence remains an anti-rollback floor.
    fn clear_entry_references(&mut self, entry: &str) {
        if self.record.selected.as_deref() == Some(entry) {
            self.record.selected = None;
            self.record.failures = 0;
        }
        if self.record.pending.as_deref() == Some(entry) {
            self.record.pending = None;
        }
        if self.record.last_good.as_deref() == Some(entry) {
            self.record.last_good = None;
        }
        if self.view.as_ref().is_some_and(|view| view.sha == entry) {
            self.view = None;
        }
    }

    fn refuse_running(&mut self, entry: String, mut reason: String) -> SelectionRefusal {
        self.clear_entry_references(&entry);
        self.running = None;
        self.running_seq = self.embedded.seq;
        if let Err(write) = self.write_record() {
            reason.push_str(&format!("; could not persist the fallback: {write}"));
        }
        self.launch_refusal = Some((entry.clone(), reason.clone()));
        SelectionRefusal {
            entry,
            reason,
            status: Box::new(self.status()),
        }
    }

    fn write_record(&self) -> Result<(), String> {
        if self.frozen {
            return Ok(());
        }
        write_atomic(&self.record_path(), self.record.to_json().as_bytes())
    }

    /// Build one entry under a temporary name and rename it into place. Whole
    /// or absent: any failure removes the temporary directory and leaves the
    /// store exactly as it was.
    fn write_entry(
        &self,
        envelope: &Envelope,
        head_url: &str,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        let target = self.entry_dir(&envelope.digest);
        if let Some(view) = self.validate(Some(envelope.digest.clone())) {
            if self.validate_contents(&view).is_ok() {
                return Ok(()); // already here, whole: the record just selects it
            }
        }
        if target.exists() {
            std::fs::remove_dir_all(&target)
                .map_err(|e| format!("cannot clear {}: {e}", target.display()))?;
        }
        let tmp = self.entries_dir().join(temporary_name());
        let filled = self.fill(&tmp, envelope, head_url, fetch);
        if filled.is_err() {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        filled?;
        std::fs::rename(&tmp, &target).map_err(|e| {
            let _ = std::fs::remove_dir_all(&tmp);
            format!("cannot put {} in place: {e}", target.display())
        })
    }

    fn validate_contents(&self, view: &EntryView) -> Result<(), String> {
        let dir = self.entry_dir(&view.sha);
        read_card(&dir.join("app.plan"), &view.plan)?;
        for asset in &view.assets {
            read_card(&dir.join("assets").join(&asset.name), asset)?;
        }
        Ok(())
    }

    fn fill(
        &self,
        tmp: &Path,
        envelope: &Envelope,
        head_url: &str,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        std::fs::create_dir_all(tmp).map_err(|e| format!("cannot make {}: {e}", tmp.display()))?;
        let plan = self.obtain(&envelope.plan, head_url, fetch)?;
        write_file(&tmp.join("app.plan"), &plan)?;
        for asset in &envelope.assets {
            safe_name(&asset.name)?;
            let bytes = self.obtain(asset, head_url, fetch)?;
            write_file(&tmp.join("assets").join(&asset.name), &bytes)?;
        }
        write_file(&tmp.join("exact.json"), &envelope.raw)
    }

    /// One file: reused from an entry that already has those bytes, else
    /// fetched. Either way the digest and the byte count are what the head
    /// declared, or nothing is written (LLP 1026 D11, assets by digest).
    fn obtain(
        &self,
        card: &crate::envelope::FileCard,
        head_url: &str,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        let url = resolve_url(head_url, &card.url)?;
        if let Some(bytes) = self.have(&card.sha256) {
            return Ok(bytes);
        }
        let bytes = fetch(&url)?;
        // The closure hands back a whole body, so the declared length is
        // checked here; a host that streams bounds the read by it as it goes.
        if bytes.len() as u64 != card.bytes {
            return Err(format!(
                "{} is {} bytes; the head declared {}",
                card.name,
                bytes.len(),
                card.bytes
            ));
        }
        let digest = sha256_hex(&bytes);
        if digest != card.sha256 {
            return Err(format!(
                "{} hashes to {digest}; the head declared {}",
                card.name, card.sha256
            ));
        }
        Ok(bytes)
    }

    /// Bytes this store already holds under `digest`, from any entry that
    /// declares them — the plan or an asset, under any name. The candidate is
    /// hashed before it is believed, so a damaged file is refetched rather than
    /// copied forward.
    fn have(&self, digest: &str) -> Option<Vec<u8>> {
        let read = std::fs::read_dir(self.entries_dir()).ok()?;
        for entry in read.flatten() {
            let dir = entry.path();
            if entry.file_name().to_string_lossy().starts_with(".tmp-") {
                continue;
            }
            let Ok(raw) = std::fs::read(dir.join("exact.json")) else {
                continue;
            };
            let Ok(envelope) = Envelope::parse(&raw) else {
                continue;
            };
            let mut candidates: Vec<PathBuf> = Vec::new();
            if envelope.plan.sha256 == digest {
                candidates.push(dir.join("app.plan"));
            }
            for asset in &envelope.assets {
                if asset.sha256 == digest && safe_name(&asset.name).is_ok() {
                    candidates.push(dir.join("assets").join(&asset.name));
                }
            }
            for candidate in candidates {
                if let Ok(bytes) = std::fs::read(&candidate) {
                    if sha256_hex(&bytes) == digest {
                        return Some(bytes);
                    }
                }
            }
        }
        None
    }
}

/// The head's URL for a stream (LLP 1030 D3a; 1030.000 D4): the channel's
/// directory under the origin's `.exact/`, then the cohort's, then the
/// manifest — `<origin>/.exact/<channel>/<compatibilityId>/exact.json`.
pub fn head_url(origin: &str, channel: &str, compatibility_id: &str) -> String {
    format!(
        "{}/.exact/{channel}/{compatibility_id}/exact.json",
        origin.trim_end_matches('/')
    )
}

/// Remove every `.tmp-…` left in `dir` by a write that did not finish.
fn sweep(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        if !item.file_name().to_string_lossy().starts_with(".tmp-") {
            continue;
        }
        let path = item.path();
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

fn read_card(path: &Path, card: &FileCard) -> Result<Vec<u8>, String> {
    if !regular_file(path) {
        return Err(format!(
            "{} is not a regular file for {}",
            path.display(),
            card.name
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| format!("cannot read {} for {}: {e}", path.display(), card.name))?;
    if bytes.len() as u64 != card.bytes {
        return Err(format!(
            "{} is {} bytes; the signed card declared {}",
            card.name,
            bytes.len(),
            card.bytes
        ));
    }
    let digest = sha256_hex(&bytes);
    if digest != card.sha256 {
        return Err(format!(
            "{} hashes to {digest}; the signed card declared {}",
            card.name, card.sha256
        ));
    }
    Ok(bytes)
}

fn validate_card_urls(head_url: &str, envelope: &Envelope) -> Result<(), String> {
    for card in std::iter::once(&envelope.plan).chain(envelope.assets.iter()) {
        resolve_url(head_url, &card.url)?;
    }
    Ok(())
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Write a file whole or not at all: a temporary beside it, then a rename over
/// the old one.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no directory", path.display()))?;
    let tmp = parent.join(temporary_name());
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot put {} in place: {e}", path.display())
    })
}

/// A name nothing else in this directory holds: the process, the clock, and a
/// counter, so two threads of one process cannot collide either.
fn temporary_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        ".tmp-{}-{nanos}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
