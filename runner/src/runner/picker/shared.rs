//! A received share (LLP 1133.024 D2, D3): one location, `action` with the
//! shared text as query parameters, and the files under an opaque token
//! each `params.files` member gets. The host copies the files into
//! `app:/tmp/shared/` under names of its own, then sets each token in the
//! runner ([`Runner::set_shared`]), again as more files land; the app reads
//! them with `sharedFiles(entry, name)`. A token not set this launch
//! answers no files, so a written link or a restored screen never shows a
//! second delivery.

use super::{extension, Picked};
use crate::{DataSource, Runner, RunnerError};
use exact_kernel::CommitReceipt;
use exact_plan::Value;

/// A member's name, its files (each with the path it is copied from) and its dropped count.
type MemberFiles = (String, Vec<(std::path::PathBuf, Picked)>, u32);

/// Where every shared file lands.
pub const SHARED: &str = "app:/tmp/shared/";
/// How long a host waits for a share's files before it counts the rest
/// as dropped (D3).
pub const COPY_BOUND_MS: f64 = 5000.0;
/// The largest file a share carries, as an outgoing share refuses one (LLP
/// 1125 D12).
pub const FILE_CAP: u64 = 64 << 20;

/// What `sharedFiles` answers for one token.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SharedFiles {
    /// The files copied so far, in the sender's order.
    pub files: Vec<Picked>,
    /// Files the host left out: a type outside `accept`, a stream it could
    /// not read, one over [`FILE_CAP`], or one still copying at
    /// [`COPY_BOUND_MS`].
    pub dropped: u32,
    /// Files still being copied.
    pub pending: u32,
}

impl SharedFiles {
    /// The `SharedFiles` record, in its declared field order.
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::list(self.files.iter().map(Picked::value).collect()),
            Value::Number(f64::from(self.dropped)),
            Value::Number(f64::from(self.pending)),
        ])
    }
}

/// The token for a launch's `n`th files member: drawn from the launch seed's
/// stream, so a driven launch draws the same ones, and never a path.
pub fn token(seed: f64, n: u32) -> String {
    // SplitMix64 over the seed's bits and the counter.
    let mut z = seed.to_bits() ^ (u64::from(n) + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    const DIGITS: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    (0..12)
        .map(|i| DIGITS[((z >> (i * 5)) & 31) as usize] as char)
        .collect()
}

/// Whether `token` is one [`token`] could draw.
pub fn is_token(token: &str) -> bool {
    token.len() == 12
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
}

/// A fresh `app:/tmp/shared/<seed>-<n>.<ext>`: named from the launch seed
/// and a counter the host keeps, never from the sender (D3).
pub fn shared_path(seed: f64, n: u32, name: &str) -> String {
    format!("{SHARED}{}-{n}.{}", seed as u64, extension(name))
}

/// The app's `share_target`, as the bake hands it to a host: the action and
/// the parameter name of each member it maps.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShareTarget {
    /// The path a share arrives at.
    pub action: String,
    /// The parameter `title` arrives under, when mapped.
    pub title: Option<String>,
    /// The parameter `text` arrives under, when mapped.
    pub text: Option<String>,
    /// The parameter `url` arrives under, when mapped.
    pub url: Option<String>,
    /// Each `params.files` member's name, with what it accepts.
    pub files: Vec<(String, Vec<String>)>,
}

