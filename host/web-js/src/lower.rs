//! @ref LLP 1071.001 part B, B5 — enumerated style bindings lowered to
//! classes (~/x2theme/x2p4-b5-plan.md, theme2 07:30 and 07:33).
//!
//! A node whose dynamic style rows are all pure functions of one enum
//! expression compared to literals gets one `Pc` (lower.js) in place of its
//! `S` effects: a cached `className` write of `xa<hash>-<arm>`. The compiler
//! decides the condition, the refusals, each arm's literal and the names;
//! the declarations are the very `S` statements `style_row` wrote, kept as a
//! recipe the page runs once a site an arm, on that arm's literals, into
//! `<style id="exact-arms">` (the same maps and `cssText`, so the same text).
//! Any refusal leaves the node's code byte for byte as `style_row` wrote it.

use super::Em;
use crate::code::Scope;
use exact_plan::{BindingsRow, Opcode};
use exact_runner::vm::instructions;
use std::cell::Cell;
use std::fmt::Write as _;

thread_local! {
    static OFF: Cell<bool> = const { Cell::new(false) };
    /// Each refusal on this thread, for lower_tests.
    #[cfg(test)]
    pub(crate) static REFUSED: std::cell::RefCell<Vec<Refusal>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn note(_why: Refusal) {
    #[cfg(test)]
    REFUSED.with(|r| r.borrow_mut().push(_why));
}

/// Whether lowering is off: the test-only comparison build (the equality
/// test sets `EXACT_JS_NO_LOWER`; nothing documents or forwards it), or a
/// unit test's [`without`].
pub(crate) fn off() -> bool {
    OFF.with(Cell::get) || std::env::var_os("EXACT_JS_NO_LOWER").is_some()
}

/// `f` with lowering off on this thread (unit tests' `--no-lower` build).
#[cfg(test)]
pub(crate) fn without<R>(f: impl FnOnce() -> R) -> R {
    let was = OFF.with(|o| o.replace(true));
    let out = f();
    OFF.with(|o| o.set(was));
    out
}

/// One dynamic style row of a candidate node, as the binding loop wrote it.
pub(super) struct Seen {
    pub b: BindingsRow,
    /// The value function `style_row` was given.
    pub f: String,
    /// Its statements, before the loop's `fm` wrap, and where the wrapped
    /// chunk sits in `out`.
    pub raw: String,
    pub at: usize,
    pub end: usize,
}

/// Why a node keeps its effects (lower_tests names one test each).
#[derive(Debug, PartialEq)]
pub(crate) enum Refusal {
    /// R1: it hears an event, or is a control or link.
    Pressable,
    /// R2: `display` (the grouped separator's `[style*="display:none"]`).
    Display,
    /// R3: a custom property a selector or reader finds in the inline text.
    CustomProperty,
    /// R4: a property a glue writes inline itself, or reads inline (the
    /// runtime's own `scroll-behavior` included), and a live time's widths.
    GlueWritten,
    /// R5: a side path in `style_row` (a mark, `$ink`, `Sx`/`Sm`/`Sr`, a
    /// press or fit hook, an SVG attribute, a painting binding); a
    /// development build's per-binding counter is not one (the key counts).
    SidePath,
    /// R6 is a side path too (`Sx`); R7: a node another module classes.
    ClassOwned,
    /// R8: a string arm outside the safe kinds.
    ValueKind,
    /// R9: `color-scheme`.
    ColorScheme,
    /// A binding of another kind between two of its style rows: its effect
    /// ran between theirs, which one class write cannot keep.
    Interleaved,
    TwoFields,
    NonLiteralArm,
    TooManyArms,
    NonEnum,
}

/// R4: what the glues write inline (glue.js 271/278, textflow-glue.js,
/// paint.js 78, glue.js 631, navigation.js 428/445).
/// R4 too: what the runtime and the glues read from an element's inline
/// style, where a class's value would not be seen (rounds 2 and 3):
/// rt.js 238 (`scroll-behavior`, the agent clock's instant scroll);
/// navigation.js 409 (`display`, `flex-direction`, `position`,
/// `appearance`); navigation.js 1490 `insetAt` (`left`, `top`, `right`,
/// `bottom`, `position-area`); glue.js 349 (`mask-size`);
/// collection-glue.js 475 and geometry-glue.js 65 (`overflow-anchor`);
/// motion-glue.js 283 (`transition`); presence-glue.js 331 (`animation`,
/// an exit appended to it); shared.js 160 (`view-transition-name`);
/// gpu-glue.js 237 (`transform-origin`, saved and put back around a
/// placement). `height`, `position`, `contain`, `visibility`, `isolation`,
/// `touch-action`, `scale`, `transform`, `top` and `left` are in GLUE.
/// Left inline on purpose, not refused: agent.js and inspect-dev.js read a
/// lowered node's arm rule (class-aware); dom.js 116 `font-size` is the
/// render shim's, where lowered rows are written inline; said.js 22
/// `background-color` is read on pressable buttons only (R1);
/// navigation.js 1042 saves and restores `width`/`flex-shrink` around one
/// synchronous measurement; presence-glue.js 314-318 writes an exiting
/// node's box, after which no arm changes it.
const READ: [&str; 11] = [
    "scroll-behavior",
    "appearance",
    "flex-direction",
    "position-area",
    "mask-size",
    "overflow-anchor",
    "transition",
    "view-transition-name",
    "right",
    "bottom",
    "transform-origin",
];
/// `animation` is a motion row, refused on a motion node already (R5);
/// listed for completeness by the read it guards.
const ANIMATION: &str = "animation";
const GLUE: [&str; 12] = [
    "transform",
    "translate",
    "rotate",
    "scale",
    "height",
    "position",
    "contain",
    "visibility",
    "isolation",
    "touch-action",
    "top",
    "left",
];

impl Em<'_> {
    /// Whether node `i` may lower, before its bindings are written: the
    /// element-wide refusals (R1, R5's SVG, motion and symbol nodes, R7).
    pub(super) fn lower_candidate(
        &self,
        i: u32,
        parts: &exact_web::host::template::Parts,
        element: &str,
        heard: bool,
        markdown: bool,
    ) -> bool {
        let plan = self.plan;
        let row = &plan.node(exact_plan::NodesId(i));
        let dynamic = row.bindings.iter().map(|b| plan.binding(b)).any(|b| {
            b.kind == exact_plan::BindingKind::Style
                && crate::style::literal(plan, plan.code(b.expr)).is_none()
        });
        if !dynamic || off() {
            return false;
        }
        let why = self.candidate_refusal(i, parts, element, heard, markdown);
        why.map(note).is_none()
    }

    pub(crate) fn candidate_refusal(
        &self,
        i: u32,
        parts: &exact_web::host::template::Parts,
        element: &str,
        heard: bool,
        markdown: bool,
    ) -> Option<Refusal> {
        const CONTROLS: [&str; 9] = [
            "button", "a", "input", "select", "textarea", "summary", "option", "label", "details",
        ];
        const OWN: [&str; 7] = [
            "data-button-style",
            "data-action",
            "data-native",
            "data-exact-field",
            "data-exact-control",
            "data-exact-switch",
            "data-exact-on",
        ];
        if heard
            || CONTROLS.contains(&element)
            || OWN.iter().any(|k| parts.props.contains_key(*k))
            || parts.css.contains("--exact-press")
        {
            return Some(Refusal::Pressable);
        }
        // A hatched node is page code's (hatches.js 474-485), which may class it.
        if markdown
            || parts.props.contains_key("contenteditable")
            || parts.props.contains_key("class")
            || parts.props.contains_key("data-hatch")
        {
            return Some(Refusal::ClassOwned);
        }
        // A live time's widths are its glue's to read inline (live-time-glue.js 31-39).
        if element == "time" {
            return Some(Refusal::GlueWritten);
        }
        let svg =
            exact_kernel::NodeType::from_wire(self.plan.node(exact_plan::NodesId(i)).node_type)
                .is_none_or(|t| t == exact_kernel::NodeType::Svg || t.is_svg_element());
        (svg || self.is_motion_node(i) || self.in_symbol(i)).then_some(Refusal::SidePath)
    }

    /// After node `i`'s binding loop: its style rows `seen` become one `Pc`
    /// where every condition holds; otherwise `out` is left as written.
    pub(super) fn lower(
        &mut self,
        e: &str,
        class: &str,
        parts: &exact_web::host::template::Parts,
        scope: &Scope,
        seen: Vec<Seen>,
    ) -> Result<(), String> {
        // A tinted image's source stamps its template rows inline (symbols.js `template`).
        let tinted = parts.tag == "img"
            && (parts.css.contains("--exact-tint")
                || seen.iter().any(|s| s.raw.contains("\"--exact-tint\"")));
        let arms = match self.arms(e, &seen, tinted) {
            Ok(arms) => arms,
            Err(why) => {
                note(why);
                return Ok(());
            }
        };
        let Arms {
            key,
            lits,
            values,
            recipe,
            props,
        } = arms;
        let key = crate::code::expression(self.plan, &key, scope, &mut self.uses)?;
        let site = e.trim_start_matches('e');
        let pick: String = lits
            .iter()
            .enumerate()
            .map(|(k, l)| format!("v==={}?{k}:", l.js()))
            .collect();
        let values: Vec<String> = values
            .iter()
            .map(|v| format!("[{}]", v.join(",")))
            .collect();
        let values = format!("[{}]", values.join(","));
        let props = serde_json::to_string(&props).unwrap();
        let statics = if class == "0" {
            String::new()
        } else {
            format!("c{class} ")
        };
        // The name is the arm set's content: its recipe, literals and every
        // module constant the recipe reads, so a build or a dev generation
        // that changes any of them names new rules (theme2 07:33 condition 7),
        // in two FNV-1a 64s (128 bits); lower.js also checks the content.
        let mut text = recipe.clone();
        text.push('\0');
        text.push_str(&values);
        for name in constants(&recipe) {
            text.push('\0');
            text.push_str(self.uses.consts.get(name).map_or("", String::as_str));
        }
        let name = format!("xa{}", digest(&text));
        let pc = self.uses.rt("Pc");
        let s = self.uses.rt("S");
        let text = format!(
            "{pc}({e},{},{statics:?},\"{name}\",{values},{props},({s},$b,$e)=>{{{recipe}}});",
            match self.site_attrs {
                true => format!(
                    "{}({site},()=>(v=>{pick}{})({key}))",
                    self.uses.rt("pf"),
                    lits.len()
                ),
                false => format!("()=>(v=>{pick}{})({key})", lits.len()),
            }
        );
        // The one `Pc` where the rows were (contiguous: `Interleaved`
        // refuses another binding between them), then the rows' chunks out.
        let text = match crate::reads::field_mask(&text) {
            Some(m) => format!("{}({m},()=>{{{text}}});", self.uses.rt("fm")),
            None => text,
        };
        self.out.insert_str(seen[seen.len() - 1].end, &text);
        for c in seen.iter().rev() {
            self.out.replace_range(c.at..c.end, "");
        }
        Ok(())
    }

    /// The arms of `seen`, or why there are none.
    pub(crate) fn arms(&self, e: &str, seen: &[Seen], tinted: bool) -> Result<Arms, Refusal> {
        let plan = self.plan;
        let mut recipe = String::new();
        let mut props = Vec::new();
        let mut key: Option<Vec<u8>> = None;
        let mut lits: Vec<Lit> = Vec::new();
        if seen.windows(2).any(|w| w[0].end != w[1].at) {
            return Err(Refusal::Interleaved);
        }
        for (k, s) in seen.iter().enumerate() {
            let at = format!("S({e},");
            // Each row's own statements only, every one an `S` of this node
            // with this binding's value function in it.
            // What it writes first (R2, R3, R4, R9), then how (R5).
            for (pos, _) in s.raw.match_indices(&at) {
                s.raw[pos + at.len()..]
                    .strip_prefix('"')
                    .and_then(|r| r.split('"').next())
                    .map_or(Ok(()), |p| refuse_prop(p, tinted))?;
            }
            if s.f.is_empty() || !s.raw.contains(&s.f) {
                return Err(Refusal::SidePath);
            }
            // A development build's counter (rows.rs `counted`) counts the key instead.
            let counter = format!("const {}=", s.f);
            for st in statements(&s.raw)
                .ok_or(Refusal::SidePath)?
                .into_iter()
                .filter(|st| !st.starts_with(&counter))
            {
                let st = &st.replace(&s.f, &format!("()=>$b[{k}]"));
                let rest = st.strip_prefix(&at).ok_or(Refusal::SidePath)?;
                let prop = rest
                    .strip_prefix('"')
                    .and_then(|r| r.split('"').next())
                    .ok_or(Refusal::SidePath)?;
                if names(rest, e) || st.contains(".$ink") {
                    return Err(Refusal::SidePath);
                }
                refuse_prop(prop, tinted)?;
                props.push(prop.to_string());
                let _ = write!(recipe, "S($e,{rest}");
            }
            let code = plan.code(s.b.expr);
            collect(plan, code, &mut key, &mut lits)?;
        }
        if lits.len() > 15 {
            return Err(Refusal::TooManyArms);
        }
        if lits.is_empty() {
            return Err(Refusal::NonEnum);
        }
        // Each row's value on each arm: the literals, then the rest.
        let mut values = Vec::new();
        for arm in lits.iter().map(Some).chain([None]) {
            let mut v = Vec::new();
            for s in seen {
                let got = run(plan, plan.code(s.b.expr), arm, &mut key, &mut lits.clone())?;
                // `revert-layer` in an author rule rolls back unlayered author
                // rules too, where inline it rolls back the inline row (CSS
                // Cascade 5 §7.3.2); `revert`, `inherit`, `initial` and `unset`
                // mean the same in both (Cascade 4 §7.3, both author origin).
                if let Some(Lit::S(text)) = &got {
                    if !safe_text(text) {
                        return Err(Refusal::ValueKind);
                    }
                }
                v.push(got.map_or("null".into(), |l| l.js()));
            }
            values.push(v);
        }
        let mut code = key.ok_or(Refusal::NonEnum)?;
        code.push(Opcode::Return as u8);
        Ok(Arms {
            key: code,
            lits,
            values,
            recipe,
            props,
        })
    }
}

