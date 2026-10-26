//! Loading the file a `use` or a `members` line resolved to.

use super::{contract_text, digest, name_error, Loader, Unit};
use crate::resolve::Resolved;
use crate::CompileError;
use contract_syntax::Span;

impl Loader<'_> {
    /// The unit of the file `resolved` names, loaded and linked unless an
    /// earlier line loaded it. `described` is the line as a refusal names
    /// it, and `span` where it is written.
    pub(super) fn load_resolved(
        &mut self,
        resolved: Resolved,
        described: &str,
        span: Span,
    ) -> Result<usize, Vec<CompileError>> {
        let key = resolved.key;
        if self.active.contains(&key) {
            return Err(vec![name_error(
                "contract-use-cycle",
                format!("{described} returns to a file already being loaded"),
                span,
            )]);
        }
        if let Some(&used) = self.cache.get(&key) {
            return Ok(used);
        }
        let used_src = match resolved.builtin {
            Some(text) => text.to_owned(),
            None => contract_text(&key)
                .map_or_else(|| std::fs::read_to_string(&key), Ok)
                .map_err(|e| {
                    vec![name_error(
                        "contract-use-unreadable",
                        format!("{described}: {}: {e}", key.display()),
                        span,
                    )]
                })?,
        };
        let used = self.sources.paths.len();
        self.sources.paths.push(key.clone());
        self.sources.origins.push(resolved.origin);
        self.sources.digests.push(digest(&used_src));
        self.sources.texts.push(used_src.as_str().into());
        self.active.push(key.clone());
        if key.file_name().is_some_and(|f| f == "exact.json") {
            // A package's module declaration (LLP 1133.001), whose
            // `use`s name its implementations' files.
            let file = crate::declaration::read(&key, &used_src, used as u32).map_err(
                |(id, message)| vec![name_error(id, format!("{described}: {message}"), span)],
            )?;
            self.units.push(Unit {
                file,
                targets: Vec::new(),
                parts: Vec::new(),
            });
            self.link(&key, used)?;
        } else {
            self.load_source(&key, &used_src, used as u32)?;
        }
        self.active.pop();
        self.cache.insert(key, used);
        Ok(used)
    }
}