impl ShareTarget {
    /// The manifest's `share_target` member (D1), as `app.json` writes it;
    /// `None` when it has no `action`.
    pub fn from_json(member: &serde_json::Value) -> Option<ShareTarget> {
        let params = member.get("params");
        let name = |key: &str| {
            params
                .and_then(|p| p.get(key))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        };
        let files = params
            .and_then(|p| p.get("files"))
            .and_then(|f| f.as_array())
            .map(|files| {
                files
                    .iter()
                    .filter_map(|f| {
                        let accept = match f.get("accept")? {
                            serde_json::Value::String(one) => vec![one.clone()],
                            serde_json::Value::Array(many) => many
                                .iter()
                                .filter_map(|a| a.as_str().map(str::to_owned))
                                .collect(),
                            _ => Vec::new(),
                        };
                        Some((f.get("name")?.as_str()?.to_owned(), accept))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(ShareTarget {
            action: member.get("action")?.as_str()?.to_owned(),
            title: name("title"),
            text: name("text"),
            url: name("url"),
            files,
        })
    }
}

impl Pending {
    /// A share in its JSON form: the inbox's `share.json` (LLP 1133.024 D4:
    /// `{"v":1, "title", "text", "url", "files": [{"file", "name", "type",
    /// "member"}], "dropped": {member: count}}`, each `file` one name in
    /// `dir`), or the driver's (`files` as `{"path", "name"}`). `None` for a
    /// record of another version, or a `file` that is not one plain name.
    pub fn from_json(record: &serde_json::Value, dir: Option<&std::path::Path>) -> Option<Pending> {
        if dir.is_some() && record.get("v").and_then(|v| v.as_u64()) != Some(1) {
            return None;
        }
        let text = |key: &str| {
            record
                .get(key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let mut files = Vec::new();
        for f in record
            .get("files")
            .and_then(|f| f.as_array())
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let source = match (
                dir,
                f.get("file").and_then(|v| v.as_str()),
                f.get("path").and_then(|v| v.as_str()),
            ) {
                (Some(dir), Some(file), _) => {
                    let plain = !file.is_empty()
                        && !file.contains(['/', '\\'])
                        && file != "."
                        && file != ".."
                        && file != "share.json";
                    if !plain {
                        return None;
                    }
                    dir.join(file)
                }
                (None, _, Some(path)) => std::path::PathBuf::from(path),
                _ => return None,
            };
            let name = f
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    source
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
            files.push(PendingFile {
                source,
                name,
                mime: f.get("type").and_then(|v| v.as_str()).map(str::to_owned),
                member: f.get("member").and_then(|v| v.as_str()).map(str::to_owned),
                size: f.get("size").and_then(|v| v.as_u64()),
            });
        }
        let dropped = record
            .get("dropped")
            .and_then(|d| d.as_object())
            .map(|d| {
                d.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_u64()?.min(10_000) as u32)))
                    .collect()
            })
            .unwrap_or_default();
        Some(Pending {
            title: text("title"),
            text: text("text"),
            url: text("url"),
            files,
            dropped,
        })
    }
}

/// What a share brought, as the host read it before copying anything.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Incoming {
    /// The shared title.
    pub title: Option<String>,
    /// The shared text.
    pub text: Option<String>,
    /// The shared link.
    pub url: Option<String>,
    /// The members that carry files or dropped some, in the target's order.
    pub members: Vec<String>,
}

/// The location a share is delivered as (D2): `action`, then each member
/// shared under its parameter name, a member not shared left out. A link
/// with no `url` member mapped goes to `text`, as Android carries one. Each
/// files member that carried anything gets `tokens(member)`.
pub fn location(
    target: &ShareTarget,
    share: &Incoming,
    mut tokens: impl FnMut(&str) -> String,
) -> String {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let text = match (&target.url, &share.url, &share.text) {
        (None, Some(link), Some(text)) if !text.contains(link.as_str()) => {
            Some(format!("{text}\n{link}"))
        }
        (None, Some(link), None) => Some(link.clone()),
        _ => share.text.clone(),
    };
    for (name, value) in [
        (&target.title, &share.title),
        (&target.text, &text),
        (&target.url, &share.url),
    ] {
        if let (Some(name), Some(value)) = (name, value) {
            pairs.push((name.clone(), value.clone()));
        }
    }
    for (member, _) in &target.files {
        if share.members.contains(member) {
            pairs.push((member.clone(), tokens(member)));
        }
    }
    let query: Vec<String> = pairs
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                exact_route::encode_uri_component(k),
                exact_route::encode_uri_component(v)
            )
        })
        .collect();
    if query.is_empty() {
        target.action.clone()
    } else {
        format!("{}?{}", target.action, query.join("&"))
    }
}

