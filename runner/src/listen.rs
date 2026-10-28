//! A local listener (LLP 1133.005): `exactListen("unix:<path>", options)`,
//! a stream whose resource holds the address while it is held (D2).
//!
//! @ref LLP 1133.005 D1 (the events and options) / D3 (bounds) / D4 (`unix:`
//! paths) / D11 (the `net.listen` grant)
//!
//! A listener is a [`crate::Request`] with [`crate::Request::listen`] set: a
//! two-way stream whose messages are its events. `listening` (`data` the
//! address, `id` the bound port), `connection` (`id` the connection),
//! `message` (`id`, `data` one line) and `end` (`id`, `data` the reason) reach
//! the mapper; an end of the stream is its `error`. Events named
//! [`LISTEN_STATUS`] are the host's word on the address alone (`waiting`,
//! `displaced`, `holding`), which `state.streams` and the journal show and no
//! mapper sees. Sends are [`crate::SocketOp::Line`] and
//! [`crate::SocketOp::End`], addressed by connection.

/// The event of a listener's message that only says where its address
/// stands: `data` is `waiting` (another listener of this process holds it,
/// D2), `displaced` (a newer one took it over), `holding` (it holds it
/// again) or `took over` (its `listening` replaced a live socket), and `id`
/// the address. The runner records it and calls no mapper.
pub const LISTEN_STATUS: &str = "exact:listen";

/// The longest `firstLineTimeout`, in ms.
pub const MAX_FIRST_LINE_TIMEOUT_MS: u32 = 600_000;
/// The default and the largest `maxLineBytes`.
pub const DEFAULT_LINE_BYTES: u32 = 65_536;
/// The largest `maxLineBytes`.
pub const MAX_LINE_BYTES: u32 = 1 << 20;
/// The default and the largest `maxConnections`.
pub const DEFAULT_CONNECTIONS: u32 = 16;
/// The largest `maxConnections`.
pub const MAX_CONNECTIONS: u32 = 64;

/// The size of `sockaddr_un`'s `sun_path`: a path takes at most one byte
/// less, for its NUL.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub const SUN_PATH: usize = 108;
/// The size of `sockaddr_un`'s `sun_path`: a path takes at most one byte
/// less, for its NUL.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub const SUN_PATH: usize = 104;

/// A listener's options (D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listen {
    /// Newest wins: publish over a live socket, and take the address from
    /// another listener of this process (D2, D4).
    pub takeover: bool,
    /// A connection whose first complete line has not arrived within this
    /// many ms ends `timeout`; none by default.
    pub first_line_timeout_ms: Option<u32>,
    /// The longest line, in bytes, before its connection ends `overflow`.
    pub max_line_bytes: u32,
    /// Connections open at once; one past it is accepted and closed at once.
    pub max_connections: u32,
}

impl Default for Listen {
    fn default() -> Self {
        Listen {
            takeover: false,
            first_line_timeout_ms: None,
            max_line_bytes: DEFAULT_LINE_BYTES,
            max_connections: DEFAULT_CONNECTIONS,
        }
    }
}

impl Listen {
    /// Why these options are refused, if they are.
    pub fn refusal(&self) -> Option<&'static str> {
        if self
            .first_line_timeout_ms
            .is_some_and(|ms| ms == 0 || ms > MAX_FIRST_LINE_TIMEOUT_MS)
        {
            return Some("exactListen: firstLineTimeout is 1 to 600000 ms");
        }
        if self.max_line_bytes == 0 || self.max_line_bytes > MAX_LINE_BYTES {
            return Some("exactListen: maxLineBytes is 1 to 1048576");
        }
        if self.max_connections == 0 || self.max_connections > MAX_CONNECTIONS {
            return Some("exactListen: maxConnections is 1 to 64");
        }
        None
    }
}

