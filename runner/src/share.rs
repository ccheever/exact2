//! `share(title=, text=, url=, files=)` (LLP 1069.003): the system share
//! sheet, by the Web Share API's member names. The command is
//! fire-and-forget; its outcome is a journal line (`share: shared`, `share:
//! dismissed`, `share: refused: <reason>`, D2). Every host's arm asks this
//! module first, so one rule validates the data and one substitute answers
//! under the agent: a held request, `tap @t shared|cancel` (D6; LLP 1069.007
//! D3). `files=` (LLP 1125 D12; 1069.003 D7) lists `app:/` files, each
//! behind the app's `fs.read` grant, and `"#id"` entries: a picture of that
//! element as the host draws it now, which needs no grant, since nothing is
//! written that the app can read.

use crate::agent::{field_num, field_str, quote};
use crate::save_file::{covered, plain_app_path};
use crate::{DataSource, Runner};
use exact_kernel::ViewId;
use exact_plan::Value;

/// The share data: at least one of `text`, `url` and `files`; `url` an
/// absolute `http:` or `https:` URL (D1).
#[derive(Clone, Debug, PartialEq)]
pub struct Share {
    /// `title=`: the sheet's title where the platform shows one.
    pub title: Option<String>,
    /// `text=`: the text shared.
    pub text: Option<String>,
    /// `url=`: the link shared.
    pub url: Option<String>,
    /// `files=`: `app:/` paths and `#id` entries, in the author's order.
    pub files: Vec<String>,
}

/// What a host can put in its sheet beyond text and a link. Each refusal is
/// the journal's reason, said by name; `None` where the host shares it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    /// The host has a share sheet (desktop Linux has none): outside the
    /// agent a share is refused `unavailable` without one.
    pub sheet: bool,
    /// Why this host refuses an `app:/` file.
    pub files: Option<&'static str>,
    /// Why this host refuses an element's picture.
    pub pictures: Option<&'static str>,
}

impl Reach {
    /// iOS and macOS: file URLs, and the node's view drawn to a PNG.
    pub const APPLE: Reach = Reach {
        sheet: true,
        files: None,
        pictures: None,
    };
    /// The browser: `navigator.share({files})` for `app:/` files. A
    /// serialised element would not be the element as drawn (LLP 1125 D12).
    pub const WEB: Reach = Reach {
        sheet: true,
        files: None,
        pictures: Some("a picture of an element is not available on the web"),
    };
}

/// A `files=` entry checked against the running app.
#[derive(Clone, Debug, PartialEq)]
pub enum File {
    /// An `app:/` file the `fs.read` grant covers, for the host to read.
    Path(String),
    /// The element `id` names and its view, for the host to draw.
    Picture(String, ViewId),
}

/// What a host's arm does with a `share` (after the journal has what the
/// runner decided).
#[derive(Clone, Debug, PartialEq)]
pub enum Arm {
    /// Refused into the journal (`share: refused: <reason>`).
    Refused(String),
    /// Held for the agent under this ticket (D6): nothing is shown or drawn.
    Held(u64),
    /// Open the sheet with this data and these files, in order; the host
    /// journals the outcome.
    Present(Share, Vec<File>),
}