/// The journal's line for a share as it arrives (D9): the members present
/// and each file's type and size, never the text.
pub fn received_line(share: &Incoming, files: &[(String, u64)]) -> String {
    let mut members: Vec<&str> = Vec::new();
    for (name, value) in [
        ("title", &share.title),
        ("text", &share.text),
        ("url", &share.url),
    ] {
        if value.is_some() {
            members.push(name);
        }
    }
    let files: Vec<String> = files
        .iter()
        .map(|(mime, size)| format!("{mime} {size} B"))
        .collect();
    let mut line = format!(
        "share: received {}",
        if members.is_empty() {
            "no text".into()
        } else {
            members.join(", ")
        }
    );
    if !files.is_empty() {
        line.push_str(&format!("; files: {}", files.join(", ")));
    }
    line
}

/// One file a share brought, before the claim copies it.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingFile {
    /// Where the file is now: the inbox, the provider's copy, a driver's fixture.
    pub source: std::path::PathBuf,
    /// The sender's name for it, kept only as `Picked.name`.
    pub name: String,
    /// Its type, when the sender said; else the name's.
    pub mime: Option<String>,
    /// The member it arrived under, when the sender said (Apple's inbox).
    pub member: Option<String>,
    /// Its size, when the host read it from what it opened (Android's
    /// descriptors, whose path the app cannot stat); else the source's.
    pub size: Option<u64>,
}

/// A share as a host found it: text members and files, and what the
/// sender's side already left out, by member.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pending {
    /// The shared title.
    pub title: Option<String>,
    /// The shared text.
    pub text: Option<String>,
    /// The shared link.
    pub url: Option<String>,
    /// The files, in the sender's order.
    pub files: Vec<PendingFile>,
    /// What the sender's side left out, by member.
    pub dropped: std::collections::BTreeMap<String, u32>,
}

/// One share claimed: its location, each token with its files still
/// pending, and the copies left to make.
#[derive(Debug)]
pub struct Claimed {
    /// The location to deliver (D2): the launch location, or a `navigate`.
    pub location: String,
    /// Each files member's token and what it answers now: every file pending.
    pub tokens: Vec<(String, SharedFiles)>,
    /// The copies, by token: `(source, picked)`, the copy's name in
    /// `app:/tmp/shared/` being the end of `picked.path`.
    copies: Vec<(String, Vec<(std::path::PathBuf, Picked)>)>,
    /// The journal's line (D9).
    pub line: String,
}

/// A launch's claims: its seed and the counters its tokens and stored names
/// are drawn from, kept by the router for the launch.
#[derive(Debug, Clone, PartialEq)]
pub struct Claims {
    seed: f64,
    tokens: u32,
    files: u32,
}

impl Claims {
    /// A launch's first claim, drawing from `seed` (the launch seed).
    pub fn new(seed: f64) -> Claims {
        Claims {
            seed,
            tokens: 0,
            files: 0,
        }
    }

