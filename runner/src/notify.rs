//! `showNotification(title=, body=, tag=, showTrigger=)` and
//! `closeNotification(tag)`: local notifications by the web's Notification
//! API names (`ServiceWorkerRegistration.showNotification`'s title and
//! options; `showTrigger` is the Notification Triggers draft's member, its
//! one trigger a timestamp, given as epoch milliseconds). Admitted
//! 2026-10-04 (`rules/DEFERRED.md`; x2apps habits F13, dash's alerts).
//!
//! Like `share`, a command is fire-and-forget and its outcome a journal line
//! (`showNotification: shown`, `scheduled`, `refused: <reason>`). Every
//! host's arm asks this module first, so one rule checks the data and the
//! `device.notifications` grant, and one substitute answers under the agent:
//! nothing reaches the system, and `state.notifications` lists what the app
//! posted, a newer one replacing an older one with its tag, as the web's
//! `tag` does.

use crate::agent::{error, field_num, field_str, num, quote};
use crate::{DataSource, Runner};
use exact_plan::Value;

/// One notification as the app posted it.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    /// `title=`: required, not empty.
    pub title: String,
    /// `body=`: the text under the title.
    pub body: Option<String>,
    /// `tag=`: a newer notification with the same tag replaces this one, and
    /// `closeNotification(tag)` takes it away, shown or scheduled.
    pub tag: Option<String>,
    /// `showTrigger=`: when to show it, in epoch milliseconds; none is now.
    pub show_trigger: Option<f64>,
}

/// What a host's arm does with a `showNotification` or `closeNotification`
/// (after the journal has what the runner decided).
#[derive(Clone, Debug, PartialEq)]
pub enum Arm {
    /// Refused into the journal (`showNotification: refused: <reason>`).
    Refused(String),
    /// Listed for the agent (`state.notifications`): nothing is shown.
    Listed,
    /// Hand it to the system; the host journals the outcome.
    Present,
}

impl Notice {
    /// The command's arguments as the compiler lowers them: `(title, body,
    /// tag, showTrigger)`, `none` for an absent one.
    pub fn from_args(args: &[Value]) -> Result<Notice, String> {
        let text = |i: usize| -> Result<Option<String>, String> {
            match args.get(i) {
                None | Some(Value::Option(None)) => Ok(None),
                Some(s @ exact_plan::str_value!()) => Ok(Some(s.text().to_string())),
                Some(Value::Option(Some(v))) if v.is_str() => Ok(Some(v.text().to_string())),
                Some(_) => Err("title=, body= and tag= are strings".into()),
            }
        };
        let at = match args.get(3) {
            None | Some(Value::Option(None)) => None,
            Some(Value::Number(n)) => Some(*n),
            Some(Value::Option(Some(v))) => match v.as_ref() {
                Value::Number(n) => Some(*n),
                _ => return Err("showTrigger= is a time in epoch milliseconds".into()),
            },
            Some(_) => return Err("showTrigger= is a time in epoch milliseconds".into()),
        };
        if args.len() != 4 {
            return Err("takes title=, body=, tag= and showTrigger=".into());
        }
        Notice {
            title: text(0)?.unwrap_or_default(),
            body: text(1)?,
            tag: text(2)?,
            show_trigger: at,
        }
        .checked()
    }

    /// A host's request, `{"title":…,"body":…,"tag":…,"showTrigger":…}`.
    pub fn from_json(json: &str) -> Result<Notice, String> {
        Notice {
            title: field_str(json, "title").unwrap_or_default(),
            body: field_str(json, "body"),
            tag: field_str(json, "tag"),
            show_trigger: field_num(json, "showTrigger"),
        }
        .checked()
    }

    fn checked(self) -> Result<Notice, String> {
        if self.title.is_empty() {
            return Err("needs a title=".into());
        }
        if self.show_trigger.is_some_and(|t| !t.is_finite() || t < 0.0) {
            return Err("showTrigger= is a time in epoch milliseconds".into());
        }
        Ok(self)
    }

