//! The store as a host runs it: opened from what the bake wrote, selecting
//! at launch, counting the boot, checking after first pixel, and answering
//! the app (LLP 1026 D9/D11/D12; LLP 1030 D7; 1030.000 §4 stage 4).
//!
//! Everything a host would otherwise write twice is here — the Apple and
//! Linux hosts wrap a [`Client`] in their own lock and thread and hand it
//! their transport as a closure; nothing here opens a socket or knows a
//! platform beyond a directory a host names.
//!
//! Dev apparatus, read here so both hosts agree (documented beside
//! `EXACT_DEV_PLAN` in each host):
//!
//! - `EXACT_UPDATE_ORIGIN=<url>` — check this origin instead of the baked
//!   one. The keys stay the baked keys: an origin override changes where a
//!   client looks, never what it trusts.
//! - `EXACT_UPDATE_DIR=<dir>` — the store's directory, instead of the
//!   app's container.
//! - Under `EXACT_AGENT=1` with no `EXACT_UPDATE_DIR`, the store is a fresh
//!   temporary directory: a scripted drive starts from nothing and leaves
//!   nothing selected in a developer's container — the rule the secret
//!   store has under `EXACT_STORE`.

use crate::binary::{Activate, Baked};
use crate::envelope::sha256_hex;
use crate::store::{
    Check, Generation, PreparedSelection, Selection, SelectionRefusal, Status, Store,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// What one check found, as the host logs it and the runner is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The head names what this client already has; nothing was written.
    Current,
    /// A bundle is on disk whole and selected for the next launch.
    Staged {
        /// The entry's name.
        entry: String,
        /// Its stream `seq`.
        seq: u64,
    },
    /// Nothing changed, and this is why.
    Refused(String),
}

impl std::fmt::Display for Outcome {
    /// One line: `current` · `staged seq N` · `refused: …`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Current => write!(f, "current"),
            Outcome::Staged { seq, .. } => write!(f, "staged seq {seq}"),
            Outcome::Refused(why) => write!(f, "refused: {why}"),
        }
    }
}

/// The store, opened for one binary, with what the host has told it about
/// this launch.
#[derive(Debug)]
pub struct Client {
    store: Store,
    baked: Baked,
    head_url: Option<String>,
    assets_root: PathBuf,
    /// Embedded asset digests by name, hashed the first time a check asks.
    digests: HashMap<String, Option<String>>,
    /// Whether the selection at open is what booted (a plan the host was
    /// handed — `EXACT_PLAN`, the dev loop — boots instead and is nobody's
    /// to count).
    booted_selection: bool,
    started: bool,
    succeeded: bool,
}

/// An owned check snapshot: fetching it needs no access to the live client.
#[derive(Debug)]
pub struct CheckRequest {
    store: Store,
    head: String,
    assets_root: PathBuf,
    digests: HashMap<String, Option<String>>,
}

/// A finished download, ready to revalidate against the live client record.
#[derive(Debug)]
pub struct DownloadedCheck {
    dir: PathBuf,
    head: String,
    envelope: Result<crate::Envelope, String>,
    digests: HashMap<String, Option<String>>,
}

impl CheckRequest {
    /// The head URL, for the host transport's response-size bound.
    pub fn head_url(&self) -> &str {
        &self.head
    }

    /// Fetch and verify the head and missing files without holding a host
    /// mutex. Whole entries may be added, but the selection record is untouched.
    pub fn fetch(
        mut self,
        fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> DownloadedCheck {
        let mut embedded = |name: &str| {
            self.digests
                .entry(name.to_string())
                .or_insert_with(|| embedded_asset_digest(&self.assets_root, name))
                .clone()
        };
        let envelope = self.store.download(&self.head, fetch, &mut embedded);
        DownloadedCheck {
            dir: self.store.dir().to_path_buf(),
            head: self.head,
            envelope,
            digests: self.digests,
        }
    }
}

impl Client {
    /// Open the store for the binary whose `compat.json` and plan bytes are
    /// given, under `base` — the platform's data directory — at
    /// `<base>/exact/<app id>/update`, subject to the dev overrides above.
    /// `assets_root` is where the binary's own assets live by name (the
    /// bundle, `EXACT_ASSETS`). Refused when the file names no cohort, or
    /// when it says this binary links no store (`store.L` of `0`).
    pub fn open(
        base: &Path,
        assets_root: &Path,
        compat: &str,
        plan: &[u8],
    ) -> Result<Client, String> {
        let origin = std::env::var("EXACT_UPDATE_ORIGIN")
            .ok()
            .filter(|s| !s.is_empty());
        Client::open_at(base, assets_root, compat, plan, origin.as_deref())
    }

