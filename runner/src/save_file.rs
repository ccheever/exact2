//! `saveFile(id, from, suggestedName)` (LLP 1069.010 D3): export is a copy
//! the host makes. The app has already written `from`, an `app:/` file,
//! under its own grant; the host copies it to where the person chooses
//! (`NSSavePanel`, the exporting document picker, `showSaveFilePicker` or a
//! download) and reports the chosen name as `change` on the element `id`
//! names, or HTML's `cancel` there. The web has no single name for "copy
//! this file out", so the command's name is a declared deviation.
//!
//! One rule on every host, as for `share` (LLP 1069.003): the runner
//! refuses bad data into the journal (`saveFile: refused: <reason>`), holds
//! the request for the agent (capability `export`, answered by `type @t
//! <path>` or `tap @t cancel`, LLP 1069.007 D3), or tells the host to
//! present it.

use crate::agent::{field_bool, field_str, quote};
use crate::{DataSource, Runner};
use exact_kernel::ViewId;
use exact_plan::Value;

/// A checked `saveFile`.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveFile {
    /// The element whose `change` / `cancel` takes the outcome.
    pub id: String,
    /// The `app:/` file to copy out.
    pub from: String,
    /// The name the panel starts with.
    pub suggested_name: String,
}

/// What a host does with a `saveFile`.
#[derive(Clone, Debug, PartialEq)]
pub enum Arm {
    /// Refused into the journal; the host fires `cancel` at `view` when
    /// there is one.
    Refused(String, Option<ViewId>),
    /// Held for the agent under this ticket.
    Held(u64),
    /// Present the panel; the outcome goes to `view`.
    Present(SaveFile, ViewId),
}

impl SaveFile {
    /// The command's positional arguments, `(id, from, suggestedName)`.
    pub fn from_args(args: &[Value]) -> Result<SaveFile, String> {
        let at = |i: usize| match args.get(i) {
            Some(s @ exact_plan::str_value!()) => Ok(s.text().to_string()),
            _ => Err("saveFile takes (id, from, suggestedName), three strings".to_owned()),
        };
        if args.len() != 3 {
            return Err("saveFile takes (id, from, suggestedName), three strings".into());
        }
        Ok(SaveFile {
            id: at(0)?,
            from: at(1)?,
            suggested_name: at(2)?,
        })
    }

    /// `{"id":…,"from":…,"suggestedName":…}`: the hold's inspection summary
    /// and what a presenting host reads.
    pub fn summary(&self) -> String {
        let mut s = String::from("{\"id\":");
        quote(&self.id, &mut s);
        s.push_str(",\"from\":");
        quote(&self.from, &mut s);
        s.push_str(",\"suggestedName\":");
        quote(&self.suggested_name, &mut s);
        s.push('}');
        s
    }

    fn checked<D: DataSource>(&self, runner: &Runner<D>) -> Result<(), String> {
        if !plain_app_path(&self.from) {
            return Err(format!("{} is not an app:/ file", self.from));
        }
        if !covered(runner.data_ref().grants(), "fs.read", &self.from) {
            return Err(format!("{} is outside the app's fs.read grants", self.from));
        }
        let name = &self.suggested_name;
        if name.is_empty()
            || name.len() > 255
            || name == "."
            || name == ".."
            || name
                .chars()
                .any(|c| matches!(c, '/' | '\\') || c.is_control())
        {
            return Err("suggestedName is not a file name".into());
        }
        Ok(())
    }
}

/// `app:/<root>/<file…>` with no empty, `.` or `..` segment.
fn plain_app_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("app:/") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('/').collect();
    parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && *p != "." && *p != ".." && !p.contains('\0'))
}

