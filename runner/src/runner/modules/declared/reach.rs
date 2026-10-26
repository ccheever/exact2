//! The system's ways into an app (LLP 1133.023 §10), substituted: the menu
//! of its icon and its content in the system's search. Under the agent the
//! runner keeps what the host's stores would (D15 item 2), and tells no
//! platform anything.
//!
//! - **`"shortcuts": "set" | "clear"`** on a send replaces the app's list
//!   (`setAppShortcuts`, D3): each item's `name` is not empty, its `url` an
//!   absolute app path the navigation root's routes match, and its icons
//!   local, else the call is refused `TypeError` and the list is kept. The
//!   agent's state shows the list after the manifest's (`manifest`, which
//!   the bake writes into the declaration from `app.json`).
//! - **`"shortcuts": "pin"`** answers `{"state": "requested"}` and lists the
//!   shortcut under `pinRequests`, or `unsupported` while the agent's word
//!   `quick-actions.pinSupported` is false (D11).
//! - **`"index": "add" | "delete" | "set" | "entries"`** keeps the content
//!   index (D4): an entry by `id`, of a `kind`, in the order the app last
//!   set them; `set` replaces one kind's entries and records the
//!   difference. At most [`MOST`] entries of at most [`TEXT`] bytes each
//!   (`QuotaExceededError`). A source declared `entries` answers them all,
//!   and is asked again when they change.

use super::super::super::{DataSource, Runner};
use super::super::{Json, ModulesRow};
use serde_json::json;

/// The most entries the index holds (D4).
const MOST: usize = 5_000;
/// The most text one entry holds, in bytes of its JSON (D4).
const TEXT: usize = 1_024;

#[derive(Default)]
pub(in crate::runner) struct Reach {
    /// The app's shortcut list, as it last set it.
    shortcuts: Vec<Json>,
    /// Each shortcut a pin was asked for, oldest first.
    pins: Vec<Json>,
    /// The content index, in the order the app set it.
    index: Vec<Json>,
    /// The last change's difference: `{kind, added, updated, deleted}`.
    last: Option<Json>,
}

impl<D: DataSource> Runner<D> {
    /// What a call of `row` with `args` does to the reach's state, where its
    /// declaration names one: `Some` of `null` or `{"error": …}`; `None` for
    /// a name that is not the reach's.
    pub(super) fn apply_reach(&mut self, row: &ModulesRow, args: &Json) -> Option<Json> {
        let sub = self.substitute(row);
        if let Some(op) = sub["shortcuts"].as_str().filter(|op| *op != "pin") {
            let items = match op {
                "set" => args["shortcuts"].as_array().cloned().unwrap_or_default(),
                _ => Vec::new(),
            };
            if let Some(why) = items.iter().find_map(|i| self.shortcut_refused(i)) {
                return Some(self.reach_refused(row, "TypeError", &why));
            }
            self.log(format!("{}: {} shortcuts", self.key(row), items.len()));
            self.extensions.declared.reach.shortcuts = items;
            return Some(Json::Null);
        }
        let op = sub["index"].as_str()?;
        let mut index = self.extensions.declared.reach.index.clone();
        let (mut added, mut updated, mut deleted) = (Vec::new(), Vec::new(), Vec::new());
        let kind = match op {
            "add" => {
                let entry = args["description"].clone();
                if let Some(why) = self.entry_refused(&entry, None) {
                    return Some(self.reach_refused(row, "TypeError", &why));
                }
                let kind = entry["kind"].clone();
                match index.iter().position(|e| e["id"] == entry["id"]) {
                    Some(at) if index[at] == entry => {}
                    Some(at) => {
                        updated.push(entry["id"].clone());
                        index[at] = entry;
                    }
                    None => {
                        added.push(entry["id"].clone());
                        index.push(entry);
                    }
                }
                kind
            }
            "delete" => {
                let id = &args["id"];
                let kind = (index.iter().find(|e| e["id"] == *id)).map(|e| e["kind"].clone());
                if kind.is_some() {
                    index.retain(|e| e["id"] != *id);
                    deleted.push(id.clone());
                }
                kind.unwrap_or_default()
            }
            "set" => {
                let kind = args["kind"].as_str().unwrap_or_default().to_owned();
                if kind.is_empty() {
                    return Some(self.reach_refused(row, "TypeError", "kind= is not empty"));
                }
                let mut entries = Vec::new();
                for e in args["entries"].as_array().into_iter().flatten() {
                    let mut e = e.clone();
                    e["kind"] = json!(kind);
                    if let Some(why) = self.entry_refused(&e, Some(&kind)) {
                        return Some(self.reach_refused(row, "TypeError", &why));
                    }
                    entries.push(e);
                }
                let mine = |e: &Json| e["kind"] == kind.as_str();
                for e in &entries {
                    match index.iter().find(|o| o["id"] == e["id"]) {
                        Some(o) if o == e => {}
                        Some(_) => updated.push(e["id"].clone()),
                        None => added.push(e["id"].clone()),
                    }
                }
                for o in index.iter().filter(|o| mine(o)) {
                    if !entries.iter().any(|e| e["id"] == o["id"]) {
                        deleted.push(o["id"].clone());
                    }
                }
                // The kind's entries take the place of its first, in the
                // order given: the order its suggestions take (D9).
                let given = |o: &Json| entries.iter().any(|e| e["id"] == o["id"]);
                let first = index.iter().position(mine).unwrap_or(index.len());
                let at = index[..first].iter().filter(|o| !given(o)).count();
                let mut rest: Vec<Json> = (index.iter())
                    .filter(|o| !mine(o) && !given(o))
                    .cloned()
                    .collect();
                let tail = rest.split_off(at);
                rest.extend(entries);
                rest.extend(tail);
                index = rest;
                json!(kind)
            }
            _ => return None,
        };
        if index.len() > MOST {
            return Some(self.reach_refused(
                row,
                "QuotaExceededError",
                &format!("the index holds at most {MOST} entries"),
            ));
        }
        self.log(format!(
            "{}: {} added, {} updated, {} deleted",
            self.key(row),
            added.len(),
            updated.len(),
            deleted.len()
        ));
        let r = &mut self.extensions.declared.reach;
        r.index = index;
        r.last =
            Some(json!({ "kind": kind, "added": added, "updated": updated, "deleted": deleted }));
        self.announce_index();
        Some(Json::Null)
    }