/// A lowered node's parts, as `lower` writes them.
pub(crate) struct Arms {
    pub key: Vec<u8>,
    pub lits: Vec<Lit>,
    pub values: Vec<Vec<String>>,
    pub recipe: String,
    pub props: Vec<String>,
}

/// R8: a string arm every engine parses alike (hex, keywords, families,
/// numbers), and not `revert-layer`, whose author-rule meaning is not its
/// inline one (CSS Cascade 5 §7.3.2; the contract refuses it upstream too).
pub(crate) fn safe_text(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || " #.,%_-".contains(c))
        && !text
            .split([' ', ','])
            .any(|w| w.eq_ignore_ascii_case("revert-layer"))
}

/// A literal the key is compared with, as JavaScript's `===` sees it: -0 is
/// 0; a NaN equals nothing, so its arm would be dead (refused).
pub(crate) fn compared(l: Lit) -> Result<Lit, Refusal> {
    match l {
        Lit::N(bits) if f64::from_bits(bits).is_nan() => Err(Refusal::ValueKind),
        Lit::N(bits) => Ok(Lit::N((f64::from_bits(bits) + 0.0).to_bits())),
        l => Ok(l),
    }
}

/// R4 on a tinted image: what symbols.js `template` stamps inline when its
/// source changes (symbols.js 143, 151-156).
const TEMPLATE: [&str; 8] = [
    "background-color",
    "mask-image",
    "mask-size",
    "mask-repeat",
    "mask-position",
    "mask-origin",
    "mask-clip",
    "object-position",
];

