//! Where the picker's files land on Apple (LLP 1069.002 D4, D7): the app's
//! `app:/data`, `app:/cache` and `app:/tmp` as storage configures them, or,
//! for a scripted drive with no scratch store, a directory of this
//! process's own. The Swift host copies into the file a `pickedPath` reply
//! names and resolves an `image`/`video` `app:/` source against the roots.

use std::path::PathBuf;
use std::sync::Mutex;

static ROOTS: Mutex<Option<[PathBuf; 3]>> = Mutex::new(None);

/// Record the app's directories and empty `app:/tmp/picked/`: the last
/// launch's picks are gone (D4).
pub fn set_roots(data: PathBuf, cache: PathBuf, temporary: PathBuf) {
    let _ = std::fs::remove_dir_all(temporary.join("picked"));
    *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some([data, cache, temporary]);
}

fn roots() -> [PathBuf; 3] {
    let mut held = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    held.get_or_insert_with(|| {
        let base = std::env::temp_dir().join(format!("exact-agent-{}", std::process::id()));
        [
            base.join("data"),
            base.join("cache"),
            base.join("temporary"),
        ]
    })
    .clone()
}

/// A `pickedPath` reply (`{"path":"app:/tmp/picked/…"}`) with the file it
/// names and the three roots, for the Swift host.
pub fn with_files(reply: String) -> String {
    let [data, cache, temporary] = roots();
    let Some(rest) = reply
        .find("app:/tmp/picked/")
        .map(|i| &reply[i + "app:/tmp/".len()..])
        .and_then(|r| r.split('"').next())
    else {
        return reply;
    };
    let file = temporary.join(rest);
    let mut s = reply.trim_end_matches('}').to_owned();
    let quote =
        |p: &std::path::Path, s: &mut String| exact_runner::agent::quote(&p.to_string_lossy(), s);
    s.push_str(",\"file\":");
    quote(&file, &mut s);
    s.push_str(",\"roots\":{\"data\":");
    quote(&data, &mut s);
    s.push_str(",\"cache\":");
    quote(&cache, &mut s);
    s.push_str(",\"tmp\":");
    quote(&temporary, &mut s);
    s.push_str("}}");
    s
}
