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
//! entries/<envelope sha256>/exact.json      the head, as it arrived
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

use crate::envelope::{resolve_url, safe_name, sha256_hex, Card, Envelope};
use std::path::{Path, PathBuf};

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
    /// The developer's verification keys, by key id — raw Ed25519 public keys.
    /// **Empty means a dev binary**, which admits an unsigned head; a binary
    /// with keys refuses one (LLP 1026 D11). Rotation is a new binary.
    pub verification_keys: Vec<(String, [u8; 32])>,
    /// The SHA-256 of the embedded bundle's own `exact.json`, when the bake
    /// wrote it beside the archive (LLP 1026 D9: "the client knows, by digest,
    /// what it already has"). With it, a fresh install whose origin still
    /// serves the bundle it shipped with answers [`Check::Current`] and
    /// downloads nothing; without it, that client stages one copy of what it
    /// already has, once.
    pub embedded_digest: Option<String>,
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
    /// `"<channel>/<compatibilityId>"`, or `"embedded"` when this client has
    /// taken nothing from a stream.
    pub stream: String,
    /// The `seq` of what is selected for the next launch.
    pub selected_seq: u64,
    /// The `seq` of the bundle in the binary.
    pub embedded_seq: u64,
    /// Whether a bundle is staged that the running one is not.
    pub staged: bool,
    /// The stream's retirement notice, when the selected bundle carries one.
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
    /// The entry this process booted, remembered from the selection at open.
    running: Option<String>,
    /// The validated view of `record.selected`, or `None` for entry zero.
    view: Option<EntryView>,
}

/// The selected entry, validated: it exists, it parses, and it is this app's
/// and this cohort's.
#[derive(Debug, Clone)]
struct EntryView {
    sha: String,
    seq: u64,
    channel: String,
    plan_sha256: String,
    sunset: Option<Card>,
}

/// The record, in memory. Its JSON is `{ "codec", "selected", "lastGood",
/// "failures", "stream": { "compatibilityId", "seq" }, "bad" }`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    selected: Option<String>,
    last_good: Option<String>,
    failures: u64,
    compatibility_id: String,
    seq: u64,
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
    fn fresh(compatibility_id: &str) -> Record {
        Record {
            selected: None,
            last_good: None,
            failures: 0,
            compatibility_id: compatibility_id.to_string(),
            seq: 0,
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
            "lastGood": quoted(&self.last_good),
            "failures": self.failures,
            "stream": { "compatibilityId": self.compatibility_id, "seq": self.seq },
            "bad": self.bad,
        });
        format!("{value}\n")
    }
}

