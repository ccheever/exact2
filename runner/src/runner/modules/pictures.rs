//! Pictures by a package's `img` scheme (LLP 1133.019 D2, D11 item 1). An
//! interface that declares `"images": {"scheme", "grants", "permission"?,
//! "owned"?}` serves `img` sources that begin `<scheme>:`. A host's raster
//! loader, given one, asks [`Runner::admit_picture`] before it asks the
//! bound implementation for pixels, and draws nothing on a refusal.
//!
//! Drawing is a read, never a request: no ticket is raised, whoever caused
//! the draw. Admitted: an implementation the host bound (the platform then
//! decides what it may read); under the agent, only while the scheme's
//! `permission` is `granted`, as the runner holds it. On a target that
//! `owned` names, a picture is let through under that grant line alone,
//! with `owned: true`, and the implementation draws it only where the
//! platform says it is the app's own (D6: Android's own photos). Refused: a
//! scheme no package serves, an app that declares none of `grants`, a
//! scheme with no implementation bound, a permission not granted.

use super::super::DataSource;
use super::{Json, Route, Runner};
use serde_json::json;
use std::collections::BTreeMap;

/// What [`Runner::admit_picture`] decided, by extension, for
/// `state.extensions`.
#[derive(Default)]
pub(in crate::runner) struct Pictures {
    /// The host's target, once it said (`set_module_target`).
    target: Option<String>,
    counts: BTreeMap<String, (u64, u64)>,
    last_refusal: BTreeMap<String, String>,
}

impl<D: DataSource> Runner<D> {
    /// Admission of an `img` source of a package's scheme, on `target` (the
    /// host's: `ios`, `macos`, `android`, `web`): `{"extension", "scheme",
    /// "agent", "owned"?}` to ask that extension's implementation for the
    /// picture, or `{"error", "message"}`, which the host shows as an
    /// unresolved image. `None` for a source of no package's scheme.
    pub fn admit_picture(&mut self, source: &str, target: &str) -> Option<Json> {
        let scheme = source.split_once(':')?.0;
        let row = (self.plan.modules.iter())
            .find(|m| self.substitute(m)["images"]["scheme"] == scheme)?
            .clone();
        let extension = self.plan.str(row.extension).to_owned();
        let images = self.substitute(&row)["images"].clone();
        let decided = self.picture_decision(&row, &images, target);
        let counts = self
            .extensions
            .pictures
            .counts
            .entry(extension.clone())
            .or_default();
        match &decided {
            Ok(_) => counts.0 += 1,
            Err(why) => {
                counts.1 += 1;
                // One line per change of reason, not per cell of a grid.
                let last = self
                    .extensions
                    .pictures
                    .last_refusal
                    .insert(extension.clone(), why.clone());
                if last.as_deref() != Some(why.as_str()) {
                    self.log(format!("{extension}: picture refused, {why}"));
                }
            }
        }
        Some(match decided {
            Ok(owned) => {
                self.extensions.pictures.last_refusal.remove(&extension);
                let agent = self.route(&row) == Route::Agent;
                let mut out = json!({ "extension": extension, "scheme": scheme, "agent": agent });
                if owned {
                    out["owned"] = json!(true);
                }
                out
            }
            Err(why) => json!({ "error": "NotAllowedError", "message": why }),
        })
    }

    /// The schemes the plan's interfaces serve pictures by, each once: the
    /// `img` sources a host's raster loader gives [`Runner::admit_picture`]
    /// rather than opening as files.
    pub fn picture_schemes(&self) -> Vec<String> {
        let mut schemes: Vec<String> = (self.plan.modules.iter())
            .filter_map(|m| Some(self.substitute(m)["images"]["scheme"].as_str()?.to_owned()))
            .collect();
        schemes.sort();
        schemes.dedup();
        schemes
    }

