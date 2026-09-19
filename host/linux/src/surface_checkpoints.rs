//! App/surface-scoped durable checkpoints and post-commit request tokens.
use super::{Abi, Canvas, Host, LIMIT};
use exact_kernel::{PropId, PropValue};
use exact_runner::{DataSource, Event};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct Requests {
    save: i64,
    load: i64,
    pub(super) pending: bool,
    reply: Option<&'static str>,
}
impl Requests {
    pub(super) fn finish(&mut self, refused: bool) {
        if self.pending {
            self.pending = false;
            self.reply = Some(if refused {
                "surface-load:error"
            } else {
                "surface-load:loaded"
            });
        }
    }
}
fn component(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn root() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("EXACT_SURFACE_STORE") {
        return Ok(path.into());
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
        .ok_or("application data directory unavailable")?;
    Ok(base.join("exact/surfaces"))
}
fn location(root: &Path, app: &str, surface: &str) -> PathBuf {
    root.join(component(app))
        .join(format!("{}.world", component(surface)))
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > LIMIT as u64 {
        return Err("world carrier exceeds 256 MiB limit".into());
    }
    let mut bytes = Vec::new();
    file.take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > LIMIT {
        return Err("world carrier exceeds 256 MiB limit".into());
    }
    Ok(bytes)
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if bytes.len() > LIMIT {
        return Err("world carrier exceeds 256 MiB limit".into());
    }
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        fs::File::open(path.parent().unwrap())?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result.map_err(|e: std::io::Error| e.to_string())
}
fn perform(c: &mut Canvas, abi: &Abi, compat: &str, save: bool) -> Result<(), String> {
    if !c.owner {
        return Err("duplicate canvas cannot checkpoint".into());
    }
    if c.restore_input {
        return Err("surface restore is pending".into());
    }
    let compat: Value = serde_json::from_str(compat).map_err(|e| e.to_string())?;
    let app = compat["inputs"]["app"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("missing app identity")?;
    let path = location(&root()?, app, &c.name);
    if save {
        let bytes = abi
            .read(b"gpu_carry", c.id)
            .ok_or_else(|| abi.error().unwrap_or("surface carries no state".into()))?;
        write(&path, &bytes)?;
        c.checkpoints.reply = Some("surface-save:saved");
    } else {
        let bytes = read(&path)?;
        if !abi.restore(c.id, &bytes) {
            return Err(abi.error().unwrap_or("surface refused saved state".into()));
        }
        c.restore_input = true;
        c.restore_bytes = Some(bytes);
        c.restore_error = None;
        c.restore_logged = false;
        c.checkpoints.pending = true;
    }
    Ok(())
}
pub(super) fn requests<D: DataSource>(
    c: &mut Canvas,
    view: u32,
    abi: &Abi,
    host: &mut Host<D>,
    compat: &str,
) {
    for (prop, save) in [(PropId::SurfaceSave, true), (PropId::SurfaceLoad, false)] {
        let token = host
            .kernel()
            .node(view)
            .and_then(|n| n.props.get(prop))
            .and_then(PropValue::as_int)
            .unwrap_or(0);
        let previous = if save {
            &mut c.checkpoints.save
        } else {
            &mut c.checkpoints.load
        };
        if *previous == token {
            continue;
        }
        *previous = token;
        if token <= 0 {
            continue;
        }
        // Emit the preceding result before another token can replace it.
        reply(c, view, host);
        if let Err(error) = perform(c, abi, compat, save) {
            host.log(format!("surface {} checkpoint: {error}", c.name));
            c.checkpoints.reply = Some(if save {
                "surface-save:error"
            } else {
                "surface-load:error"
            });
        }
    }
}
pub(super) fn reply<D: DataSource>(c: &mut Canvas, view: u32, host: &mut Host<D>) -> bool {
    let Some(message) = c.checkpoints.reply.take() else {
        return false;
    };
    if let Some(error) = host.dispatch_at(view, Event::Message(message.into()), host.now()) {
        host.log(error);
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_store_is_scoped_exact_atomic_and_size_bounded() {
        let dir = std::env::temp_dir().join(format!("exact-checkpoints-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = location(&dir, "a/../b", "world/../other");
        let bytes = [0, 255, 1, 0, 9];
        write(&path, &bytes).unwrap();
        assert_eq!(read(&path).unwrap(), bytes);
        assert!(read(&location(&dir, "other", "world/../other")).is_err());
        assert!(read(&location(&dir, "a/../b", "other")).is_err());
        let huge = dir.join("huge");
        fs::File::create(&huge)
            .unwrap()
            .set_len(LIMIT as u64 + 1)
            .unwrap();
        assert!(read(&huge).unwrap_err().contains("256 MiB"));
        write(&path, &[7, 8]).unwrap();
        assert_eq!(read(&path).unwrap(), [7, 8]);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn deferred_ack_is_once_after_commit_or_refusal() {
        for refused in [false, true] {
            let mut requests = Requests {
                pending: true,
                ..Default::default()
            };
            assert!(requests.reply.is_none());
            requests.finish(refused);
            assert_eq!(
                requests.reply.take(),
                Some(if refused {
                    "surface-load:error"
                } else {
                    "surface-load:loaded"
                })
            );
            requests.finish(refused);
            assert!(requests.reply.is_none());
        }
    }
}
