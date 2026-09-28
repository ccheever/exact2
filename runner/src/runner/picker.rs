//! The file picker's runner half (LLP 1069.002): the element a
//! `showPicker(id)` names, the hold an agent answers in its place (LLP
//! 1069.007 D3, D4), the `app:/tmp/picked/` names a host copies into, and
//! the `change` payload a host reports. The runner never sees the bytes.
//!
//! The payload (host event kind 26) is one line per file, tab-separated:
//! `path`, `name`, `type`, `size`, `width`, `height`, `duration`, the last
//! three empty when unread. A host writes names with tabs and newlines as
//! spaces.

use super::{DataSource, Runner};
use exact_kernel::{NodeType, PropId, ViewId};
use exact_plan::Value;

/// One picked file, as `change` delivers it (the `Picked` shape).
#[derive(Debug, Clone, PartialEq)]
pub struct Picked {
    /// `app:/tmp/picked/<id>.<ext>`.
    pub path: String,
    /// The original name, for display.
    pub name: String,
    /// The MIME type of what is at `path`.
    pub mime: String,
    /// Bytes at `path`.
    pub size: f64,
    /// Pixels, orientation applied, when read.
    pub width: Option<f64>,
    /// Pixels, orientation applied, when read.
    pub height: Option<f64>,
    /// A video's seconds, when read.
    pub duration: Option<f64>,
}

impl Picked {
    /// The record, in the `Picked` shape's field order.
    pub(super) fn value(&self) -> Value {
        let maybe = |n: Option<f64>| n.map_or(Value::NONE, |n| Value::some(Value::Number(n)));
        Value::record(vec![
            Value::str(&self.path),
            Value::str(&self.name),
            Value::str(&self.mime),
            Value::Number(self.size),
            maybe(self.width),
            maybe(self.height),
            maybe(self.duration),
        ])
    }

    /// Decode a host's payload; `None` for anything malformed, or a path
    /// outside `app:/tmp/picked/`.
    pub fn payload(payload: &str) -> Option<Vec<Picked>> {
        let number = |s: &str| {
            let n = exact_num::parse_f64(s).ok()?;
            (n.is_finite() && n >= 0.0).then_some(n)
        };
        let maybe = |s: &str| {
            if s.is_empty() {
                Some(None)
            } else {
                number(s).map(Some)
            }
        };
        let mut out = Vec::new();
        for line in payload.lines().filter(|l| !l.is_empty()) {
            let f: Vec<&str> = line.split('\t').collect();
            let [path, name, mime, size, width, height, duration] = f[..] else {
                return None;
            };
            let rest = path.strip_prefix(PICKED)?;
            if rest.is_empty() || rest.contains('/') || rest.contains("..") {
                return None;
            }
            out.push(Picked {
                path: path.into(),
                name: name.into(),
                mime: mime.into(),
                size: number(size)?,
                width: maybe(width)?,
                height: maybe(height)?,
                duration: maybe(duration)?,
            });
        }
        Some(out)
    }

    /// The payload line a host writes for this file.
    pub fn line(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        let maybe = |n: Option<f64>| n.map(|n| n.to_string()).unwrap_or_default();
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.path,
            clean(&self.name),
            clean(&self.mime),
            self.size,
            maybe(self.width),
            maybe(self.height),
            maybe(self.duration)
        )
    }
}

/// Where every picked file lands.
pub const PICKED: &str = "app:/tmp/picked/";

/// What a `showPicker(id)` names: the file input and what it asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerRequest {
    /// The input's view.
    pub view: ViewId,
    /// `accept`'s tokens, as HTML reads them (trimmed, lowercased).
    pub accept: Vec<String>,
    /// `multiple`.
    pub multiple: bool,
}

impl PickerRequest {
    /// `{"view":…,"accept":[…],"multiple":…}`: what a host presents from,
    /// and, less the view, the hold's inspection summary (LLP 1069.007 D3).
    pub fn json(&self, id: &str, with_view: bool) -> String {
        let mut s = String::from("{");
        if with_view {
            s.push_str(&format!("\"view\":{},", self.view));
        }
        s.push_str("\"id\":");
        crate::agent::quote(id, &mut s);
        s.push_str(",\"accept\":[");
        for (i, t) in self.accept.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            crate::agent::quote(t, &mut s);
        }
        s.push_str(&format!("],\"multiple\":{}}}", self.multiple));
        s
    }
}

/// An extension for a picked file's name: the original's, when it is short
/// and plain, else `bin`.
pub fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty() && e.len() <= 8 && e.bytes().all(|b| b.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "bin".into())
}

/// The MIME type a file's name says, as a browser guesses it; unknown is
/// `application/octet-stream`.
pub fn mime_for(name: &str) -> &'static str {
    match extension(name).as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "heif" => "image/heif",
        "avif" => "image/avif",
        "tif" | "tiff" => "image/tiff",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "m4v" => "video/x-m4v",
        "webm" => "video/webm",
        "json" => "application/json",
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

/// Whether `accept` takes a file named `name` of type `mime`, as HTML
/// matches it: an extension, `kind/*`, or the exact type.
pub fn accepts(accept: &[String], name: &str, mime: &str) -> bool {
    let ext = format!(".{}", extension(name));
    accept.iter().any(|t| {
        t == &ext
            || t == mime
            || t.strip_suffix("/*")
                .is_some_and(|kind| mime.split('/').next() == Some(kind))
    })
}

