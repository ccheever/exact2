//! The JS target's half of `host/web/src/sized_css.rs`: a progress or a canvas keeps its width with
//! `justify-self: start` only where its parent's `justify-items` would stretch it (a progress in
//! block flow; a canvas in any parent). The parent writes custom properties its sized children read:
//! statically through [`parent_css`], bound through [`writes`]. `initial` is guaranteed-invalid, so
//! a `var()` falls back and an ancestor's value is not inherited.
//!
//! A sized node that parents a sized node writes its children's properties under the pair's other
//! name (`-2`), so its own `justify-self` still reads what its parent wrote; the names alternate
//! down a chain of them ([`alternates`]). Uses are inlined into the one tree `parents` is built
//! over, so a component's root placed directly inside a canvas reads that canvas's names too; only
//! a node with no parent in it (the root) reads the first names.
use exact_kernel::{Display, JustifyItems, NodeRef, PositionType, StyleId};
use exact_web::host::template::Parts;

/// The names a parent writes, by its place in the chain: (display, alignment).
fn names(alt: bool) -> (&'static str, &'static str) {
    if alt {
        ("--exact-progress-justify-2", "--exact-progress-aligned-2")
    } else {
        ("--exact-progress-justify", "--exact-progress-aligned")
    }
}

/// A progress's in-flow `justify-self`, reading the names its parent writes.
pub fn progress_justify(alt: bool) -> String {
    let (justify, aligned) = names(alt);
    format!("var({aligned},var({justify},start))")
}

/// A canvas's: `start` unless an aligning parent's `justify-items` applies.
pub fn canvas_justify(alt: bool) -> String {
    format!("var({},start)", names(alt).1)
}

/// For each node, whether it writes the second names: a sized node whose parent writes the first.
pub fn alternates(sized: &[bool], parents: &[Option<usize>]) -> Vec<bool> {
    fn alt(i: usize, sized: &[bool], parents: &[Option<usize>], memo: &mut [Option<bool>]) -> bool {
        if let Some(a) = memo[i] {
            return a;
        }
        let a = sized[i] && !parents[i].is_some_and(|p| alt(p, sized, parents, memo));
        memo[i] = Some(a);
        a
    }
    let mut memo = vec![None; sized.len()];
    (0..sized.len())
        .map(|i| alt(i, sized, parents, &mut memo))
        .collect()
}

/// A sized child's parent: its live display (block or not) and whether its own `justify-items`
/// aligns (`start`, `center`, …), as the kernel's layout does since Taffy patch 66.
pub fn parent_css(node: &NodeRef<'_>, alt: bool, css: &mut String) {
    let (justify, aligned) = names(alt);
    let block = node.style.display == Display::Block;
    let aligns = !matches!(
        node.style.justify_items,
        JustifyItems::Normal | JustifyItems::Stretch
    );
    let (j, a) = (
        if block { "start" } else { "auto" },
        if aligns { "auto" } else { "initial" },
    );
    css.push_str(&format!("{justify}:{j};{aligned}:{a};"));
}

/// A sized node's own `justify-self`, over the host CSS's `start`, reading its parent's names.
pub fn own_css(node: &NodeRef<'_>, canvas: bool, reads_alt: bool, css: &mut String) {
    let absolute = node.style.position_type == PositionType::Absolute;
    if canvas && absolute {
        return;
    }
    *css = css.replace("justify-self:start;", "");
    let justify = match (canvas, absolute) {
        (false, true) => "auto".into(),
        (false, false) => progress_justify(reads_alt),
        (true, _) => canvas_justify(reads_alt),
    };
    css.push_str(&format!("justify-self:{justify};"));
}

/// The bound writers for row `id` of a sized node or a sized node's parent, as (property, unit,
/// map); each map calls the runtime's `normal`.
pub fn writes(id: StyleId, parts: &Parts) -> Vec<(String, String, Option<String>)> {
    let css = &parts.css;
    // Which names this node writes for its children, and which it reads from its parent.
    let writes_alt = css.contains("--exact-progress-justify-2:");
    let parent = writes_alt || css.contains("--exact-progress-justify:");
    let reads_alt = css.contains("justify-self:var(--exact-progress-aligned-2");
    let (justify, aligned) = names(writes_alt);
    let mut out = Vec::new();
    if id == StyleId::Display && parent {
        out.push((justify.into(), String::new(), Some(
            "v=>[\"flex\",\"grid\",\"none\"].includes(normal(\"display\",String(v??\"\")))?\"auto\":\"start\"".into(),
        )));
    }
    if id == StyleId::JustifyItems && parent {
        // Aligning only for a value that aligns a block child (`start`, `center`, … with its
        // `safe`/`unsafe` forms, `baseline`): anything else is `normal`, `stretch`, a value the
        // justify-items writer refuses (grid.js `gridValue`: a CSS-wide keyword, a legacy form,
        // `last baseline`, a function) or one the browser rejects, all of which align nothing.
        // Comments dropped and spaces collapsed first, as a server render's DOM keeps the string
        // unparsed (dom.js). Known gap: that string's CSS escapes (`c\65 nter`) aren't decoded,
        // and `\s` takes a no-break space for a space; the client's own value corrects both.
        out.push((aligned.into(), String::new(), Some(
            "v=>{const s=normal(\"justify-items\",String(v??\"\")).toLowerCase().replace(/\\/\\*[\\s\\S]*?\\*\\//g,\" \").replace(/\\s+/g,\" \").trim();return /^(?:(?:safe|unsafe) )?(?:start|end|center|self-start|self-end|flex-start|flex-end|left|right)$|^(?:first )?baseline$/.test(s)?\"auto\":\"initial\"}".into(),
        )));
    }
    // A canvas out of flow keeps `start`; a progress stretches between absolute insets.
    let canvas = parts.tag == "canvas";
    if id == StyleId::PositionType && (canvas || parts.props.contains_key("data-exact-progress")) {
        let (out_of_flow, justify) = match canvas {
            true => ("start", canvas_justify(reads_alt)),
            false => ("auto", progress_justify(reads_alt)),
        };
        out.push((
            "justify-self".into(),
            String::new(),
            Some(format!(
            "v=>normal(\"position\",String(v??\"\"))===\"absolute\"?{out_of_flow:?}:{justify:?}"
        )),
        ));
    }
    out
}