/// The path of a `unix:` address, or why it is refused before anything is
/// created (D4): an address of another scheme, a relative path, a NUL, or a
/// path `sun_path` cannot hold.
pub fn unix_path(address: &str) -> Result<&str, String> {
    let Some(path) = address.strip_prefix("unix:") else {
        return Err(format!(
            "exactListen: an address is unix:<absolute path>, not {address}"
        ));
    };
    if !path.starts_with('/') {
        return Err("exactListen: a unix: path is absolute".into());
    }
    if path.contains('\0') {
        return Err("exactListen: a unix: path has no NUL byte".into());
    }
    if path.len() >= SUN_PATH {
        return Err(format!(
            "exactListen: a unix: path is at most {} bytes here",
            SUN_PATH - 1
        ));
    }
    if path.ends_with('/') || path.split('/').any(|s| s == "." || s == "..") {
        return Err("exactListen: a unix: path names a file, with no . or .. segment".into());
    }
    Ok(path)
}

/// The prefix a `net.listen` line admits (`unix:` alone admits every path,
/// as the empty prefix), or `None` for a line that is not one.
pub fn grant_prefix(line: &str) -> Option<&str> {
    let mut words = line.split_whitespace();
    if words.next()? != "net.listen" {
        return None;
    }
    let prefix = words.next()?.strip_prefix("unix:")?;
    words.next().is_none().then_some(prefix)
}

/// Why a line that begins `net.listen` is not one (D11), if it is not.
pub fn grant_refusal(line: &str) -> Option<String> {
    match grant_prefix(line) {
        Some("") => None,
        Some(prefix) if prefix.starts_with('/') && !prefix.contains('\0') => None,
        _ => Some(format!(
            "`{line}`: net.listen admits unix: or unix:<absolute path prefix>"
        )),
    }
}

/// Whether `path` is `prefix` or under it, by whole segments.
pub fn under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    prefix.is_empty()
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The `net.listen` line that admits `path` (its ancestors resolved), of the
/// app's grants `spec` narrowed by a source's `scope`, which may only select
/// lines the app has. `resolve` resolves a prefix as the path was (D4), so
/// `/tmp/…` and `/private/tmp/…` on macOS are one path.
pub fn admitting<'a>(
    spec: &'a str,
    scope: Option<&'a str>,
    path: &str,
    resolve: impl Fn(&str) -> String,
) -> Option<&'a str> {
    let lines = |text: &'a str| text.lines().map(str::trim).filter(|l| !l.is_empty());
    if scope.is_some_and(|scope| lines(scope).any(|l| !lines(spec).any(|a| a == l))) {
        return None;
    }
    lines(scope.unwrap_or(spec)).find(|line| {
        grant_prefix(line).is_some_and(|prefix| prefix.is_empty() || under(path, &resolve(prefix)))
    })
}

/// A connection's end reason, as the `end` event spells it (D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The peer closed or reset it.
    Peer,
    /// The app's `end()` finished, or the listener closed.
    App,
    /// `firstLineTimeout` passed before its first complete line.
    Timeout,
    /// A line or a queue passed its bound (D3).
    Overflow,
}

impl EndReason {
    /// Its spelling in the `end` event.
    pub fn name(self) -> &'static str {
        match self {
            EndReason::Peer => "peer",
            EndReason::App => "app",
            EndReason::Timeout => "timeout",
            EndReason::Overflow => "overflow",
        }
    }
}

/// Where a listener's address stands, for `state.streams` (D2, D12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Standing {
    /// Not yet bound: binding, or the first event has not landed.
    #[default]
    Binding,
    /// Another listener of this process holds the address (D2).
    Waiting,
    /// Bound, and receiving new connections.
    Listening,
    /// A newer listener took the address over: this one answers the
    /// connections it has and receives no new ones.
    Displaced,
}

impl Standing {
    /// Its word in `state.streams`.
    pub fn name(self) -> &'static str {
        match self {
            Standing::Binding => "binding",
            Standing::Waiting => "waiting",
            Standing::Listening => "listening",
            Standing::Displaced => "displaced",
        }
    }
}