    /// `requestPinShortcut` (D11), substituted.
    pub(in crate::runner) fn pin_shortcut(&mut self, row: &ModulesRow, args: &Json) -> Json {
        let shortcut = &args["shortcut"];
        if let Some(why) = self.shortcut_refused(shortcut) {
            return self.reach_refused(row, "TypeError", &why);
        }
        let word = format!("{}.pinSupported", self.plan.str(row.extension));
        if self.extensions.preferred.get(&word) == Some(&Json::Bool(false)) {
            self.log(format!("{}: unsupported", self.key(row)));
            return json!({ "state": "unsupported" });
        }
        self.extensions.declared.reach.pins.push(shortcut.clone());
        self.log(format!("{}: requested", self.key(row)));
        json!({ "state": "requested" })
    }

    fn reach_refused(&mut self, row: &ModulesRow, code: &str, why: &str) -> Json {
        self.log(format!("{}: refused: {code} ({why})", self.key(row)));
        json!({ "error": code, "message": why })
    }

    /// Why a shortcut is refused (D3), if it is.
    fn shortcut_refused(&self, item: &Json) -> Option<String> {
        if item["name"].as_str().is_none_or(str::is_empty) {
            return Some("a shortcut's name= is not empty".into());
        }
        self.url_refused(&item["url"])
            .or_else(|| icons_refused(&item["icons"]))
    }

    /// Why a content index entry is refused (D4), if it is.
    fn entry_refused(&self, entry: &Json, kind: Option<&str>) -> Option<String> {
        if entry["id"].as_str().is_none_or(str::is_empty) {
            return Some("an entry's id= is not empty".into());
        }
        if kind.is_none() && entry["kind"].as_str().is_none_or(str::is_empty) {
            return Some("an entry added alone names its kind=".into());
        }
        if entry.to_string().len() > TEXT {
            return Some(format!("an entry holds at most {TEXT} bytes of text"));
        }
        self.url_refused(&entry["url"])
            .or_else(|| icons_refused(&entry["icons"]))
    }