    /// [`Client::open`] with the `EXACT_UPDATE_ORIGIN` override given, not
    /// read from the environment. A development binary checks only this.
    pub fn open_at(
        base: &Path,
        assets_root: &Path,
        compat: &str,
        plan: &[u8],
        origin: Option<&str>,
    ) -> Result<Client, String> {
        let baked = Baked::from_compat(compat, plan)?;
        if !baked.store_linked {
            return Err("this binary links no update store (L = 0)".into());
        }
        let dir = store_dir(base, &baked.embedded.app_id);
        let mut store = Store::open(&dir, baked.embedded.clone())?;
        store.hold_staged(baked.activate == Activate::AppDecides);
        let head_url = baked.head_url(origin);
        Ok(Client {
            store,
            baked,
            head_url,
            assets_root: assets_root.to_path_buf(),
            digests: HashMap::new(),
            booted_selection: false,
            started: false,
            succeeded: false,
        })
    }

    /// Where the check looks, or `None` when neither the manifest nor the
    /// environment names an origin.
    pub fn head_url(&self) -> Option<&str> {
        self.head_url.as_deref()
    }

    /// The store's directory.
    pub fn dir(&self) -> &Path {
        self.store.dir()
    }

    /// The immutable baked identity, used to describe a prepared generation.
    pub fn embedded(&self) -> &crate::Embedded {
        &self.baked.embedded
    }

    /// When a staged bundle applies (the manifest's `deploy.activate`).
    pub fn activate_policy(&self) -> Activate {
        self.baked.activate
    }

    /// What this launch boots (`Store::select`).
    pub fn selection(&self) -> Selection {
        self.store.select()
    }

    /// Prepare the update selected for this launch, or `None` for entry zero.
    /// The plan bytes and complete asset roster are pinned to one generation;
    /// a corrupt plan returns refreshed fallback status instead of booting or
    /// counting that selection.
    pub fn prepare_selected(&mut self) -> Result<Option<PreparedSelection>, SelectionRefusal> {
        let prepared = self.store.prepare_selected();
        self.booted_selection = matches!(prepared, Ok(Some(_)));
        prepared
    }

    /// The selection as one JSON line for a host that reads it across an
    /// ABI: `{"entry":…|null,"seq":N,"plan":"…","assets":"…"}` — the paths
    /// empty for entry zero.
    pub fn selection_json(&self) -> String {
        let s = self.store.select();
        let mut out = String::from("{\"entry\":");
        match &s.entry {
            Some(e) => json_string(e, &mut out),
            None => out.push_str("null"),
        }
        out.push_str(&format!(",\"seq\":{},\"plan\":", s.seq));
        json_string(
            &s.plan
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            &mut out,
        );
        out.push_str(",\"assets\":");
        json_string(
            &s.assets_dir
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            &mut out,
        );
        out.push('}');
        out
    }

    /// The selection is booting: count it (LLP 1026 D11), once per process.
    pub fn boot_started(&mut self) -> Result<(), String> {
        if self.started {
            return Ok(());
        }
        self.started = true;
        self.booted_selection = true;
        self.store.boot_started()
    }

    /// The selected entry's plan was refused at boot and entry zero booted
    /// instead: the failure stands in the record (first pixel will not bless
    /// the entry). Returns the journal line and the refreshed running status
    /// together so a host cannot publish one without the other.
    pub fn entry_refused(&mut self, entry: &str, why: &str) -> (String, Status) {
        self.booted_selection = false;
        self.store.entry_refused();
        (
            format!("exact update: entry {entry} refused at boot: {why}; booted entry zero"),
            self.store.status(),
        )
    }

    /// A prepared generation's asset verification failed before it became
    /// live. Integrity failure removes that boot candidate without a boot
    /// count or blessing and returns already-refreshed fallback status. This
    /// is not a per-session refusal API for a multi-session host.
    pub fn selection_corrupt(
        &mut self,
        generation: &Generation,
        why: impl Into<String>,
    ) -> SelectionRefusal {
        self.booted_selection = false;
        self.store.refuse_prepared(generation, why)
    }

    /// First pixel: the selection that booted is good (LLP 1026 D11), once
    /// per process, and only when the selection is what booted.
    pub fn boot_succeeded(&mut self, generation: &Generation) -> Result<(), String> {
        if self.succeeded || !self.booted_selection || *generation != self.store.generation() {
            return Ok(());
        }
        self.succeeded = true;
        self.store.boot_succeeded(generation)
    }

    /// One check against the head, over the host's `fetch` — every URL the
    /// store needs goes through it, the head first. Never an error: a
    /// refusal is an [`Outcome`] the host logs.
    pub fn check(&mut self, fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>) -> Outcome {
        match self.begin_check() {
            Ok(request) => self.finish_check(request.fetch(fetch)),
            Err(why) => Outcome::Refused(why),
        }
    }

