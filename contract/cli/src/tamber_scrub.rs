//! A scrub's frames (LLP 1121 D13, LLP 1122 D6): `scrub-fold`, the wire's
//! own request for each update of an open scrub (`scripts/tamber-scrub.mjs`).
//!
//! - **The base** is taken once, when the scrub opens (`open: true`), or
//!   when the digests it names are not the base's: the files at those
//!   digests (a different one on disk is a conflict, as a `propose`'s is),
//!   the app's synthetic tokens, its params and the plan's retention. A
//!   frame plans over the base; it reads the files only to ask the
//!   background whole build (`check`) whether a build under the base's
//!   retention fails.
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
    /// The retention key it opened under: a frame is `rebuild` once the
    /// background whole build says a build under it fails.
    key: String,
    /// The app's params (D15) when it opened.
    params: std::rc::Rc<contract::tamber_params::AppParams>,
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
                key: self.last_key.clone(),
                params: self.app_params(),
            });
        }
        // Live only from a build that reached the design, and while no
        // whole build under its retention has failed.
        let opened = self
            .scrub
            .as_ref()
            .and_then(|b| b.exprs.is_some().then(|| b.key.clone()));
        let known = opened.is_some_and(|key| self.known_under(&key));
        let Some(base) = &self.scrub else {
            unreachable!("taken above");
        };
        if let Some((file, _)) = ops.iter().find(|(f, _)| !base.files.contains_key(f)) {
            return Err(
                json!({"message": format!("`{file}`: a scrub edits only the files it opened on (`files`)")}),
            );
        }
        crate::tamber_ops::param_names(&mut ops, &base.snap, &base.params)?;
        // A package's or base token's first set is the app's override, as a write's is (*1.14*, D11).
        let design = base
            .snap
            .iter()
            .find(|(f, _, _)| *f == self.design)
            .map_or(String::new(), |(_, t, _)| t.clone());
        crate::tamber_packages::ops(&mut ops, &base.params, None, &self.design, &design, false)?;
        let texts: Vec<(String, String)> = base
            .snap
            .iter()
            .map(|(f, t, _)| (f.clone(), t.clone()))
            .collect();
        let exprs = base.exprs.clone().unwrap_or_default();
        let params = crate::tamber_ops::params(&base.params);
        let synthetic = exact_tamber::Synthetic {
            file: &self.design,
            exprs: &exprs,
            retention: base.retention.as_ref(),
            params: &params,
        };
        let planned = exact_tamber::plan(&texts, &ops, &self.vocab, false, synthetic).map_err(|all| {
            json!({"message": "refused", "diagnostics": all.iter().map(|(f, d)| super::located(f, d, &base.params)).collect::<Vec<_>>()})
        })?;
        let class = match known {
            true => planned.class.name(),
            false => exact_tamber::Class::Rebuild.name(),
        };
        let verify = request["verify"] == json!(true);
        if verify {
            // Verified against what it was planned on: Contract sources or
            // package designs moved since the scrub opened (its params no
            // longer the server's), a conflict the client opens it again on.
            if !std::rc::Rc::ptr_eq(&base.params, &self.app_params()) {
                return Err(
                    json!({"conflict": true, "message": "the app's components or its packages' designs changed since the scrub opened: open it again", "files": files_json(&base.snap), "stale": []}),
                );
            }
            self.write_checks(&texts, &planned, &base.key)?;
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
    /// The rows a write breaks (theme2's 14:19 ruling): the compiler's
    /// own check of just the literal rows that name a token the write
    /// touches (`contract_lower::rows::check`), under the design before and
    /// after; a row that passed and fails is refused, in the compiler's
    /// words, where it is. `before` is laid only when a row fails after.
    fn rows_check(
        &self,
        texts: &[(String, String)],
        planned: &exact_tamber::Planned,
        app: &contract::tamber_params::AppParams,
        folded: &exact_tamber::Folding,
        retention: &str,
    ) -> Result<(), Value> {
        let Some((key, rows, paths)) = &self.rows else {
            return Ok(());
        };
        // A write that edits a Contract source too (a rename's) leaves the
        // map behind it: those rows are the build's to judge.
        let contract = planned
            .files
            .iter()
            .any(|(p, _, _)| p.ends_with(".contract"));
        if *key != self.sources_key() || rows.is_empty() || contract {
            return Ok(());
        }
        // A file a whole build under this retention failed (before the
        // write, for a reason of its own: an element lowering refuses
        // before its rows, say) is refused in the build's own words, and
        // no row's refusal could be them: the rows are the build's (stage
        // 13 D, the reviews' rounds 2 and 3).
        if self.checker.seen_failed(retention) {
            return Ok(());
        }
        let Some((_, after, _)) = planned.files.iter().find(|(p, _, _)| *p == self.design) else {
            return Ok(());
        };
        let old = texts
            .iter()
            .find(|(p, _)| *p == self.design)
            .map(|(_, t)| t.as_str());
        let Ok(parsed) = exact_tamber::parse(after) else {
            return Ok(());
        };
        let names = |d: &exact_tamber::Design| -> Vec<String> {
            d.tokens
                .iter()
                .chain(&d.icons)
                .map(|t| t.name.value.clone())
                .collect()
        };
        // What the write changes, and every name it removes.
        let mut touched: Vec<String> = planned
            .changed
            .iter()
            .filter(|(f, _)| *f == self.design)
            .map(|(_, t)| t.clone())
            .collect();
        let was = old.and_then(|t| exact_tamber::parse(t).ok());
        if let Some(was) = &was {
            let now = names(&parsed);
            touched.extend(names(was).into_iter().filter(|n| !now.contains(n)));
        }
        if touched.is_empty() {
            return Ok(());
        }
        let touched: Vec<&str> = touched.iter().map(String::as_str).collect();
        let exprs = self
            .synthetic
            .as_ref()
            .map(|(_, e, _)| e.clone())
            .unwrap_or_default();
        let broken = contract_lower::rows::check(
            rows,
            &touched,
            &folded.design,
            folded,
            || was.as_ref().map(|d| contract::tamber_params::laid(d, app)),
            &exprs,
            &app.tokens,
        );
        if broken.is_empty() {
            return Ok(());
        }
        let diagnostics: Vec<Value> = broken
            .iter()
            .map(|b| {
                let span = b.error.span;
                // As tamber/1.15 names a file: the app's relative, a
                // package's by its name (`tamber_uses::shown`).
                let file = paths
                    .get(span.source_id as usize)
                    .map(|p| contract::tamber_uses::shown(p, &self.dir))
                    .unwrap_or_default();
                diagnostic(
                    &file,
                    &exact_tamber::Diagnostic {
                        id: b.error.id,
                        message: b.error.message.clone(),
                        line: span.line,
                        col: span.col,
                    },
                )
            })
            .collect();
        Err(json!({"message": "refused", "diagnostics": diagnostics}))
    }

    /// `key`: the retention key of the texts checked (a scrub's, as it was
    /// opened), whose whole build's verdict the rows read.
    pub(super) fn write_checks(
        &self,
        texts: &[(String, String)],
        planned: &exact_tamber::Planned,
        key: &str,
    ) -> Result<(), Value> {
        // A role that varied before this write is the file's own,
        // already refused by the build; only what the write makes vary.
        // Folded beside its packages' tokens (LLP 1121 D11), which an app token may name.
        let (joined, joined_icons) = contract::tamber_params::joined_all(&self.app_params());
        let joined = exact_tamber::params::Params {
            joined,
            joined_icons,
            ..exact_tamber::params::Params::default()
        };
        let varies = |text: &str| -> Vec<(String, u32, u32)> {
            let folded = exact_tamber::parse(text).and_then(|d| {
                exact_tamber::check_design_cached(
                    exact_tamber::params::laid(&d, &joined),
                    &self.vocab,
                )
            });
            folded.map_or(Vec::new(), |f| {
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
        let app = self.app_params();
        // The app's design as planned, folded as the build folds it with its
        // packages' designs (LLP 1121 D11): an edit that leaves a package
        // token naming what isn't its own (an override removed) is refused.
        // (the fold kept for the slot and row checks below: one fold a write)
        let chains = self.slot_chains();
        let mut folded = None;
        if !app.packages.is_empty() || chains.is_some() || self.rows.is_some() {
            for (path, text, _) in planned.files.iter().filter(|(p, _, _)| *p == self.design) {
                match contract::tamber_params::fold_with(text, &app, &self.vocab) {
                    Ok((f, _)) => folded = Some((path.clone(), f)),
                    Err(all) if !app.packages.is_empty() => {
                        return Err(
                            json!({"message": "refused", "diagnostics": all.iter().map(|d| super::located(path, d, &app)).collect::<Vec<_>>()}),
                        );
                    }
                    Err(_) => {} // (no packages: the plan's own check said it)
                }
            }
        }
        // The library's slot chains (LLP 1139 D4, stage 12 D2): the candidate
        // fold, with its packages and params, checked by the compiler's own
        // checker, only on the chains the write's tokens reach (closed over
        // aliases and expressions by the checker), refused in its words.
        // (the app design's own changed tokens: one of another `.tamber` file is no name here)
        let mut touched: Vec<&str> = planned
            .changed
            .iter()
            .filter(|(f, _)| *f == self.design)
            .map(|(_, t)| t.as_str())
            .collect();
        // A name the write removes is touched though the plan's changes miss
        // it (they compare folds, and a design that didn't fold before has
        // none to compare); every chain when either side doesn't parse.
        let names = |text: &str| -> Option<Vec<String>> {
            exact_tamber::parse(text)
                .ok()
                .map(|d| d.tokens.iter().map(|t| t.name.value.clone()).collect())
        };
        let was = texts
            .iter()
            .find(|(p, _)| *p == self.design)
            .map(|(_, t)| names(t));
        let now = planned
            .files
            .iter()
            .find(|(p, _, _)| *p == self.design)
            .map(|(_, t, _)| names(t));
        let mut every = false;
        let removed: Vec<String> = match (was, now) {
            (Some(Some(was)), Some(Some(now))) => {
                was.into_iter().filter(|n| !now.contains(n)).collect()
            }
            (Some(_), Some(_)) => {
                every = true;
                Vec::new()
            }
            _ => Vec::new(),
        };
        touched.extend(removed.iter().map(String::as_str));
        if every {
            touched.clear();
        }
        if let (Some(chains), Some((path, folding)), false) =
            (&chains, &folded, touched.is_empty() && !every)
        {
            let mut all =
                contract_lower::anatomy::slot::check_slots(folding, None, chains, &touched);
            // Refused: said at the line the write replaces where the candidate
            // has none (a removed override), the design as it was folded too,
            // only then (a write that passes folds nothing more).
            if !all.is_empty() {
                let was = texts
                    .iter()
                    .find(|(p, _)| *p == self.design)
                    .and_then(|(_, t)| {
                        contract::tamber_params::fold_with(t, &app, &self.vocab).ok()
                    });
                if let Some((before, _)) = &was {
                    all = contract_lower::anatomy::slot::check_slots(
                        folding,
                        Some(before),
                        chains,
                        &touched,
                    );
                }
            }
            if !all.is_empty() {
                return Err(
                    json!({"message": "refused", "diagnostics": all.iter().map(|d| super::located(path, d, &app)).collect::<Vec<_>>()}),
                );
            }
        }
        // The literal rows the write reaches, judged as the build judges them.
        if let Some((_, after)) = &folded {
            self.rows_check(texts, planned, &app, after, key)?;
        }
        if !app.tokens.is_empty() {
            let set: Vec<(String, String)> = planned
                .files
                .iter()
                .filter(|(p, _, _)| *p == self.design)
                .map(|(p, t, _)| (p.clone(), t.clone()))
                .collect();
            crate::tamber_ops::param_values(&set, &app, &self.vocab, &self.dir)?;
        }
        Ok(())
    }
}