    fn url_refused(&self, url: &Json) -> Option<String> {
        let url = url.as_str().unwrap_or_default();
        if !url.starts_with('/') || url.starts_with("//") {
            return Some(format!("url={url:?} is an absolute app path"));
        }
        (!self.route_matches(url)).then(|| format!("url={url:?} matches none of the routes"))
    }

    fn announce_index(&self) {
        let keys: Vec<String> = (self.plan.modules.iter())
            .filter(|m| self.substitute(m)["index"] == "entries")
            .map(|m| self.key(m))
            .collect();
        for key in keys {
            self.announce(&key);
        }
    }

    /// A source declared `"index": "entries"`: every entry of the index.
    pub(in crate::runner) fn reach_value(&self, sub: &Json) -> Option<Json> {
        (sub["index"] == "entries")
            .then(|| Json::Array(self.extensions.declared.reach.index.clone()))
    }

    /// The entries of `kind` whose title or description contains `matching`,
    /// case- and diacritic-insensitive, in the index's order (D9).
    pub fn index_entities(&self, kind: &str, matching: Option<&str>) -> Vec<Json> {
        let folded = matching.map(fold);
        (self.extensions.declared.reach.index.iter())
            .filter(|e| e["kind"] == kind)
            .filter(|e| {
                folded.as_ref().is_none_or(|m| {
                    ["title", "description"]
                        .iter()
                        .any(|f| e[*f].as_str().is_some_and(|t| fold(t).contains(m.as_str())))
                })
            })
            .cloned()
            .collect()
    }

    /// The reach's fields of `extension`'s entry in `state.extensions`.
    pub(in crate::runner) fn reach_json(&self, extension: &str, entry: &mut Json) {
        let r = &self.extensions.declared.reach;
        let ours = |k: &str| {
            (self.plan.modules.iter()).any(|m| {
                self.plan.str(m.extension) == extension && !self.substitute(m)[k].is_null()
            })
        };
        if ours("shortcuts") {
            let manifest = (self.plan.modules.iter())
                .filter(|m| self.plan.str(m.extension) == extension)
                .find_map(|m| self.substitute(m)["manifest"].as_array().cloned())
                .unwrap_or_default();
            entry["manifest"] = json!(manifest);
            entry["shortcuts"] = json!(r.shortcuts);
            entry["pinRequests"] = json!(r.pins);
        }
        if ours("index") {
            let mut counts = serde_json::Map::new();
            for e in &r.index {
                let kind = e["kind"].as_str().unwrap_or_default().to_owned();
                let n = counts.get(&kind).and_then(Json::as_u64).unwrap_or(0);
                counts.insert(kind, json!(n + 1));
            }
            entry["counts"] = Json::Object(counts);
            entry["last"] = r.last.clone().unwrap_or(Json::Null);
        }
    }
}

/// A remote icon is refused (D3): iOS takes symbols and bundled images, and
/// Android an `Icon` from a resource or a bitmap.
fn icons_refused(icons: &Json) -> Option<String> {
    (icons.as_array().into_iter().flatten())
        .filter_map(|i| i["src"].as_str())
        .find(|src| src.contains("://") && !src.starts_with("app:/"))
        .map(|src| format!("icon {src:?} is a local image (an asset or an app:/ path)"))
}

/// The Latin-1 and Latin Extended-A letters from U+00C0, each as its base
/// letter where Unicode decomposes it to one with a mark.
const BASES: &str = "AAAAAAÆCEEEEIIIIÐNOOOOO×ØUUUUYÞßaaaaaaæceeeeiiiiðnooooo÷øuuuuyþyAaAaAaCcCcCcCcDdĐđEeEeEeEeEeGgGgGgGgHhĦħIiIiIiIiIıĲĳJjKkĸLlLlLlĿŀŁłNnNnNnŉŊŋOoOoOoŒœRrRrRrSsSsSsSsTtTtŦŧUuUuUuUuUuUuWwYyYZzZzZzſ";

/// `text` lowercased, a Latin letter with a mark as its base letter, and
/// combining marks dropped: enough of Unicode's folding for a title.
fn fold(text: &str) -> String {
    (text.chars())
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .map(|c| match c as u32 {
            n @ 0xC0..=0x17F => BASES.chars().nth((n - 0xC0) as usize).unwrap_or(c),
            _ => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}
