//! The File System Access API's three pickers, issued as commands (LLP
//! 1069.010 D2): `showOpenFilePicker(id)` (or `(id, multiple)`),
//! `showDirectoryPicker(id)`, `showSaveFilePicker(id, suggestedName)`.
//! Each opens a document in place: the host mints a `doc:` handle for
//! what the person chose (D1) and reports it as `change` on the element
//! `id` names (several, one per line, under `multiple`), or HTML's `cancel`
//! there. The types are the manifest's `file_handlers`, never the
//! command's, so an app cannot widen them per call.
//!
//! One rule on every host, as for `share` and `saveFile`: the runner
//! refuses an unknown element or bad arguments into the journal (`<name>:
//! refused: <reason>`), holds the request for the agent (capabilities
//! `open-file`, `open-directory`, `save-file`; answered by `type @t
//! <path>…` or `tap @t cancel`, LLP 1069.007 D3), or tells the host to
//! present it. A host without the picker (Linux; a browser without
//! `showOpenFilePicker`) refuses and fires `cancel`.

use crate::agent::{field_bool, field_str, quote};
use crate::{DataSource, Runner};
use exact_kernel::ViewId;
use exact_plan::Value;

/// Which picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `showOpenFilePicker`: files, opened in place.
    Open,
    /// `showDirectoryPicker`: a folder.
    Directory,
    /// `showSaveFilePicker`: a file to write, which may not exist yet.
    Save,
}

impl Kind {
    /// The command's name.
    pub fn command(self) -> &'static str {
        match self {
            Kind::Open => "showOpenFilePicker",
            Kind::Directory => "showDirectoryPicker",
            Kind::Save => "showSaveFilePicker",
        }
    }

    /// The hold's capability in `pending` (LLP 1069.010 §Substitute).
    pub fn capability(self) -> &'static str {
        match self {
            Kind::Open => "open-file",
            Kind::Directory => "open-directory",
            Kind::Save => "save-file",
        }
    }

    /// The picker a command names, if it names one.
    pub fn of(command: &str) -> Option<Kind> {
        [Kind::Open, Kind::Directory, Kind::Save]
            .into_iter()
            .find(|k| k.command() == command)
    }
}

/// A checked picker request.
#[derive(Clone, Debug, PartialEq)]
pub struct Picker {
    /// Which picker.
    pub kind: Kind,
    /// The element whose `change` / `cancel` takes the outcome.
    pub id: String,
    /// `showOpenFilePicker`'s `multiple`.
    pub multiple: bool,
    /// `showSaveFilePicker`'s `suggestedName`.
    pub suggested_name: String,
}

/// What a host does with a picker command.
#[derive(Clone, Debug, PartialEq)]
pub enum Arm {
    /// Refused into the journal; the host fires `cancel` at the view when
    /// there is one.
    Refused(String, Option<ViewId>),
    /// Held for the agent under this ticket.
    Held(u64),
    /// Present the picker; the outcome goes to the view.
    Present(Picker, ViewId),
}

impl Picker {
    /// The command's positional arguments.
    pub fn from_args(kind: Kind, args: &[Value]) -> Result<Picker, String> {
        let usage = match kind {
            Kind::Open => "showOpenFilePicker takes (id) or (id, multiple)",
            Kind::Directory => "showDirectoryPicker takes (id)",
            Kind::Save => "showSaveFilePicker takes (id, suggestedName)",
        };
        let Some(id) = args.first().and_then(Value::as_str) else {
            return Err(usage.into());
        };
        let mut picker = Picker {
            kind,
            id: id.to_string(),
            multiple: false,
            suggested_name: String::new(),
        };
        match (kind, &args[1..]) {
            (Kind::Open, []) | (Kind::Directory, []) => {}
            (Kind::Open, [Value::Bool(multiple)]) => picker.multiple = *multiple,
            (Kind::Save, [name @ exact_plan::str_value!()]) => {
                picker.suggested_name = name.text().to_string()
            }
            _ => return Err(usage.into()),
        }
        picker.checked()
    }

    fn checked(self) -> Result<Picker, String> {
        let name = &self.suggested_name;
        if self.kind == Kind::Save
            && (name.is_empty()
                || name.len() > 255
                || name == "."
                || name == ".."
                || name
                    .chars()
                    .any(|c| matches!(c, '/' | '\\') || c.is_control()))
        {
            return Err("suggestedName is not a file name".into());
        }
        Ok(self)
    }