/// What was on disk where the record lives.
enum RecordFile {
    /// No record, or one this client cannot read as JSON at all: start fresh.
    Fresh,
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
        _ => return RecordFile::Foreign,
    }
    let stream = object.get("stream").and_then(|v| v.as_object());
    RecordFile::Ours(Record {
        selected: object
            .get("selected")
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
        seq: stream
            .and_then(|s| s.get("seq"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
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
        let mut store = Store {
            dir: dir.to_path_buf(),
            embedded,
            record: Record::fresh(""),
            frozen: false,
            running: None,
            view: None,
        };
        match read_record(&store.record_path()) {
            RecordFile::Foreign => {
                store.frozen = true;
                store.record = Record::fresh(&store.embedded.compatibility_id);
                return Ok(store);
            }
            RecordFile::Fresh => {
                store.record = Record::fresh(&store.embedded.compatibility_id);
            }
            RecordFile::Ours(record) => store.record = record,
        }
        // A launch has no other client of this store: a stage or a record write
        // that died left a temporary behind, and nothing points at it.
        sweep(dir);
        sweep(&entries);
        let before = store.record.clone();
        if store.record.compatibility_id != store.embedded.compatibility_id {
            // A new binary over an old store: the old cohort's entries stay on
            // disk (their files are still reusable by digest) but none of them
            // is selectable here, so this cohort starts at entry zero.
            store.record = Record::fresh(&store.embedded.compatibility_id);
        }
        store.view = store.validate(store.record.selected.clone());
        if store.view.is_none() {
            store.record.selected = None;
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
        if store.validate(store.record.last_good.clone()).is_none() {
            store.record.last_good = None;
        }
        store.running = store.record.selected.clone();
        if store.record != before {
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
    pub fn boot_succeeded(&mut self) -> Result<(), String> {
        if self.frozen {
            return Ok(());
        }
        self.record.failures = 0;
        self.record.last_good.clone_from(&self.running);
        self.write_record()
    }

    /// Ask the origin for this stream's head, and stage what it names.
    ///
    /// The URL is the stream's own path (LLP 1030 D3a):
    /// `<origin>/.exact/<compatibilityId>/exact.json`, so a static host serves
    /// each cohort its own manifest with no negotiation. `fetch` is the host's
    /// network — this crate opens no socket — and is called once for the head,
    /// then once for each file the store does not already have.
    ///
    /// Refused, before anything is written: a head over
    /// [`MAX_ENVELOPE_BYTES`](crate::MAX_ENVELOPE_BYTES), an `exact` major this
    /// binary does not read, another app's id, another cohort's compatibility
    /// id, a `seq` below what is selected (anti-rollback), an unsigned or
    /// wrongly signed head when the binary carries keys, a bundle already
    /// demoted for failing to boot, a file whose digest or byte count is not
    /// what the head declared, and a url that is not http or https.
    ///
    /// On success the entry is on disk whole and selected for the **next**
    /// launch; nothing about the running app changes until the host calls
    /// [`Store::activate`] (LLP 1026 D11; LLP 1030 D5: two acts).
    pub fn check(
        &mut self,
        origin: &str,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Check, String> {
        if self.frozen {
            return Err(
                "the update record was written by a newer binary; this one changes nothing".into(),
            );
        }
        let head_url = format!(
            "{}/.exact/{}/exact.json",
            origin.trim_end_matches('/'),
            self.embedded.compatibility_id
        );
        let raw = fetch(&head_url)?;
        let envelope = Envelope::parse(&raw)?;
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
        let floor = self.select().seq;
        if envelope.stream.seq < floor {
            return Err(format!(
                "the head is seq {}, below the selected seq {floor}",
                envelope.stream.seq
            ));
        }
        envelope.verify(&self.embedded.verification_keys)?;
        let current = match &self.view {
            Some(view) => view.sha == envelope.digest,
            None => self.embedded.embedded_digest.as_deref() == Some(envelope.digest.as_str()),
        };
        if current {
            if self.record.seq != envelope.stream.seq {
                self.record.seq = envelope.stream.seq;
                self.write_record()?;
            }
            return Ok(Check::Current {
                sunset: envelope.sunset,
            });
        }
        if self.record.bad.contains(&envelope.digest) {
            return Err(format!(
                "bundle {} failed to reach first pixel twice; it is not staged again",
                envelope.digest
            ));
        }
        self.write_entry(&envelope, &head_url, fetch)?;
        let view = self
            .validate(Some(envelope.digest.clone()))
            .ok_or_else(|| {
                format!(
                    "entry {} was written but does not validate",
                    envelope.digest
                )
            })?;
        self.record.selected = Some(envelope.digest.clone());
        self.record.seq = envelope.stream.seq;
        self.record.failures = 0;
        self.view = Some(view);
        self.write_record()?;
        Ok(Check::Staged {
            entry: envelope.digest,
            seq: envelope.stream.seq,
            sunset: envelope.sunset,
        })
    }

    /// The bundle selected for the next launch, when it is not the one this
    /// process booted (LLP 1030 D5/D7).
    pub fn staged(&self) -> Option<Staged> {
        let view = self.view.as_ref()?;
        if self.running.as_deref() == Some(view.sha.as_str()) {
            return None;
        }
        let dir = self.entry_dir(&view.sha);
        Some(Staged {
            entry: view.sha.clone(),
            seq: view.seq,
            plan: dir.join("app.plan"),
            assets_dir: dir.join("assets"),
        })
    }

    /// The staged plan's bytes, for a host that applies it now — the app's own
    /// `delivery.activate` (LLP 1030 D7). The digest is checked once more
    /// against the entry's envelope before the bytes are handed over; from here
    /// the staged entry is the running one.
    pub fn activate(&mut self) -> Option<Vec<u8>> {
        let staged = self.staged()?;
        let plan = std::fs::read(&staged.plan).ok()?;
        let expected = self.view.as_ref().map(|v| v.plan_sha256.clone())?;
        if sha256_hex(&plan) != expected {
            return None;
        }
        self.running = Some(staged.entry);
        Some(plan)
    }

    /// What the app and the agent are told (LLP 1030 D7).
    pub fn status(&self) -> Status {
        Status {
            stream: match &self.view {
                Some(view) => format!("{}/{}", view.channel, self.embedded.compatibility_id),
                None => "embedded".to_string(),
            },
            selected_seq: self.select().seq,
            embedded_seq: self.embedded.seq,
            staged: self.staged().is_some(),
            sunset: self.view.as_ref().and_then(|v| v.sunset.clone()),
            entry: self.running.clone(),
        }
    }

    /// The store's directory.
    pub fn dir(&self) -> &Path {
        &self.dir
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
        let dir = self.entry_dir(&sha);
        if !dir.join("app.plan").is_file() {
            return None;
        }
        let raw = std::fs::read(dir.join("exact.json")).ok()?;
        let envelope = Envelope::parse(&raw).ok()?;
        if envelope.digest != sha
            || envelope.app_id != self.embedded.app_id
            || envelope.stream.compatibility_id != self.embedded.compatibility_id
        {
            return None;
        }
        Some(EntryView {
            sha,
            seq: envelope.stream.seq,
            channel: envelope.stream.channel,
            plan_sha256: envelope.plan.sha256,
            sunset: envelope.sunset,
        })
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
        if self.validate(Some(envelope.digest.clone())).is_some() {
            return Ok(()); // already here, whole: the record just selects it
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
        if let Some(bytes) = self.have(&card.sha256) {
            return Ok(bytes);
        }
        let url = resolve_url(head_url, &card.url)?;
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