impl Share {
    /// The command's arguments as the compiler lowers them: `(title, text,
    /// url, files)`, `none` for an absent one.
    pub fn from_args(args: &[Value]) -> Result<Share, String> {
        let at = |i: usize| -> Result<Option<String>, String> {
            match args.get(i) {
                None | Some(Value::Option(None)) => Ok(None),
                Some(s @ exact_plan::str_value!()) => Ok(Some(s.text().to_string())),
                Some(Value::Option(Some(v))) if v.is_str() => Ok(Some(v.text().to_string())),
                Some(_) => Err("arguments are strings".into()),
            }
        };
        if args.len() != 4 {
            return Err("takes title=, text=, url= and files=".into());
        }
        let list = match &args[3] {
            Value::Option(None) => None,
            Value::Option(Some(v)) => Some(&**v),
            v => Some(v),
        };
        let files = match list {
            None => Vec::new(),
            Some(Value::List(items)) => items
                .iter()
                .map(|v| v.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or("files= is a list of strings")?,
            Some(_) => return Err("files= is a list of strings".into()),
        };
        Share {
            title: at(0)?,
            text: at(1)?,
            url: at(2)?,
            files,
        }
        .checked()
    }

    /// A host's request, `{"title":…,"text":…,"url":…,"files":[…]}` (a
    /// member absent or `null` when the app gave none).
    pub fn from_json(json: &str) -> Result<Share, String> {
        let files = match serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|v| v.get("files").cloned())
        {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .map(|v| v.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or("files= is a list of strings")?,
            Some(_) => return Err("files= is a list of strings".into()),
        };
        Share {
            title: field_str(json, "title"),
            text: field_str(json, "text"),
            url: field_str(json, "url"),
            files,
        }
        .checked()
    }

    fn checked(self) -> Result<Share, String> {
        if self.text.is_none() && self.url.is_none() && self.files.is_empty() {
            return Err("needs text=, url= or files=".into());
        }
        if let Some(url) = &self.url {
            if !absolute_http(url) {
                return Err("url is not an absolute http: or https: URL".into());
            }
        }
        Ok(self)
    }

    /// `{"title":…,"text":…,"url":…,"files":[…],"anchor":…}`: the hold's
    /// inspection summary (D6), `null` for what is absent.
    pub fn summary(&self, anchor: Option<ViewId>) -> String {
        let mut s = String::from("{");
        for (i, (key, value)) in [
            ("title", &self.title),
            ("text", &self.text),
            ("url", &self.url),
        ]
        .into_iter()
        .enumerate()
        {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("\"{key}\":"));
            match value {
                Some(v) => quote(v, &mut s),
                None => s.push_str("null"),
            }
        }
        s.push_str(",\"files\":[");
        for (i, file) in self.files.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(file, &mut s);
        }
        s.push(']');
        match anchor {
            Some(id) => s.push_str(&format!(",\"anchor\":{id}}}")),
            None => s.push_str(",\"anchor\":null}"),
        }
        s
    }
}

/// `http://` or `https://` (any case) and a host: what `new URL` and
/// `URL(string:)` both read as absolute, without a relative form to resolve.
fn absolute_http(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &url[8..]
    } else if lower.starts_with("http://") {
        &url[7..]
    } else {
        return false;
    };
    !rest.is_empty()
        && !rest.starts_with(['/', '?', '#'])
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// One `files=` entry against the running app and the host: an `#id` the
/// view has, or an `app:/` file the `fs.read` grant covers.
fn file<D: DataSource>(runner: &Runner<D>, entry: &str, reach: Reach) -> Result<File, String> {
    if let Some(id) = entry.strip_prefix('#') {
        if let Some(reason) = reach.pictures {
            return Err(reason.into());
        }
        let kernel = runner.kernel();
        return kernel
            .find_by_id(id)
            .into_iter()
            .find_map(|key| kernel.node_by_key(key).map(|n| n.id))
            .map(|view| File::Picture(id.to_string(), view))
            .ok_or_else(|| format!("no element with id \"{id}\""));
    }
    if !plain_app_path(entry) {
        return Err(format!(
            "{entry} is neither an app:/ file nor an element's #id"
        ));
    }
    if !covered(runner.data_ref().grants(), "fs.read", entry) {
        return Err(format!("{entry} is outside the app's fs.read grants"));
    }
    if let Some(reason) = reach.files {
        return Err(reason.into());
    }
    Ok(File::Path(entry.to_string()))
}

/// A host's `share` (D1–D6), one rule on every host: refuse bad data, a
/// file the app may not read, an element the view lacks, what this host
/// cannot share, or a host that has no sheet outside the agent, into the
/// journal; hold it for the agent under agent mode, where no host shows a
/// sheet or draws a picture; otherwise present it. `source` is the node
/// whose input ran the action (D3), the hold's anchor. The hold is not tied
/// to that node: a menu row that closes with its press must not retire the
/// sheet it opened.
pub fn arm<D: DataSource>(
    runner: &mut Runner<D>,
    share: Result<Share, String>,
    source: Option<ViewId>,
    agent: bool,
    reach: Reach,
) -> Arm {
    let refuse = |runner: &mut Runner<D>, reason: String| {
        runner.log(format!("share: refused: {reason}"));
        Arm::Refused(reason)
    };
    let share = match share {
        Ok(share) => share,
        Err(reason) => return refuse(runner, reason),
    };
    let files = match share
        .files
        .iter()
        .map(|entry| file(runner, entry, reach))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(files) => files,
        Err(reason) => return refuse(runner, reason),
    };
    if agent {
        let summary = share.summary(source);
        return Arm::Held(runner.hold("share", None, &summary, &["shared"], false));
    }
    if !reach.sheet {
        return refuse(runner, "unavailable".into());
    }
    Arm::Present(share, files)
}