    /// Composes `share`'s location for `target` and assigns each file to the
    /// first member whose `accept` takes it, naming it under
    /// `app:/tmp/shared/`. A file no member accepts, or over [`FILE_CAP`],
    /// is dropped. Nothing is copied yet ([`Claimed::copy`]).
    pub fn claim(&mut self, target: &ShareTarget, share: Pending) -> Claimed {
        let mut by_member: Vec<MemberFiles> = target
            .files
            .iter()
            .map(|(name, _)| {
                (
                    name.clone(),
                    Vec::new(),
                    share.dropped.get(name).copied().unwrap_or(0),
                )
            })
            .collect();
        let mut sizes = Vec::new();
        for file in share.files {
            let mime = file
                .mime
                .clone()
                .unwrap_or_else(|| super::mime_for(&file.name).into());
            let size = file
                .size
                .or_else(|| std::fs::metadata(&file.source).map(|m| m.len()).ok());
            let member = by_member
                .iter()
                .position(|(name, _, _)| match &file.member {
                    Some(m) => m == name,
                    None => target
                        .files
                        .iter()
                        .any(|(n, accept)| n == name && super::accepts(accept, &file.name, &mime)),
                });
            let Some(i) = member else {
                if let Some(first) = by_member.first_mut() {
                    first.2 += 1;
                }
                continue;
            };
            let Some(size) = size.filter(|s| *s <= FILE_CAP) else {
                by_member[i].2 += 1;
                continue;
            };
            self.files += 1;
            let path = shared_path(self.seed, self.files, &file.name);
            sizes.push((mime.clone(), size));
            let picked = Picked {
                path,
                name: file.name,
                mime,
                size: size as f64,
                width: None,
                height: None,
                duration: None,
            };
            by_member[i].1.push((file.source, picked));
        }
        let members: Vec<String> = by_member
            .iter()
            .filter(|(_, f, d)| !f.is_empty() || *d > 0)
            .map(|(n, _, _)| n.clone())
            .collect();
        let incoming = Incoming {
            title: share.title,
            text: share.text,
            url: share.url,
            members,
        };
        let mut drawn = Vec::new();
        let location = location(target, &incoming, |member| {
            let t = token(self.seed, self.tokens);
            self.tokens += 1;
            drawn.push((member.to_string(), t.clone()));
            t
        });
        let mut tokens = Vec::new();
        let mut copies = Vec::new();
        for (member, t) in drawn {
            let (_, files, dropped) = by_member
                .iter_mut()
                .find(|(n, _, _)| *n == member)
                .expect("drawn for a member");
            tokens.push((
                t.clone(),
                SharedFiles {
                    files: Vec::new(),
                    dropped: *dropped,
                    pending: files.len() as u32,
                },
            ));
            copies.push((t, std::mem::take(files)));
        }
        Claimed {
            location,
            tokens,
            copies,
            line: received_line(&incoming, &sizes),
        }
    }
}

impl Claimed {
    /// Makes each copy (a rename when `moving` and the two are on one volume,
    /// as the Apple inbox and the app's container are), and answers each
    /// token's files as they now are: those that failed are dropped.
    /// `shared_dir` is the host's directory for `app:/tmp/shared/`.
    pub fn copy(
        &mut self,
        shared_dir: &std::path::Path,
        moving: bool,
    ) -> Vec<(String, SharedFiles)> {
        self.copy_with(shared_dir, moving, |_| None)
    }

    /// [`Claimed::copy`], reading a source from what `opened` answers for
    /// it when it answers one: a file the host already holds open, which
    /// opening its path again would not reach (Android's descriptors).
    pub fn copy_with(
        &mut self,
        shared_dir: &std::path::Path,
        moving: bool,
        opened: impl Fn(&std::path::Path) -> Option<std::fs::File>,
    ) -> Vec<(String, SharedFiles)> {
        let mut out = Vec::new();
        for ((token, files), (_, now)) in self.copies.iter_mut().zip(self.tokens.iter()) {
            let mut answer = SharedFiles {
                files: Vec::new(),
                dropped: now.dropped,
                pending: 0,
            };
            let _ = std::fs::create_dir_all(shared_dir);
            for (source, picked) in files.drain(..) {
                let local = shared_dir.join(picked.path.trim_start_matches(SHARED));
                let done = match opened(&source) {
                    Some(mut from) => std::fs::File::create(&local)
                        .and_then(|mut to| std::io::copy(&mut from, &mut to))
                        .is_ok(),
                    None => {
                        (moving && std::fs::rename(&source, &local).is_ok())
                            || std::fs::copy(&source, &local).is_ok()
                    }
                };
                if done {
                    answer.files.push(picked);
                } else {
                    answer.dropped += 1;
                }
            }
            out.push((token.clone(), answer));
        }
        out
    }
}

impl SharedFiles {
    /// Refuses a path outside `app:/tmp/shared/` or a count past what a
    /// share could carry.
    fn validate(&self) -> Result<(), String> {
        for f in &self.files {
            let rest = f
                .path
                .strip_prefix(SHARED)
                .ok_or_else(|| format!("{} is not under {SHARED}", f.path))?;
            if rest.is_empty() || rest.contains('/') || rest.contains("..") {
                return Err(format!("{} is not one file under {SHARED}", f.path));
            }
        }
        if self.files.len() + self.dropped as usize + self.pending as usize > 10_000 {
            return Err("more files than a share carries".into());
        }
        Ok(())
    }
}