/// One connection's counts, as the host reports them for `state.streams`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectionView {
    /// Its id.
    pub id: u64,
    /// Lines received from the peer.
    pub received: u64,
    /// Lines sent to it.
    pub sent: u64,
}

/// What the host shows of one listener (`state.streams`, D12).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListenView {
    /// Its address, as asked.
    pub address: String,
    /// Its open connections, by id.
    pub connections: Vec<ConnectionView>,
}

/// The host's live view of its listeners, by stream ticket; `None` for a
/// ticket it holds no listener for.
pub type ListenReader = std::sync::Arc<dyn Fn(u64) -> Option<ListenView> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_paths_are_absolute_files_that_fit() {
        assert_eq!(unix_path("unix:/tmp/a.sock"), Ok("/tmp/a.sock"));
        assert!(unix_path("tcp://127.0.0.1:1").is_err());
        assert!(unix_path("unix:a.sock").is_err());
        assert!(unix_path("unix:/tmp/a\0b").is_err());
        assert!(unix_path("unix:/tmp/../a").is_err());
        assert!(unix_path("unix:/tmp/").is_err());
        let long = format!("unix:/{}", "x".repeat(SUN_PATH));
        assert!(unix_path(&long).unwrap_err().contains("at most"));
        let fits = format!("unix:/{}", "x".repeat(SUN_PATH - 2));
        assert!(unix_path(&fits).is_ok());
    }

    #[test]
    fn grants_admit_by_resolved_prefix() {
        assert_eq!(grant_prefix("net.listen unix:"), Some(""));
        assert_eq!(grant_prefix("net.listen unix:/tmp/t3"), Some("/tmp/t3"));
        assert_eq!(grant_prefix("net.listen tcp://127.0.0.1"), None);
        assert!(grant_refusal("net.listen unix:relative").is_some());
        assert!(grant_refusal("net.listen unix:/a b").is_some());
        assert!(grant_refusal("net.listen unix:/tmp").is_none());
        let spec = "net.fetch https:\nnet.listen unix:/tmp/t3";
        let resolve = |p: &str| p.replace("/tmp", "/private/tmp");
        let at = |path| admitting(spec, None, path, resolve);
        assert_eq!(
            at("/private/tmp/t3/a.sock"),
            Some("net.listen unix:/tmp/t3")
        );
        assert_eq!(at("/private/tmp/t3"), Some("net.listen unix:/tmp/t3"));
        assert_eq!(at("/private/tmp/t30/a.sock"), None);
        assert_eq!(at("/tmp/t3/a.sock"), None, "compared resolved");
        let any = "net.listen unix:";
        assert_eq!(admitting(any, None, "/x/y", |p| p.into()), Some(any));
        // A scope selects only lines the app has.
        assert_eq!(
            admitting(spec, Some("net.listen unix:"), "/x", |p| p.into()),
            None
        );
        let scoped = admitting(spec, Some("net.listen unix:/tmp/t3"), "/tmp/t3/a", |p| {
            p.into()
        });
        assert_eq!(scoped, Some("net.listen unix:/tmp/t3"));
    }

    #[test]
    fn options_are_bounded() {
        assert_eq!(Listen::default().refusal(), None);
        let o = |f: fn(&mut Listen)| {
            let mut l = Listen::default();
            f(&mut l);
            l.refusal()
        };
        assert!(o(|l| l.first_line_timeout_ms = Some(0)).is_some());
        assert!(o(|l| l.first_line_timeout_ms = Some(600_001)).is_some());
        assert!(o(|l| l.first_line_timeout_ms = Some(600_000)).is_none());
        assert!(o(|l| l.max_line_bytes = MAX_LINE_BYTES + 1).is_some());
        assert!(o(|l| l.max_connections = 65).is_some());
        assert!(o(|l| l.max_connections = 64).is_none());
    }
}