    /// Snapshot the check's immutable inputs under the host mutex, then let
    /// [`CheckRequest::fetch`] run after releasing it.
    pub fn begin_check(&self) -> Result<CheckRequest, String> {
        let head = self.head_url.clone().ok_or_else(|| {
            "no origin: the manifest names none and EXACT_UPDATE_ORIGIN is unset".to_string()
        })?;
        Ok(CheckRequest {
            store: self.store.check_snapshot(),
            head,
            assets_root: self.assets_root.clone(),
            digests: self.digests.clone(),
        })
    }

    /// Adopt a downloaded check under the host mutex. Only its result is
    /// applied; boot and activation state changed during the fetch survives.
    pub fn finish_check(&mut self, downloaded: DownloadedCheck) -> Outcome {
        if downloaded.dir != self.dir() || self.head_url.as_deref() != Some(&downloaded.head) {
            return Outcome::Refused("the update client changed during its check".into());
        }
        self.digests.extend(downloaded.digests);
        let envelope = match downloaded.envelope {
            Ok(envelope) => envelope,
            Err(why) => return Outcome::Refused(why),
        };
        let mut embedded = |name: &str| self.digests.get(name).cloned().flatten();
        match self
            .store
            .finish_check(envelope, &downloaded.head, &mut embedded)
        {
            Ok(Check::Current { .. }) => Outcome::Current,
            Ok(Check::Staged { entry, seq, .. }) => Outcome::Staged { entry, seq },
            Err(why) => Outcome::Refused(why),
        }
    }

    /// The staged generation, pinned but not applied.
    pub fn prepare_activation(&self) -> Result<Option<PreparedSelection>, String> {
        self.store.prepare_activation()
    }

    /// Commit the exact candidate accepted by the host. A new generation
    /// gets its own first-pixel acknowledgement, not the previous boot's.
    pub fn commit_activation(&mut self, generation: &Generation) -> Result<(), String> {
        self.store.commit_activation(generation)?;
        self.started = false;
        self.succeeded = false;
        self.booted_selection = true;
        Ok(())
    }

    /// The app generation this process actually runs.
    pub fn generation(&self) -> Generation {
        self.store.generation()
    }