impl<D: DataSource> Runner<D> {
    /// Claims a share that arrives while the app runs (D2, D3) for `target`,
    /// from this launch's claims, and journals what arrived. The host then
    /// sets each of [`Claimed::tokens`], delivers the location, makes the
    /// copies ([`Claimed::copy`]) and sets each token again with what
    /// landed. `None` when the app declares no routes.
    pub fn claim_share(&mut self, target: &ShareTarget, share: Pending) -> Option<Claimed> {
        let seed = self.place().seed;
        let Some(router) = self.router.as_deref() else {
            self.log("share: refused: the app declares no routes, so no route reads a share");
            return None;
        };
        let claimed = router.claims(seed).claim(target, share);
        self.log(claimed.line.clone());
        Some(claimed)
    }

    /// What the host copied of a share so far, under its token (D3): the
    /// readers of `sharedFiles` are evaluated again in one commit. Called
    /// once before the files land (all `pending`) and again as they do; a
    /// cold share sets its tokens before the boot instead
    /// ([`crate::time::Launch::shared`]).
    pub fn set_shared(
        &mut self,
        token: &str,
        files: SharedFiles,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if !is_token(token) {
            self.log(format!("share: refused: {token:?} is not a share token"));
            return Ok(None);
        }
        if let Err(why) = files.validate() {
            self.log(format!("share: refused: {why}"));
            return Ok(None);
        }
        let Some(router) = self.router.as_deref() else {
            self.log("share: refused: the app declares no routes, so no route reads a share");
            return Ok(None);
        };
        let line = format!(
            "share: {} files set, {} pending, {} dropped",
            files.files.len(),
            files.pending,
            files.dropped
        );
        if !router.share(token, files.value()) {
            return Ok(None);
        }
        self.log(line);
        self.commit_again(Vec::new(), "shared").map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_the_seeds_and_never_repeat_in_a_launch() {
        let a: Vec<String> = (0..50).map(|n| token(7.0, n)).collect();
        assert!(a.iter().all(|t| is_token(t)));
        assert_eq!(a, (0..50).map(|n| token(7.0, n)).collect::<Vec<_>>());
        assert_eq!(
            a.iter().collect::<std::collections::BTreeSet<_>>().len(),
            50
        );
        assert_ne!(token(7.0, 0), token(8.0, 0));
    }

    #[test]
    fn a_share_is_one_location_with_its_members_named_by_the_app() {
        let target = ShareTarget {
            action: "/share".into(),
            title: Some("t".into()),
            text: Some("body".into()),
            url: None,
            files: vec![("files".into(), vec!["text/plain".into()])],
        };
        let share = Incoming {
            title: Some("A & B".into()),
            text: Some("hi there".into()),
            url: Some("https://x.test/a?b=c".into()),
            members: vec!["files".into()],
        };
        let at = location(&target, &share, |_| "TOKEN0000000".into());
        assert_eq!(at, "/share?t=A%20%26%20B&body=hi%20there%0Ahttps%3A%2F%2Fx.test%2Fa%3Fb%3Dc&files=TOKEN0000000");
        // Nothing shared under a member leaves it out.
        let bare = location(&target, &Incoming::default(), |_| unreachable!());
        assert_eq!(bare, "/share");
        assert_eq!(
            shared_path(3.0, 2, "../../etc/notes.TXT"),
            "app:/tmp/shared/3-2.txt"
        );
    }

    #[test]
    fn the_journal_says_what_arrived_but_not_the_text() {
        let share = Incoming {
            text: Some("secret words".into()),
            ..Incoming::default()
        };
        let line = received_line(&share, &[("text/plain".into(), 12)]);
        assert_eq!(line, "share: received text; files: text/plain 12 B");
        assert!(!line.contains("secret"));
    }
}