    /// `{"title":…,"body":…,"tag":…,"showTrigger":…}`, `null` for what is
    /// absent: an entry of `state.notifications`.
    pub fn summary(&self, s: &mut String) {
        s.push_str("{\"title\":");
        quote(&self.title, s);
        for (key, value) in [("body", &self.body), ("tag", &self.tag)] {
            s.push_str(&format!(",\"{key}\":"));
            match value {
                Some(v) => quote(v, s),
                None => s.push_str("null"),
            }
        }
        match self.show_trigger {
            Some(t) => s.push_str(&format!(",\"showTrigger\":{}}}", num(t))),
            None => s.push_str(",\"showTrigger\":null}"),
        }
    }
}

/// Whether the app's grants name `device.notifications` (LLP 1069.008 D3).
fn granted<D: DataSource>(runner: &Runner<D>) -> bool {
    crate::device::granted(runner.data_ref().grants(), "notifications")
}

/// A host's `showNotification`, one rule on every host: refuse bad data, a
/// missing grant, or a host with no notifications (`available` false)
/// outside the agent, into the journal; list it for the agent, where no host
/// reaches the system; otherwise present it.
pub fn arm<D: DataSource>(
    runner: &mut Runner<D>,
    notice: Result<Notice, String>,
    agent: bool,
    available: bool,
) -> Arm {
    let refuse = |runner: &mut Runner<D>, reason: String| {
        runner.log(format!("showNotification: refused: {reason}"));
        Arm::Refused(reason)
    };
    let notice = match notice {
        Ok(notice) => notice,
        Err(reason) => return refuse(runner, reason),
    };
    if !granted(runner) {
        return refuse(runner, "the grants name no device.notifications".into());
    }
    if agent {
        runner.log(format!(
            "showNotification: listed{}",
            notice
                .tag
                .as_deref()
                .map(|t| format!(" ({t})"))
                .unwrap_or_default()
        ));
        let list = runner.notifications_mut();
        if let Some(tag) = &notice.tag {
            list.retain(|n| n.tag.as_ref() != Some(tag));
        }
        list.push(notice);
        return Arm::Listed;
    }
    if !available {
        return refuse(runner, "unavailable".into());
    }
    Arm::Present
}

/// A host's `closeNotification(tag)`: under the agent it takes the listed
/// one away; elsewhere the host closes and unschedules it (never refused: a
/// tag nothing carries is nothing to close).
pub fn close<D: DataSource>(runner: &mut Runner<D>, tag: &str, agent: bool) -> Arm {
    if agent {
        runner
            .notifications_mut()
            .retain(|n| n.tag.as_deref() != Some(tag));
        return Arm::Listed;
    }
    Arm::Present
}

/// [`arm`] and [`close`] for a host whose command arrived as JSON (the web
/// glue, the Apple session): `{"command":"showNotification",…,"agent":true}`
/// or `{"command":"closeNotification","tag":…}`. The reply is
/// `{"refused":"…"}`, `{"listed":true}` or `{"present":true}`.
pub fn request<D: DataSource>(runner: &mut Runner<D>, json: &str) -> String {
    let agent = crate::agent::field_bool(json, "agent");
    let arm = if field_str(json, "command").as_deref() == Some("closeNotification") {
        match field_str(json, "tag") {
            Some(tag) => close(runner, &tag, agent),
            None => return error("closeNotification takes a tag"),
        }
    } else {
        arm(runner, Notice::from_json(json), agent, true)
    };
    match arm {
        Arm::Refused(reason) => {
            let mut s = String::from("{\"refused\":");
            quote(&reason, &mut s);
            s.push('}');
            s
        }
        Arm::Listed => "{\"listed\":true}".into(),
        Arm::Present => "{\"present\":true}".into(),
    }
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
        let ok =
            Notice::from_args(&[s("Stretch"), none.clone(), s("h1"), Value::Number(5e12)]).unwrap();
        assert_eq!(
            (ok.tag.as_deref(), ok.show_trigger),
            (Some("h1"), Some(5e12))
        );
        assert!(Notice::from_args(&[s(""), none.clone(), none.clone(), none.clone()]).is_err());
        assert!(
            Notice::from_args(&[s("T"), none.clone(), none.clone(), Value::Number(-1.0)]).is_err()
        );
        assert!(Notice::from_json(r#"{"title":"T","body":null,"tag":null}"#).is_ok());
        let mut out = String::new();
        ok.summary(&mut out);
        assert_eq!(
            out,
            r#"{"title":"Stretch","body":null,"tag":"h1","showTrigger":5000000000000}"#
        );
    }
}