/// Whether a `<capability> <prefix>` line of `grants` covers `path`, a
/// component at a time, as `PathPrefix::covers` reads it (`app:/data`
/// covers `app:/data/x`, never `app:/database`).
pub fn covered(grants: &str, capability: &str, path: &str) -> bool {
    let components = |p: &str| -> Vec<String> {
        let (ns, rest) = p.split_once(":/").unwrap_or(("", p));
        std::iter::once(ns.to_owned())
            .chain(rest.split('/').filter(|c| !c.is_empty()).map(str::to_owned))
            .collect()
    };
    let target = components(path);
    grants.lines().any(|line| {
        let mut words = line.split_whitespace();
        words.next() == Some(capability)
            && words
                .next()
                .is_some_and(|prefix| target.starts_with(&components(prefix)))
    })
}

/// A host's `saveFile`: refuse bad data or an unknown element into the
/// journal; under the agent, hold it (`export`, answered by `type @t
/// <path>` or `tap @t cancel`); otherwise present it. `available` false is
/// a host with no panel (Linux), which refuses outside the agent.
pub fn arm<D: DataSource>(
    runner: &mut Runner<D>,
    request: Result<SaveFile, String>,
    agent: bool,
    available: bool,
) -> Arm {
    let refuse = |runner: &mut Runner<D>, reason: String, view: Option<ViewId>| {
        runner.log(format!("saveFile: refused: {reason}"));
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
        return refuse(
            runner,
            format!("no element with id \"{}\"", request.id),
            None,
        );
    };
    if let Err(reason) = request.checked(runner) {
        return refuse(runner, reason, Some(view));
    }
    if agent {
        let summary = request.summary();
        return Arm::Held(runner.hold("export", Some(view), &summary, &[], true));
    }
    if !available {
        return refuse(runner, "unavailable".into(), Some(view));
    }
    Arm::Present(request, view)
}

/// [`arm`] for a host whose command arrived as JSON (the web glue, the
/// Apple session): `{"id":…,"from":…,"suggestedName":…,"agent":true}`. The
/// reply is `{"refused":"…","view":N?}`, `{"ticket":N}` or
/// `{"present":true,"view":N,"id":…,"from":…,"suggestedName":…}`.
pub fn request<D: DataSource>(runner: &mut Runner<D>, json: &str) -> String {
    let parsed = match (
        field_str(json, "id"),
        field_str(json, "from"),
        field_str(json, "suggestedName"),
    ) {
        (Some(id), Some(from), Some(suggested_name)) => Ok(SaveFile {
            id,
            from,
            suggested_name,
        }),
        _ => Err("saveFile takes (id, from, suggestedName), three strings".to_owned()),
    };
    match arm(runner, parsed, field_bool(json, "agent"), true) {
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

/// The name a chosen path ends in: what `change` carries.
pub fn chosen_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_cover_by_component() {
        let g = "sqlite.open app:/data/x.db\nfs.read app:/data/backups\nfs.write app:/data/backups";
        assert!(covered(g, "fs.read", "app:/data/backups/f.json"));
        assert!(covered(g, "fs.read", "app:/data/backups"));
        assert!(!covered(g, "fs.read", "app:/data/backupsx/f.json"));
        assert!(!covered(g, "fs.read", "app:/data/x.db"));
        assert!(!covered(g, "fs.write", "app:/tmp/f"));
        assert!(covered("fs.read doc:/", "fs.read", "doc:/1/a.md"));
        assert!(!covered("fs.read doc:/", "fs.read", "app:/data/a"));
        assert!(plain_app_path("app:/data/backups/f.json"));
        assert!(!plain_app_path("app:/data/../x"));
        assert!(!plain_app_path("app:/data"));
        assert!(!plain_app_path("/etc/passwd"));
        assert_eq!(chosen_name("/tmp/out/fieldnotes.json"), "fieldnotes.json");
    }

    #[test]
    fn args_are_three_strings() {
        let s = Value::str;
        let ok = SaveFile::from_args(&[s("x"), s("app:/data/a"), s("a.json")]).unwrap();
        assert_eq!(
            ok.summary(),
            r#"{"id":"x","from":"app:/data/a","suggestedName":"a.json"}"#
        );
        assert!(SaveFile::from_args(&[s("x"), s("app:/data/a")]).is_err());
        assert!(SaveFile::from_args(&[s("x"), Value::Number(1.0), s("a")]).is_err());
    }
}