/// iOS Safari's rule, on every host (LLP 1069.002 D6): a HEIC or HEIF
/// photo is delivered as JPEG when `accept` names images but not HEIC;
/// otherwise as it is.
pub fn delivered_type<'a>(mime: &'a str, accept: &[String]) -> &'a str {
    let heic = matches!(mime, "image/heic" | "image/heif");
    let keeps = accept.iter().any(|t| {
        matches!(
            t.as_str(),
            "image/*" | "image/heic" | "image/heif" | ".heic" | ".heif"
        )
    });
    let images = accept.iter().any(|t| t.starts_with("image/"));
    if heic && !keeps && images {
        "image/jpeg"
    } else {
        mime
    }
}

/// The paths in a `type @t` answer to a picker: one per line (the driver
/// writes absolute paths), or, on one line, separated by spaces.
pub fn answer_paths(value: &str) -> Vec<String> {
    let lines: Vec<&str> = value
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() == 1 {
        return lines[0].split_whitespace().map(str::to_owned).collect();
    }
    lines.into_iter().map(str::to_owned).collect()
}

impl<D: DataSource> Runner<D> {
    /// A picker's answer, checked before the hold is consumed: at least one
    /// file, one unless `multiple`, each of a type `accept` takes once D6
    /// has converted it — what the real picker would have let the person
    /// choose.
    pub(super) fn check_pick(&self, node: Option<ViewId>, value: &str) -> Result<(), String> {
        let n = node
            .and_then(|id| self.kernel.node(id))
            .ok_or("the file input is gone")?;
        let accept: Vec<String> = n
            .props
            .str(PropId::Accept)
            .unwrap_or("")
            .split(',')
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        let paths = answer_paths(value);
        if paths.is_empty() {
            return Err("pick takes one or more file paths; tap @t cancel dismisses it".into());
        }
        if paths.len() > 1 && n.props.bool(PropId::Multiple) != Some(true) {
            return Err(format!("this input takes one file, not {}", paths.len()));
        }
        for path in &paths {
            let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
            let mime = mime_for(name);
            let delivered = delivered_type(mime, &accept);
            let ext_ok = delivered == mime && accepts(&accept, name, mime);
            if !ext_ok && !accepts(&accept, name, delivered) {
                return Err(format!(
                    "{name} ({mime}) is not among accept=\"{}\"",
                    accept.join(",")
                ));
            }
        }
        Ok(())
    }

    /// The live file input whose `id` is `id` (the first, when several).
    pub fn picker(&self, id: &str) -> Option<PickerRequest> {
        self.kernel.find_by_id(id).into_iter().find_map(|key| {
            let n = self.kernel.node_by_key(key)?;
            if n.node_type != NodeType::Control || n.props.str(PropId::Type) != Some("file") {
                return None;
            }
            Some(PickerRequest {
                view: n.id,
                accept: n
                    .props
                    .str(PropId::Accept)
                    .unwrap_or("")
                    .split(',')
                    .map(|t| t.trim().to_ascii_lowercase())
                    .filter(|t| !t.is_empty())
                    .collect(),
                multiple: n.props.bool(PropId::Multiple) == Some(true),
            })
        })
    }

    /// `showPicker(id)` under the agent (LLP 1069.002 D9): hold a `pick`
    /// request for the agent; the OS is never asked. A second while one is
    /// held for the same input does nothing and says so, as on a device.
    pub fn hold_picker(&mut self, id: &str) -> Result<u64, String> {
        let Some(request) = self.picker(id) else {
            let e = format!("picker: refused: no file input with id \"{id}\"");
            self.log(e.clone());
            return Err(e);
        };
        if self
            .device_holds()
            .iter()
            .any(|h| h.capability == "pick" && h.node == Some(request.view))
        {
            let e = format!("picker: \"{id}\" is already open");
            self.log(e.clone());
            return Err(e);
        }
        Ok(self.hold(
            "pick",
            Some(request.view),
            &request.json(id, false),
            &[],
            true,
        ))
    }

    /// A fresh `app:/tmp/picked/<id>.<ext>`: `<id>` from the launch seed
    /// and a counter (LLP 1027.000), never the user's file name.
    pub fn picked_path(&mut self, name: &str) -> String {
        self.picked_count += 1;
        format!(
            "{PICKED}{}-{}.{}",
            self.place().seed as u64,
            self.picked_count,
            extension(name)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payload_round_trips_and_refuses_a_path_outside_picked() {
        let f = Picked {
            path: "app:/tmp/picked/1-1.jpg".into(),
            name: "IMG\t0412.HEIC".into(),
            mime: "image/jpeg".into(),
            size: 1234.0,
            width: Some(4032.0),
            height: Some(3024.0),
            duration: None,
        };
        let back = Picked::payload(&f.line()).unwrap();
        assert_eq!(back[0].name, "IMG 0412.HEIC");
        assert_eq!(back[0].width, Some(4032.0));
        assert_eq!(back[0].duration, None);
        assert!(Picked::payload("app:/data/x\ta\tb\t1\t\t\t").is_none());
        assert!(Picked::payload("app:/tmp/picked/../x\ta\tb\t1\t\t\t").is_none());
        assert!(Picked::payload("app:/tmp/picked/x\ta\tb\t-1\t\t\t").is_none());
        assert_eq!(Picked::payload("").unwrap(), vec![]);
        assert_eq!(extension("IMG_0412.HEIC"), "heic");
        assert_eq!(extension("noext"), "bin");
    }
}