pub(crate) fn refuse_prop(prop: &str, tinted: bool) -> Result<(), Refusal> {
    match prop {
        "display" => Err(Refusal::Display),
        "color-scheme" => Err(Refusal::ColorScheme),
        p if GLUE.contains(&p)
            || READ.contains(&p)
            || p == ANIMATION
            || tinted && TEMPLATE.contains(&p) =>
        {
            Err(Refusal::GlueWritten)
        }
        // The type companions are read only through `var()`.
        p if p.starts_with("--")
            && !p.starts_with("--exact-said-")
            && !p.starts_with("--exact-style-") =>
        {
            Err(Refusal::CustomProperty)
        }
        _ => Ok(()),
    }
}

/// `text`'s top-level `S(…);` statements, or `None` where it is anything else.
fn statements(text: &str) -> Option<Vec<&str>> {
    let b = text.as_bytes();
    let (mut out, mut start, mut i, mut depth) = (Vec::new(), 0, 0, 0i32);
    let mut quote: Vec<u8> = Vec::new();
    while i < b.len() {
        let c = b[i];
        match quote.last().copied() {
            Some(q) => {
                if c == b'\\' {
                    i += 1;
                } else if q == b'`' && c == b'$' && b.get(i + 1) == Some(&b'{') {
                    quote.push(b'}');
                    i += 1;
                } else if c == q {
                    quote.pop();
                } else if q == b'}' && (c == b'"' || c == b'`') {
                    quote.push(c);
                }
            }
            None => match c {
                b'"' | b'`' | b'\'' => quote.push(c),
                b'\\' => i += 1,
                b'(' | b'{' | b'[' => depth += 1,
                b')' | b'}' | b']' => depth -= 1,
                b';' if depth == 0 => {
                    out.push(&text[start..=i]);
                    start = i + 1;
                }
                _ => {}
            },
        }
        if depth < 0 {
            return None;
        }
        i += 1;
    }
    (start == b.len() && depth == 0 && quote.is_empty() && !out.is_empty()).then_some(out)
}