    /// `Ok(owned)` where the picture may be asked for; else why not.
    fn picture_decision(
        &self,
        row: &exact_plan::ModulesRow,
        images: &Json,
        target: &str,
    ) -> Result<bool, String> {
        let declares = |line: &str| {
            (self.data.grants().lines()).any(|g| g.split_whitespace().next() == Some(line))
        };
        let lines: Vec<&str> = (images["grants"].as_array().into_iter().flatten())
            .filter_map(Json::as_str)
            .collect();
        if !lines.iter().any(|l| declares(l)) {
            return Err(format!("grant-missing (needs {})", lines.join(" or ")));
        }
        let route = self.route(row);
        if route == Route::Substitute {
            return Err("no implementation bound".into());
        }
        let Some(permission) = images["permission"].as_str() else {
            return Ok(false);
        };
        let granted = route == Route::Real || self.permission_state(permission) == "granted";
        let read = self
            .permission_lines(row, permission)
            .iter()
            .any(|l| declares(l));
        if granted && read {
            return Ok(false);
        }
        // The app's own item under another line, where the target says so.
        if let Some(line) = images["owned"][target].as_str() {
            if declares(line) {
                return Ok(true);
            }
        }
        if !read {
            return Err(format!("grant-missing (permission {permission})"));
        }
        Err(format!(
            "permission {permission} is {}",
            self.permission_state(permission)
        ))
    }

    /// The host's target (`ios`, `macos`, `android`, `web`), which a name's
    /// `owned` and an interface's `images.owned` are by.
    /// A change asks every module name again, since its answers were by the
    /// target before (`targets`, a permission's `targets`).
    pub fn set_module_target(&mut self, target: &str) {
        if self.host_target() == target {
            self.extensions.pictures.target = Some(target.to_owned());
            return;
        }
        self.extensions.pictures.target = Some(target.to_owned());
        // The substitutes read for the old target go (`Runner::substitute`).
        self.extensions.substitutes.get_mut().clear();
        let keys: Vec<String> = (self.plan.modules.iter()).map(|m| self.key(m)).collect();
        for key in keys {
            self.announce(&key);
        }
    }

    /// The target a declaration's per-target words are read by: the one the
    /// host said, else the one this runner was built for.
    pub(in crate::runner) fn host_target(&self) -> &str {
        match self.extensions.pictures.target.as_deref() {
            Some(target) => target,
            None => super::ceremony::target(),
        }
    }

    /// Whether a call of `row` is let through as a read of the app's own
    /// item (LLP 1133.019 D11 item 4): on a target its `owned` names, the
    /// app declares that line, and its permission is not granted.
    pub(in crate::runner) fn owned_call(&self, row: &exact_plan::ModulesRow) -> bool {
        let Some(target) = self.extensions.pictures.target.as_deref() else {
            return false;
        };
        let sub = self.substitute(row);
        let Some(line) = sub["owned"][target].as_str() else {
            return false;
        };
        let declares =
            (self.data.grants().lines()).any(|g| g.split_whitespace().next() == Some(line));
        let name = sub["permission"]["name"].as_str().unwrap_or_default();
        declares && self.permission_state(name) != "granted"
    }

    /// The grant lines permission `name` covers, as its interface declares it.
    fn permission_lines(&self, row: &exact_plan::ModulesRow, name: &str) -> Vec<String> {
        let sub = self.substitute(row);
        let list = sub["permissionList"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        (list.iter().chain([&sub["permission"]]))
            .filter(|p| p["name"] == name)
            .flat_map(|p| p["grants"].as_array().cloned().unwrap_or_default())
            .filter_map(|l| l.as_str().map(str::to_owned))
            .collect()
    }

    /// `state.extensions`' `pictures` of `extension`: how many draws were
    /// admitted and refused, and the last refusal's reason.
    pub(in crate::runner) fn pictures_json(&self, extension: &str) -> Option<Json> {
        let (admitted, refused) = *self.extensions.pictures.counts.get(extension)?;
        let mut out = json!({ "admitted": admitted, "refused": refused });
        if let Some(why) = self.extensions.pictures.last_refusal.get(extension) {
            out["refusal"] = json!(why);
        }
        Some(out)
    }
}
