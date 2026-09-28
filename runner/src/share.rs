//! `share(title=, text=, url=)` (LLP 1069.003): the system share sheet, by
//! the Web Share API's member names. The command is fire-and-forget; its
//! outcome is a journal line (`share: shared`, `share: dismissed`, `share:
//! refused: <reason>`, D2). Every host's arm asks this module first, so one
//! rule validates the data and one substitute answers under the agent: a
//! held request, `tap @t shared|cancel` (D6; LLP 1069.007 D3).

use crate::agent::{field_num, field_str, quote};
use crate::{DataSource, Runner};
use exact_kernel::ViewId;
use exact_plan::Value;

/// The share data: at least one of `text` and `url`; `url` an absolute
/// `http:` or `https:` URL (D1).
#[derive(Clone, Debug, PartialEq)]
pub struct Share {
    /// `title=`: the sheet's title where the platform shows one.
    pub title: Option<String>,
    /// `text=`: the text shared.
    pub text: Option<String>,
    /// `url=`: the link shared.
    pub url: Option<String>,
}

/// What a host's arm does with a `share` (after the journal has what the
/// runner decided).
#[derive(Clone, Debug, PartialEq)]
pub enum Arm {
    /// Refused into the journal (`share: refused: <reason>`).
    Refused(String),
    /// Held for the agent under this ticket (D6): nothing is shown.
    Held(u64),
    /// Open the sheet with this data; the host journals the outcome.
    Present(Share),
}

impl Share {
    /// The command's arguments as the compiler lowers them: `(title, text,
    /// url)`, `none` for an absent one.
    pub fn from_args(args: &[Value]) -> Result<Share, String> {
        let at = |i: usize| -> Result<Option<String>, String> {
            match args.get(i) {
                None | Some(Value::Option(None)) => Ok(None),
                Some(s @ exact_plan::str_value!()) => Ok(Some(s.text().to_string())),
                Some(Value::Option(Some(v))) if v.is_str() => Ok(Some(v.text().to_string())),
                Some(_) => Err("arguments are strings".into()),
            }
        };
        if args.len() != 3 {
            return Err("takes title=, text= and url=".into());
        }
        Share {
            title: at(0)?,
            text: at(1)?,
            url: at(2)?,
        }
        .checked()
    }

    /// A host's request, `{"title":…,"text":…,"url":…}` (a member absent or
    /// `null` when the app gave none).
    pub fn from_json(json: &str) -> Result<Share, String> {
        Share {
            title: field_str(json, "title"),
            text: field_str(json, "text"),
            url: field_str(json, "url"),
        }
        .checked()
    }

    fn checked(self) -> Result<Share, String> {
        if self.text.is_none() && self.url.is_none() {
            return Err("needs text= or url=".into());
        }
        if let Some(url) = &self.url {
            if !absolute_http(url) {
                return Err("url is not an absolute http: or https: URL".into());
            }
        }
        Ok(self)
    }

    /// `{"title":…,"text":…,"url":…,"anchor":…}`: the hold's inspection
    /// summary (D6), `null` for what is absent.
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

/// A host's `share` (D1–D6), one rule on every host: refuse bad data, or a
/// host that has no sheet (`available` false) outside the agent, into the
/// journal; hold it for the agent under agent mode, where no host shows a
/// sheet; otherwise present it. `source` is the node whose input ran the
/// action (D3), the hold's anchor. The hold is not tied to that node: a
/// menu row that closes with its press must not retire the sheet it opened.
pub fn arm<D: DataSource>(
    runner: &mut Runner<D>,
    share: Result<Share, String>,
    source: Option<ViewId>,
    agent: bool,
    available: bool,
) -> Arm {
    let refuse = |runner: &mut Runner<D>, reason: String| {
        runner.log(format!("share: refused: {reason}"));
        Arm::Refused(reason)
    };
    let share = match share {
        Ok(share) => share,
        Err(reason) => return refuse(runner, reason),
    };
    if agent {
        let summary = share.summary(source);
        return Arm::Held(runner.hold("share", None, &summary, &["shared"], false));
    }
    if !available {
        return refuse(runner, "unavailable".into());
    }
    Arm::Present(share)
}

/// [`arm`] for a host whose command arrived as JSON (the web glue, the
/// Apple session): `{"title":…,"text":…,"url":…,"source":N,"agent":true}`.
/// The reply is `{"refused":"…"}`, `{"ticket":N}` or `{"present":true}`.
pub fn request<D: DataSource>(runner: &mut Runner<D>, json: &str) -> String {
    let source = field_num(json, "source")
        .filter(|n| *n >= 0.0 && *n <= u32::MAX as f64 && *n == n.trunc())
        .map(|n| n as ViewId);
    let agent = crate::agent::field_bool(json, "agent");
    match arm(runner, Share::from_json(json), source, agent, true) {
        Arm::Refused(reason) => {
            let mut s = String::from("{\"refused\":");
            quote(&reason, &mut s);
            s.push('}');
            s
        }
        Arm::Held(ticket) => format!("{{\"ticket\":{ticket}}}"),
        Arm::Present(_) => "{\"present\":true}".into(),
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
        let ok = Share::from_args(&[s("T"), none.clone(), s("https://bsky.app/x")]).unwrap();
        assert_eq!(ok.url.as_deref(), Some("https://bsky.app/x"));
        assert!(Share::from_args(&[s("T"), none.clone(), none.clone()]).is_err());
        for bad in [
            "/relative",
            "ftp://x.org",
            "https://",
            "https:///x",
            "http://a b",
            "javascript:alert(1)",
        ] {
            assert!(
                Share::from_args(&[none.clone(), none.clone(), s(bad)]).is_err(),
                "{bad}"
            );
        }
        assert!(Share::from_args(&[none.clone(), s("hi"), none.clone()]).is_ok());
        assert!(
            Share::from_json(r#"{"title":null,"text":"hi","url":"HTTP://Example.com"}"#).is_ok()
        );
        assert_eq!(
            ok.summary(Some(7)),
            r#"{"title":"T","text":null,"url":"https://bsky.app/x","anchor":7}"#
        );
    }
}