    /// What the app and the agent are told (`Store::status`).
    pub fn status(&self) -> Status {
        self.store.status()
    }
}

/// The store's directory: `EXACT_UPDATE_DIR`, else a fresh temporary
/// directory under `EXACT_AGENT=1`, else `<base>/exact/<app id>/update`.
fn store_dir(base: &Path, app_id: &str) -> PathBuf {
    if let Some(dir) = std::env::var_os("EXACT_UPDATE_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    if std::env::var_os("EXACT_AGENT").is_some() {
        let dir = std::env::temp_dir().join(format!("exact-update-agent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        return dir;
    }
    let id = if app_id.is_empty() { "app" } else { app_id };
    base.join("exact").join(id).join("update")
}

/// The digest of the asset the binary embeds under `name`, or `None`. The
/// name is the envelope's — `assets/mark.png`, `deck/index.html`,
/// `shaders/glass.wgsl` — under the asset root; a `shaders/` name is also
/// tried under `gpu/`, where the source tree keeps them when the root is
/// the app directory rather than a bundle.
fn embedded_asset_digest(root: &Path, name: &str) -> Option<String> {
    if crate::envelope::safe_name(name).is_err() {
        return None;
    }
    let mut candidates = vec![root.join(name)];
    if name.starts_with("shaders/") {
        candidates.push(root.join("gpu").join(name));
    }
    candidates
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .map(|bytes| sha256_hex(&bytes))
}

fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exact-client-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const COMPAT: &str = r#"{"id":"abc","inputs":{"app":"com.exact.t","keys":null,"trust":"development","store":{"L":"A"}},"delivery":{"activate":"next-launch","channel":"prod","origin":"https://o.example"}}"#;
    /// Dev binaries check only a named origin; name the fixture's.
    const ORIGIN: &str = "https://o.example";

    #[test]
    fn a_client_opens_under_the_app_id_and_names_its_head() {
        let base = temp("open");
        let assets = temp("open-assets");
        std::fs::create_dir_all(assets.join("assets")).unwrap();
        std::fs::write(assets.join("assets/mark.png"), b"a mark").unwrap();
        let mut c = Client::open_at(&base, &assets, COMPAT, b"plan", Some(ORIGIN)).unwrap();
        assert_eq!(c.dir(), base.join("exact/com.exact.t/update"));
        assert_eq!(
            c.head_url(),
            Some("https://o.example/.exact/prod/abc/exact.json")
        );
        assert_eq!(c.activate_policy(), Activate::NextLaunch);
        assert!(c.prepare_selected().unwrap().is_none());
        assert!(matches!(c.prepare_selected(), Ok(None)));
        assert_eq!(
            c.selection_json(),
            "{\"entry\":null,\"seq\":0,\"plan\":\"\",\"assets\":\"\"}"
        );
        // The boot marks are once, and only for the selection.
        c.boot_started().unwrap();
        c.boot_started().unwrap();
        c.boot_succeeded(&c.generation()).unwrap();
        assert_eq!(c.status().stream, "embedded");
        assert_eq!(
            embedded_asset_digest(&assets, "assets/mark.png").as_deref(),
            Some(sha256_hex(b"a mark").as_str())
        );
        assert_eq!(embedded_asset_digest(&assets, "assets/none.png"), None);
        assert_eq!(embedded_asset_digest(&assets, "../mark.png"), None);
        // A check with no reachable origin is a refusal, never an error.
        let mut fetch = |url: &str| -> Result<Vec<u8>, String> { Err(format!("down: {url}")) };
        assert_eq!(
            c.check(&mut fetch),
            Outcome::Refused("down: https://o.example/.exact/prod/abc/exact.json".into())
        );
        assert_eq!(
            c.check(&mut fetch).to_string(),
            "refused: down: https://o.example/.exact/prod/abc/exact.json"
        );
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&assets);
    }

    fn head(seq: u64, plan: &[u8]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "exact": 1, "app": {"id": "com.exact.t"},
            "plan": {"url": "./app.plan", "sha256": sha256_hex(plan), "bytes": plan.len()},
            "assets": [], "stream": {"channel": "prod", "compatibilityId": "abc", "seq": seq}
        }))
        .unwrap()
    }

    #[test]
    fn a_download_cannot_overwrite_a_newer_check_or_concurrent_activation() {
        let base = temp("check-race");
        let mut client = Client::open_at(&base, &base, COMPAT, b"embedded", Some(ORIGIN)).unwrap();
        let fetch = |seq, plan: &[u8]| {
            client.begin_check().unwrap().fetch(&mut |url| {
                Ok(if url.ends_with("exact.json") {
                    head(seq, plan)
                } else {
                    plan.to_vec()
                })
            })
        };
        let older = fetch(1, b"older");
        let newer = fetch(2, b"newer");
        assert!(matches!(
            client.finish_check(newer),
            Outcome::Staged { seq: 2, .. }
        ));
        let candidate = client.prepare_activation().unwrap().unwrap();
        assert_eq!(&*candidate.plan, b"newer");
        client.commit_activation(&candidate.generation).unwrap();
        client.boot_started().unwrap();
        client.boot_succeeded(&client.generation()).unwrap();
        let record = std::fs::read(client.dir().join("record.json")).unwrap();
        assert!(
            matches!(client.finish_check(older), Outcome::Refused(why) if why.contains("below the accepted seq 2"))
        );
        assert_eq!(
            std::fs::read(client.dir().join("record.json")).unwrap(),
            record
        );
        assert_eq!(client.status().running_seq, 2);
        assert!(!client.status().staged);
        let mut reopened =
            Client::open_at(&base, &base, COMPAT, b"embedded", Some(ORIGIN)).unwrap();
        assert_eq!(
            reopened.prepare_selected().unwrap().unwrap().plan.as_ref(),
            b"newer"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_stripped_binary_and_a_bad_file_open_no_store() {
        let base = temp("stripped");
        let stripped = COMPAT.replace("\"L\":\"A\"", "\"L\":\"0\"");
        assert!(Client::open_at(&base, &base, &stripped, b"", Some(ORIGIN))
            .unwrap_err()
            .contains("L = 0"));
        assert!(Client::open_at(&base, &base, "{}", b"", Some(ORIGIN)).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn entry_refused_keeps_first_pixel_from_blessing_it() {
        let base = temp("refused");
        let mut c = Client::open_at(&base, &base, COMPAT, b"plan", Some(ORIGIN)).unwrap();
        c.boot_started().unwrap();
        let (line, status) = c.entry_refused("deadbeef", "format 9 is newer than this binary");
        assert!(line.starts_with("exact update: entry deadbeef refused at boot: format 9"));
        assert!(line.ends_with("booted entry zero"));
        assert_eq!(status.entry, None);
        assert_eq!(status.stream, "embedded");
        assert_eq!(status.running_seq, 0);
        c.boot_succeeded(&c.generation()).unwrap();
        assert!(!c.succeeded);
        let _ = std::fs::remove_dir_all(&base);
    }
}
