//! Declared fonts (LLP 1019) and Canvas 2D surface arguments (LLP 1056) in
//! the JS target (LLP 1071 §7).
//!
//! Fonts are the stylesheet's, not the runtime's: each declared face is an
//! `@font-face` rule under the family name the web host's CSS uses
//! (`exact_web::host::fonts`), with `font-display: optional` — the wasm
//! host's own policy (glue.js `prepareFonts`: a face not ready within about
//! 100 ms stays unused for the page, no swap after paint) — and a preload in
//! the served head, so a face is usually there before the text that needs it.

use exact_plan::{Opcode, Plan};
use exact_runner::vm::instructions;

/// The faces' `@font-face` rules, the preload links for the head, and the
/// declared-name → CSS-family map canvas text uses (`exact.fontAliases`,
/// LLP 1056 D8), as a JavaScript object literal; empty when none.
pub struct Fonts {
    pub css: String,
    pub preloads: String,
    pub aliases: String,
}

pub fn fonts(plan: &Plan) -> Result<Fonts, String> {
    let faces = exact_web::host::fonts::font_faces(plan);
    let mut out = Fonts {
        css: String::new(),
        preloads: String::new(),
        aliases: String::new(),
    };
    let mut aliases = Vec::new();
    for f in &faces {
        if f.source
            .chars()
            .any(|c| matches!(c, '"' | '\\' | '<' | '>' | '\n' | '\r'))
        {
            return Err(format!("font source {:?} can't be a CSS string", f.source));
        }
        out.css.push_str(&format!(
            "@font-face{{font-family:\"{}\";src:url(\"{}\");font-weight:{};font-style:{};font-display:optional}}",
            f.family,
            f.source,
            f.weight,
            if f.italic { "italic" } else { "normal" }
        ));
        // host/web/src/page.rs `fonts`: a preload each, for the formats a
        // browser names.
        let kind = f.source.rsplit('.').next().map(str::to_ascii_lowercase);
        if let Some(kind) = kind.filter(|k| matches!(k.as_str(), "ttf" | "otf" | "woff" | "woff2"))
        {
            out.preloads.push_str(&format!(
                "<link rel=\"preload\" href=\"{}\" as=\"font\" type=\"font/{kind}\" crossorigin>\n",
                f.source
            ));
        }
        let pair = format!(
            "{}:{}",
            serde_json::to_string(&f.declared).unwrap(),
            serde_json::to_string(&f.family).unwrap()
        );
        if !aliases.contains(&pair) {
            aliases.push(pair);
        }
    }
    if !aliases.is_empty() {
        out.aliases = format!("{{{}}}", aliases.join(","));
    }
    Ok(out)
}

/// A surface argument's declared type code (`types::type_code`) when its
/// code reads one slot, derive or resource; `None` otherwise, and the data
/// seam encodes the value by its shape (`host/web-js/rust-data.js`).
pub fn arg_type(plan: &Plan, code: &[u8]) -> Option<exact_plan::TypesId> {
    let ins: Vec<_> = instructions(code).collect::<Result<_, _>>().ok()?;
    let mut loads = ins.iter().filter(|i| i.op != Opcode::Return);
    let x = loads.next()?;
    if loads.next().is_some() {
        return None;
    }
    let k = x.args[0] as usize;
    match x.op {
        Opcode::LoadSlot => plan.slots.get(k).map(|s| s.ty),
        Opcode::LoadDerive => plan.derives.get(k).map(|d| d.ty),
        Opcode::LoadResource => plan.resources.get(k).map(|r| r.ty),
        _ => None,
    }
}
