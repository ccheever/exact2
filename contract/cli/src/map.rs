//! Development-only source locations beside a plan, never inside its bytes.
//! @ref LLP 1035.005 D3; LLP 1035.002 D6.

use crate::{sources::Sources, BakeError, CompileError, RelatedLocation};
use contract_lower::{Declared, Sites};
use contract_syntax::Span;
use sha2::{Digest, Sha256};
use std::io::Write;

/// Compiler-owned locations for plan nodes and named declarations.
/// Source files resolve through each span's identity, including slot fills and
/// declarations imported separately from their component call sites.
pub struct SourceMap {
    sites: Sites,
    sources: Sources,
}

/// SHA-256 of the final encoded plan, matching the development envelope.
pub fn plan_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl SourceMap {
    pub(crate) fn new(sites: Sites, sources: Sources) -> Self {
        Self { sites, sources }
    }

    /// A producer compiled a verified copy of the app tree. Restore its original
    /// filenames by relative path, without reading newer source contents. Every
    /// source must belong to the captured tree; a refusal changes no locations.
    pub fn relocate_sources(
        &mut self,
        captured: &std::path::Path,
        original: &std::path::Path,
    ) -> Result<(), String> {
        self.sources.relocate(captured, original)
    }

    fn at(&self, out: &mut Vec<u8>, span: Span) {
        out.extend_from_slice(b"\"file\":");
        quote(out, &self.sources.path(span).to_string_lossy());
        write!(
            out,
            ",\"line\":{},\"col\":{},\"end_col\":{}",
            span.line, span.col, span.end_col
        )
        .unwrap();
    }

    fn declarations(&self, out: &mut Vec<u8>, declarations: &[Declared]) {
        out.push(b'{');
        for (i, declaration) in declarations.iter().enumerate() {
            if i != 0 {
                out.push(b',');
            }
            quote(out, &declaration.name);
            out.extend_from_slice(b":{");
            self.at(out, declaration.span);
            out.extend_from_slice(b",\"component\":");
            quote(
                out,
                &self.sites.instances[declaration.instance as usize].component,
            );
            out.push(b'}');
        }
        out.push(b'}');
    }

    /// Resolve a bake refusal through the node the runner actually measured.
    /// Refusals without a node keep an absent file and zero source range.
    pub fn bake_error(&self, error: &BakeError) -> CompileError {
        let (id, message, site) = match error {
            BakeError::Lint { id, message, site } => (*id, message.clone(), *site),
            BakeError::Runner(error) => ("bake-runner", format!("{error:?}"), None),
        };
        let node = site.and_then(|site| self.sites.nodes.get(site.0 as usize));
        let mut related = Vec::new();
        if let Some(node) = node {
            let mut instance = node.instance as usize;
            while let Some(parent) = self.sites.instances[instance].parent {
                let call = &self.sites.instances[instance];
                related.push(RelatedLocation {
                    span: call.span,
                    file: Some(self.sources.path(call.span).to_path_buf()),
                    note: format!("`{}` is instantiated here", call.component),
                });
                instance = parent as usize;
            }
        }
        CompileError {
            pass: "bake",
            id: id.into(),
            message,
            span: node.map_or_else(Span::default, |node| node.span),
            file: node.map(|node| self.sources.path(node.span).to_path_buf()),
            related: related.into_boxed_slice(),
        }
    }

    /// Serialize beside the final plan bytes. A bake changes the plan's resource
    /// values, so the caller supplies the encoded bytes *after* baking.
    pub fn json(&self, plan_bytes: &[u8]) -> String {
        // Serialize directly: no second tree of JSON objects proportional to
        // all nodes and repeated call chains on the edit-to-plan path.
        let mut out = Vec::with_capacity(self.sites.nodes.len() * 256);
        out.extend_from_slice(b"{\"digest\":");
        quote(&mut out, &plan_digest(plan_bytes));
        out.extend_from_slice(b",\"nodes\":[");
        for (i, node) in self.sites.nodes.iter().enumerate() {
            if i != 0 {
                out.push(b',');
            }
            out.push(b'{');
            self.at(&mut out, node.span);
            out.extend_from_slice(b",\"component\":");
            quote(
                &mut out,
                &self.sites.instances[node.instance as usize].component,
            );
            out.extend_from_slice(b",\"chain\":[");
            let mut instance = node.instance as usize;
            let mut first = true;
            while let Some(parent) = self.sites.instances[instance].parent {
                if !first {
                    out.push(b',');
                }
                first = false;
                out.push(b'{');
                self.at(&mut out, self.sites.instances[instance].span);
                out.extend_from_slice(b",\"component\":");
                quote(&mut out, &self.sites.instances[parent as usize].component);
                out.push(b'}');
                instance = parent as usize;
            }
            out.extend_from_slice(b"],\"bindings\":[");
            for (i, (row, origin)) in node.rows.iter().enumerate() {
                if i != 0 {
                    out.push(b',');
                }
                out.extend_from_slice(b"{\"row\":");
                quote(&mut out, row.name());
                out.extend_from_slice(b",\"origin\":");
                quote(&mut out, &origin.label());
                out.push(b'}');
            }
            out.extend_from_slice(b"]}");
        }
        out.extend_from_slice(b"],\"slots\":");
        self.declarations(&mut out, &self.sites.slots);
        out.extend_from_slice(b",\"derives\":");
        self.declarations(&mut out, &self.sites.derives);
        out.extend_from_slice(b",\"actions\":");
        self.declarations(&mut out, &self.sites.actions);
        out.push(b'}');
        String::from_utf8(out).expect("JSON serialization is UTF-8")
    }
}

fn quote(out: &mut Vec<u8>, value: &str) {
    serde_json::to_writer(out, value).expect("writing JSON into a Vec cannot fail");
}