/// Whether `text` names the identifier `e` (the node's element) itself.
fn names(text: &str, e: &str) -> bool {
    let word =
        |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    text.match_indices(e).any(|(at, _)| {
        !word(text[..at].chars().next_back()) && !word(text[at + e.len()..].chars().next())
    })
}

/// The module constants (`$cm`, `$$m0` …) `recipe` names.
fn constants(recipe: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let b = recipe.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$'
            && (i == 0
                || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'$'))
        {
            let end = (i..b.len())
                .find(|&j| !(b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'$'))
                .unwrap_or(b.len());
            if !out.contains(&&recipe[i..end]) {
                out.push(&recipe[i..end]);
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// Two FNV-1a 64s of `text` (different offset bases), in base 36.
fn digest(text: &str) -> String {
    let fnv = |basis: u64| {
        text.bytes()
            .fold(basis, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
    };
    // Each half its full 13 digits, so two splits never read alike.
    format!(
        "{:0>13}{:0>13}",
        base36(fnv(0xcbf29ce484222325)),
        base36(fnv(0x84222325cbf29ce4))
    )
}

fn base36(mut n: u64) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// A literal an arm compares or gives.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Lit {
    S(String),
    N(u64),
    B(bool),
}

impl Lit {
    pub(crate) fn js(&self) -> String {
        match self {
            Lit::S(s) => serde_json::to_string(s).unwrap(),
            Lit::N(bits) => crate::code::number(f64::from_bits(*bits)),
            Lit::B(b) => b.to_string(),
        }
    }
}

#[derive(Clone)]
enum V {
    Key,
    Lit(Lit),
    Null,
}

const READS: [Opcode; 6] = [
    Opcode::LoadItem,
    Opcode::LoadParam,
    Opcode::LoadSlot,
    Opcode::LoadDerive,
    Opcode::LoadBound,
    Opcode::LoadResource,
];

/// Every literal `code` compares its key with, run to a fixed point over
/// the arms it finds (a comparison under another arm's branch included).
fn collect(
    plan: &exact_plan::Plan,
    code: &[u8],
    key: &mut Option<Vec<u8>>,
    lits: &mut Vec<Lit>,
) -> Result<(), Refusal> {
    let mut done = 0;
    run(plan, code, None, key, lits)?;
    while done < lits.len() {
        let arm = lits[done].clone();
        run(plan, code, Some(&arm), key, lits)?;
        done += 1;
        if lits.len() > 15 {
            return Err(Refusal::TooManyArms);
        }
    }
    Ok(())
}

/// `code`'s value when its key is `arm` (`None`: no literal it names),
/// noting the literals it compares the key with.
fn run(
    plan: &exact_plan::Plan,
    code: &[u8],
    arm: Option<&Lit>,
    key: &mut Option<Vec<u8>>,
    lits: &mut Vec<Lit>,
) -> Result<Option<Lit>, Refusal> {
    let ins: Vec<_> = instructions(code)
        .collect::<Result<_, _>>()
        .map_err(|_| Refusal::NonLiteralArm)?;
    let (mut stack, mut locals, mut at, mut steps): (Vec<V>, Vec<V>, usize, usize) =
        (Vec::new(), Vec::new(), 0, 0);
    let goto = |pc: u64| {
        ins.iter()
            .position(|i| i.pc as u64 == pc)
            .ok_or(Refusal::NonLiteralArm)
    };
    while at < ins.len() {
        steps += 1;
        if steps > 10_000 {
            return Err(Refusal::NonLiteralArm);
        }
        let i = ins[at];
        at += 1;
        let pop = |s: &mut Vec<V>| s.pop().ok_or(Refusal::NonLiteralArm);
        match i.op {
            op if READS.contains(&op) => {
                let mut end = at;
                while end < ins.len() && ins[end].op == Opcode::Field {
                    end += 1;
                }
                let bytes = code[i.pc..ins.get(end).map_or(code.len(), |n| n.pc)].to_vec();
                match key {
                    Some(k) if *k != bytes => return Err(Refusal::TwoFields),
                    _ => *key = Some(bytes),
                }
                at = end;
                stack.push(V::Key);
            }
            Opcode::Number => stack.push(V::Lit(Lit::N(i.number.to_bits()))),
            Opcode::Bool => stack.push(V::Lit(Lit::B(i.args[0] != 0))),
            Opcode::Str => stack.push(V::Lit(Lit::S(
                plan.str(exact_plan::StrId(i.args[0] as u32)).to_string(),
            ))),
            Opcode::None => stack.push(V::Null),
            // An option's `some(x)` is `x` in the JS program (code.rs), a literal still.
            Opcode::Some => match stack.last() {
                Some(V::Lit(_)) => {}
                _ => return Err(Refusal::NonLiteralArm),
            },
            Opcode::Eq | Opcode::Ne => {
                let (b, a) = (pop(&mut stack)?, pop(&mut stack)?);
                let same = match (a, b) {
                    // JavaScript's `===`, as the pick compares.
                    (V::Key, V::Lit(l)) | (V::Lit(l), V::Key) => {
                        let l = compared(l)?;
                        if !lits.contains(&l) {
                            lits.push(l.clone());
                        }
                        arm == Some(&l)
                    }
                    (V::Lit(Lit::N(a)), V::Lit(Lit::N(b))) => {
                        f64::from_bits(a) == f64::from_bits(b)
                    }
                    (V::Lit(a), V::Lit(b)) => a == b,
                    (V::Key, _) | (_, V::Key) => return Err(Refusal::NonEnum),
                    _ => return Err(Refusal::NonLiteralArm),
                };
                stack.push(V::Lit(Lit::B(same == (i.op == Opcode::Eq))));
            }
            Opcode::Not => match pop(&mut stack)? {
                V::Lit(Lit::B(b)) => stack.push(V::Lit(Lit::B(!b))),
                V::Key => return Err(Refusal::NonEnum),
                _ => return Err(Refusal::NonLiteralArm),
            },
            Opcode::JumpIfFalse => match pop(&mut stack)? {
                V::Lit(Lit::B(true)) => {}
                V::Lit(Lit::B(false)) => at = goto(i.args[0])?,
                V::Key => return Err(Refusal::NonEnum),
                _ => return Err(Refusal::NonLiteralArm),
            },
            Opcode::Jump => at = goto(i.args[0])?,
            Opcode::BindLocal => {
                let v = pop(&mut stack)?;
                locals.push(v);
            }
            Opcode::LoadLocal => stack.push(
                locals
                    .get(i.args[0] as usize)
                    .cloned()
                    .ok_or(Refusal::NonLiteralArm)?,
            ),
            Opcode::DropLocal => {
                locals.pop().ok_or(Refusal::NonLiteralArm)?;
            }
            Opcode::Pop => {
                pop(&mut stack)?;
            }
            Opcode::Return => {
                return match pop(&mut stack)? {
                    V::Lit(l) => Ok(Some(l)),
                    V::Null => Ok(None),
                    V::Key => Err(Refusal::NonLiteralArm),
                };
            }
            _ => return Err(Refusal::NonLiteralArm),
        }
    }
    Err(Refusal::NonLiteralArm)
}
