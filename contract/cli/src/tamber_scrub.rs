//! A scrub's frames (LLP 1121 D13, LLP 1122 D6): `scrub-fold`, the wire's
//! own request for each update of an open scrub (`scripts/tamber-scrub.mjs`).
//!
//! - **The base** is taken once, when the scrub opens (`open: true`), or
//!   when the digests it names are not the base's: the files at those
//!   digests (a different one on disk is a conflict, as a `propose`'s is),
//!   the app's synthetic tokens, its params and the plan's retention. A
//!   frame reads no file and walks no Contract source.
//! - **Each frame** plans the gesture's whole edit over the base with
//!   `exact_tamber::plan`, the fold `propose` runs, the whole design each
//!   time. It answers `class` (a rebuild-class edit has no scrub), `changed`
//!   and `synthetic` as `propose` does, and `values`: each changed token of
//!   the design file named as the plan's theme row names it, with its CSS
//!   text per full app tuple, from which the dev loop patches the design
//!   the builds run instead of compiling the app.
//! - It writes nothing and checks no more than the fold. The write's own
//!   checks (a role that comes to vary by an axis, a param's range,
//!   [`Server::write_checks`]) run with `verify: true`, which the wire's
//!   verifier asks once the gesture rests, answering the texts it would
//!   write too; and on release, which is a transaction.

use super::{diagnostic, files_json, seen_files, Server, Snapshot};
use serde_json::{json, Value};
use std::collections::HashMap;

/// What a scrub's frames fold over.
pub(super) struct Base {
    /// The digests the scrub opened on, by path.
    files: HashMap<String, String>,
    /// The files at those digests.
    snap: Snapshot,
    /// The app's synthetic tokens, none when its build stopped before the
    /// design (then no frame is live).
    exprs: Option<Vec<(String, bool)>>,
    retention: Option<exact_tamber::Retention>,
}

impl Server {
    /// `scrub-fold` `{ops, files, open?, verify?}`.
    pub(super) fn scrub_fold(&mut self, request: &Value) -> Result<Value, Value> {
        let seen = seen_files(request, "files")?;
        let mut ops = crate::tamber_ops::parse_ops(&request["ops"], &self.design)?;
        if request["open"] == json!(true) || self.scrub.as_ref().is_none_or(|b| b.files != seen) {
            self.scrub = None;
            let snap = self.snapshot()?;
            let stale: Vec<&String> = snap
                .iter()
                .filter(|(p, _, d)| seen.get(p).is_some_and(|s| s != d))
                .map(|(p, _, _)| p)
                .collect();
            if !stale.is_empty() {
                return Err(
                    json!({"conflict": true, "message": "a file changed since the client read it", "files": files_json(&snap), "stale": stale}),
                );
            }
            let exprs = self.synthetic(&snap);
            let retention = self.synthetic.as_ref().and_then(|(_, _, r)| r.clone());
            self.scrub = Some(Base {
                files: seen,
                snap,
                exprs,
                retention,
            });
        }
        let Some(base) = &self.scrub else {
            unreachable!("taken above");
        };
        if let Some((file, _)) = ops.iter().find(|(f, _)| !base.files.contains_key(f)) {
            return Err(
                json!({"message": format!("`{file}`: a scrub edits only the files it opened on (`files`)")}),
            );
        }
        crate::tamber_ops::param_names(&mut ops, &base.snap, &self.params.1)?;
        let texts: Vec<(String, String)> = base
            .snap
            .iter()
            .map(|(f, t, _)| (f.clone(), t.clone()))
            .collect();
        let exprs = base.exprs.clone().unwrap_or_default();
        let params = crate::tamber_ops::params(&self.params.1);
        let synthetic = exact_tamber::Synthetic {
            file: &self.design,
            exprs: &exprs,
            retention: base.retention.as_ref(),
            params: &params,
        };
        let planned = exact_tamber::plan(&texts, &ops, &self.vocab, false, synthetic).map_err(|all| {
            json!({"message": "refused", "diagnostics": all.iter().map(|(f, d)| super::located(f, d, &self.params.1)).collect::<Vec<_>>()})
        })?;
        let class = match base.exprs {
            Some(_) => planned.class.name(),
            None => exact_tamber::Class::Rebuild.name(),
        };
        let verify = request["verify"] == json!(true);
        if verify {
            self.write_checks(&texts, &planned)?;
        }
        let mut out = json!({
            "files": files_json(&base.snap),
            "changed": planned.changed.iter().map(|(f, t)| json!({"file": f, "token": t})).collect::<Vec<_>>(),
            "class": class,
            "synthetic": planned.synthetic.iter().map(|(expr, values)| json!({"expr": expr, "values": values.iter().map(|v| v.css()).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "values": planned.values.iter().map(|(name, css)| json!({"name": name, "css": css})).collect::<Vec<_>>(),
        });
        if verify {
            out["texts"] = json!(planned
                .files
                .iter()
                .map(|(p, text, _)| (p.clone(), json!(text)))
                .collect::<serde_json::Map<_, _>>());
        }
        Ok(out)
    }

    /// What the build refuses beyond the crate's check, a write refuses too,
    /// so nothing is written that the app cannot build: a role override
    /// that comes to vary by an app axis (LLP 1121 D5, the compiler's
    /// `tamber-role-axis`), and a param's value out of its range (D15).
    /// `texts` are the files as planned over.
    pub(super) fn write_checks(
        &self,
        texts: &[(String, String)],
        planned: &exact_tamber::Planned,
    ) -> Result<(), Value> {
        // A role that varied before this write is the file's own,
        // already refused by the build; only what the write makes vary.
        let varies = |text: &str| -> Vec<(String, u32, u32)> {
            exact_tamber::check(text, &self.vocab).map_or(Vec::new(), |f| {
                f.tokens
                    .iter()
                    .zip(&f.design.tokens)
                    .filter(|(t, _)| {
                        t.role.is_some() && t.values.windows(2).any(|w| w[0].css() != w[1].css())
                    })
                    .map(|(t, d)| (t.name.clone(), d.name.line, d.name.col))
                    .collect()
            })
        };
        let mut axis = Vec::new();
        for (path, text, _) in &planned.files {
            let before: Vec<String> = texts
                .iter()
                .find(|(p, _)| p == path)
                .map_or(Vec::new(), |(_, t)| {
                    varies(t).into_iter().map(|(n, ..)| n).collect()
                });
            for (name, line, col) in varies(text) {
                if !before.contains(&name) {
                    axis.push(diagnostic(path, &exact_tamber::Diagnostic { id: "tamber-role-axis", message: format!("`{name}` overrides a role and varies by an app axis: a role has one value, which the hosts' own surfaces read; vary a token the rows name instead (LLP 1121 D5)"), line, col }));
                }
            }
        }
        if !axis.is_empty() {
            return Err(json!({"message": "refused", "diagnostics": axis}));
        }
        // A param's value folds with the params where it lands, inside
        // its range (D15), as the build checks it.
        // Any write: an unset, a token's or an axis's edit can take a
        // param out of its range or its `when` out of the design too.
        if !self.params.1.tokens.is_empty() {
            let set: Vec<(String, String)> = planned
                .files
                .iter()
                .filter(|(p, _, _)| *p == self.design)
                .map(|(p, t, _)| (p.clone(), t.clone()))
                .collect();
            crate::tamber_ops::param_values(&set, &self.params.1, &self.vocab, &self.dir)?;
        }
        Ok(())
    }
}