/// [`arm`] for a host whose command arrived as JSON (the web glue, the
/// Apple session): `{"title":…,"text":…,"url":…,"files":[…],"source":N,
/// "agent":true,"web":true}`, `web` for the browser's reach. The reply is
/// `{"refused":"…"}`, `{"ticket":N}` or `{"present":true,"files":[…]}`,
/// each file `{"path":"app:/…"}` or `{"id":"card","view":N}`.
pub fn request<D: DataSource>(runner: &mut Runner<D>, json: &str) -> String {
    let source = field_num(json, "source")
        .filter(|n| *n >= 0.0 && *n <= u32::MAX as f64 && *n == n.trunc())
        .map(|n| n as ViewId);
    let agent = crate::agent::field_bool(json, "agent");
    let reach = if crate::agent::field_bool(json, "web") {
        Reach::WEB
    } else {
        Reach::APPLE
    };
    match arm(runner, Share::from_json(json), source, agent, reach) {
        Arm::Refused(reason) => {
            let mut s = String::from("{\"refused\":");
            quote(&reason, &mut s);
            s.push('}');
            s
        }
        Arm::Held(ticket) => format!("{{\"ticket\":{ticket}}}"),
        Arm::Present(_, files) => {
            let mut s = String::from("{\"present\":true,\"files\":[");
            for (i, file) in files.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                match file {
                    File::Path(path) => {
                        s.push_str("{\"path\":");
                        quote(path, &mut s);
                    }
                    File::Picture(id, view) => {
                        s.push_str("{\"id\":");
                        quote(id, &mut s);
                        s.push_str(&format!(",\"view\":{view}"));
                    }
                }
                s.push('}');
            }
            s.push_str("]}");
            s
        }
    }
}

/// The journal line an answered share hold leaves (D2, D6): `tap @t
/// shared` is `share: shared`; `cancel` is `share: dismissed`.
pub(crate) fn answered<D: DataSource>(runner: &mut Runner<D>, choice: &str) {
    runner.log(if choice == "shared" {
        "share: shared"
    } else {
        "share: dismissed"
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Value {
        Value::str(v)
    }

    #[test]
    fn data_is_checked_by_one_rule() {
        let none = Value::Option(None);
        let ok = Share::from_args(&[s("T"), none.clone(), s("https://bsky.app/x"), none.clone()])
            .unwrap();
        assert_eq!(ok.url.as_deref(), Some("https://bsky.app/x"));
        assert!(Share::from_args(&[s("T"), none.clone(), none.clone(), none.clone()]).is_err());
        for bad in [
            "/relative",
            "ftp://x.org",
            "https://",
            "https:///x",
            "http://a b",
            "javascript:alert(1)",
        ] {
            assert!(
                Share::from_args(&[none.clone(), none.clone(), s(bad), none.clone()]).is_err(),
                "{bad}"
            );
        }
        assert!(Share::from_args(&[none.clone(), s("hi"), none.clone(), none.clone()]).is_ok());
        assert!(
            Share::from_json(r#"{"title":null,"text":"hi","url":"HTTP://Example.com"}"#).is_ok()
        );
        assert_eq!(
            ok.summary(Some(7)),
            r#"{"title":"T","text":null,"url":"https://bsky.app/x","files":[],"anchor":7}"#
        );
    }

    #[test]
    fn files_alone_are_a_share_and_are_strings() {
        let none = Value::Option(None);
        let files = Value::list(vec![s("app:/data/a.png"), s("#card")]);
        let ok = Share::from_args(&[none.clone(), none.clone(), none.clone(), files]).unwrap();
        assert_eq!(ok.files, ["app:/data/a.png", "#card"]);
        assert_eq!(
            ok.summary(None),
            r##"{"title":null,"text":null,"url":null,"files":["app:/data/a.png","#card"],"anchor":null}"##
        );
        let empty = Value::list(vec![]);
        assert!(Share::from_args(&[none.clone(), none.clone(), none.clone(), empty]).is_err());
        let numbers = Value::list(vec![Value::Number(1.0)]);
        assert!(Share::from_args(&[none.clone(), s("t"), none.clone(), numbers]).is_err());
        let json = Share::from_json(r##"{"files":["#card"],"source":3}"##).unwrap();
        assert_eq!(json.files, ["#card"]);
        assert!(Share::from_json(r#"{"text":"t","files":"app:/data/a"}"#).is_err());
    }
}