    /// `{"id":…,"multiple":…}` / `{"id":…,"suggestedName":…}`: the hold's
    /// inspection summary and what a presenting host reads.
    pub fn summary(&self) -> String {
        let mut s = String::from("{\"id\":");
        quote(&self.id, &mut s);
        match self.kind {
            Kind::Open => s.push_str(&format!(",\"multiple\":{}", self.multiple)),
            Kind::Directory => {}
            Kind::Save => {
                s.push_str(",\"suggestedName\":");
                quote(&self.suggested_name, &mut s);
            }
        }
        s.push('}');
        s
    }
}

/// A host's picker command: refuse, hold for the agent, or present.
/// `available` false is a host with no such picker, which refuses outside
/// the agent.
pub fn arm<D: DataSource>(
    runner: &mut Runner<D>,
    kind: Kind,
    request: Result<Picker, String>,
    agent: bool,
    available: bool,
) -> Arm {
    let refuse = |runner: &mut Runner<D>, reason: String, view: Option<ViewId>| {
        runner.log(format!("{}: refused: {reason}", kind.command()));
        Arm::Refused(reason, view)
    };
    let request = match request {
        Ok(request) => request,
        Err(reason) => return refuse(runner, reason, None),
    };
    let view = runner
        .kernel()
        .find_by_id(&request.id)
        .into_iter()
        .find_map(|key| runner.kernel().node_by_key(key).map(|n| n.id));
    let Some(view) = view else {
        let reason = format!("no element with id \"{}\"", request.id);
        return refuse(runner, reason, None);
    };
    if agent {
        let summary = request.summary();
        return Arm::Held(runner.hold(kind.capability(), Some(view), &summary, &[], true));
    }
    if !available {
        return refuse(runner, "unavailable".into(), Some(view));
    }
    Arm::Present(request, view)
}

/// [`arm`] for a host whose command arrived as JSON: `{"command":
/// "showOpenFilePicker","id":…,"multiple":false,"agent":true}` (and
/// `suggestedName` for a save, `available` false for a host that has no
/// such picker). The reply is `{"refused":"…","view":N?}`, `{"ticket":N}`
/// or `{"present":true,"view":N,"id":…,…}`.
pub fn request<D: DataSource>(runner: &mut Runner<D>, json: &str) -> String {
    let command = field_str(json, "command").unwrap_or_default();
    let Some(kind) = Kind::of(&command) else {
        return crate::agent::error(&format!("{command} is not a picker"));
    };
    let parsed = match field_str(json, "id") {
        Some(id) => Picker {
            kind,
            id,
            multiple: field_bool(json, "multiple"),
            suggested_name: field_str(json, "suggestedName").unwrap_or_default(),
        }
        .checked(),
        None => Err(format!("{command} names an element id")),
    };
    let available = !json.contains("\"available\":false");
    match arm(runner, kind, parsed, field_bool(json, "agent"), available) {
        Arm::Refused(reason, view) => {
            let mut s = String::from("{\"refused\":");
            quote(&reason, &mut s);
            if let Some(view) = view {
                s.push_str(&format!(",\"view\":{view}"));
            }
            s.push('}');
            s
        }
        Arm::Held(ticket) => format!("{{\"ticket\":{ticket}}}"),
        Arm::Present(request, view) => {
            let summary = request.summary();
            format!("{{\"present\":true,\"view\":{view},{}", &summary[1..])
        }
    }
}

/// The paths of a `type @t` answer: one per line, each absolute.
pub fn answer_paths(value: &str) -> Vec<&str> {
    value
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_picker_takes_its_own_arguments() {
        let s = Value::str;
        let open = Picker::from_args(Kind::Open, &[s("f")]).unwrap();
        assert_eq!(open.summary(), r#"{"id":"f","multiple":false}"#);
        let many = Picker::from_args(Kind::Open, &[s("f"), Value::Bool(true)]).unwrap();
        assert!(many.multiple);
        assert_eq!(
            Picker::from_args(Kind::Directory, &[s("d")])
                .unwrap()
                .summary(),
            r#"{"id":"d"}"#
        );
        let save = Picker::from_args(Kind::Save, &[s("s"), s("a.md")]).unwrap();
        assert_eq!(save.summary(), r#"{"id":"s","suggestedName":"a.md"}"#);
        assert!(Picker::from_args(Kind::Save, &[s("s")]).is_err());
        assert!(Picker::from_args(Kind::Save, &[s("s"), s("../a")]).is_err());
        assert!(Picker::from_args(Kind::Directory, &[s("d"), Value::Bool(true)]).is_err());
        assert_eq!(Kind::of("showDirectoryPicker"), Some(Kind::Directory));
        assert_eq!(Kind::Save.capability(), "save-file");
        assert_eq!(answer_paths("/a\n /b \n"), ["/a", "/b"]);
    }
}
