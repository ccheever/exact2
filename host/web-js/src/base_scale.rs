//! @ref LLP 1121 stage 11 B (B2) — the base scale on the JS target, which
//! keeps no kernel at run time. Its one look is the neutral one (the build
//! says so, `roles::set_column`), so its column is the kernel's in this
//! build (`metrics::column()`):
//! - a static row naming a base name is that column's length, as the kernel
//!   reads the literal while the sheet is projected (`theme::measure`);
//! - the sheet declares every base name the design does not define, once
//!   as a custom property on `:root`, `--exact-<name>` (`--exact-space-md`),
//!   from the kernel's table, never a copied number, when the plan may read
//!   any base name as a length, and none when it can't ([`named`]: theme2's
//!   all-nine-or-none, B2 round 2);
//! - a bound row's value naming one reads that property (`measures::map`),
//!   an SVG attribute its length;
//! - a length token over base names (stage 12 B2) is a `calc()` over their
//!   properties (`calc(var(--exact-space-md) * 2)`, [`expression`]), and in
//!   an SVG attribute that column's px ([`folded`]);
//! - a shadow token whose lengths are over base names (stage 12 B3) is the
//!   row's CSS with each such length its property or `calc()` ([`shadow`]).
//!
//! A design that defines a base name makes it its own token, which the
//! token path writes (`modes`, `measures`). The web has no iOS or Mac look
//! class to declare a second column under; one would add its rules here.

use exact_kernel::style::{metrics, scale};
use exact_kernel::StyleId;
use exact_plan::{BindingKind, Opcode, Plan};
use std::collections::BTreeSet;

/// The base names the sheet declares and the maps hold: every one the
/// design does not define when the plan may read any base name as a length,
/// none when it can't (theme2, B2 round 2: declaring all nine is safe,
/// missing one is not, and a switch between two base names then never needs
/// a property the sheet lacks). It may read one when:
/// - a length token aliases one (`card.pad: length = space.md`), or is an
///   expression over one (`calc(space.md * 2)`, stage 12 B2), or a shadow
///   token's length is (`0 space.md 4px black`, B3);
/// - a length row's value may be one, through `?:`, `match` and `let` (its
///   bytecode's string constants, trimmed as the page's lookup trims,
///   [`returns`]);
/// - a length row's value may be any string: a string read (a state, a
///   field, data), a string call, a concatenation not proven base-free.
///
/// A text that says `space.md` reads none, so a release that uses no base
/// name is unchanged by the scale.
pub fn named(plan: &Plan) -> Vec<&'static str> {
    let at = plan as *const Plan as usize;
    if let Some(kept) = KEPT.with(|k| {
        k.borrow()
            .as_ref()
            .filter(|(p, _)| *p == at)
            .map(|(_, v)| v.clone())
    }) {
        return kept;
    }
    reads(plan)
}

thread_local! {
    /// [`named`]'s answer for the plan an emit is writing ([`keep`]).
    static KEPT: std::cell::RefCell<Option<(usize, Vec<&'static str>)>> = const { std::cell::RefCell::new(None) };
}

/// [`named`] for `plan` asked once and kept while the guard lives: an emit
/// asks it once a bound row (`measures::map`, `map_name`).
pub fn keep(plan: &Plan) -> Kept {
    let names = reads(plan);
    Kept(KEPT.with(|k| k.replace(Some((plan as *const Plan as usize, names)))))
}

/// [`keep`]'s guard: what was kept before, restored.
pub struct Kept(Option<(usize, Vec<&'static str>)>);

impl Drop for Kept {
    fn drop(&mut self) {
        KEPT.with(|k| *k.borrow_mut() = self.0.take());
    }
}

/// [`named`], unkept.
fn reads(plan: &Plan) -> Vec<&'static str> {
    let mut every = false;
    let mut said: BTreeSet<&str> = BTreeSet::new();
    for row in &plan.theme {
        use exact_kernel::style::theme::Kind;
        if !matches!(
            Kind::of(row.kind),
            Some(Kind::Length | Kind::BoxShadow | Kind::TextShadow)
        ) {
            continue;
        }
        let slices = plan.theme_slices[row.slices.start as usize..][..row.slices.len as usize]
            .iter()
            .map(|s| s.value);
        for v in std::iter::once(row.value).chain(slices) {
            let v = js_trim(plan.str(v));
            said.insert(v);
            said.extend(scale::named_in(v));
        }
    }
    // Each binding's node's arm: the regions whose frames its code reads
    // (`LoadItem`, `LoadBound`), innermost first.
    let mut arms: Vec<Option<exact_plan::ArmsId>> = vec![None; plan.bindings.len()];
    let looks = plan
        .looks
        .iter()
        .map(|l| (l.bindings, plan.nodes.get(l.node.0 as usize).unwrap().arm));
    for (range, arm) in plan.nodes.iter().map(|n| (n.bindings, n.arm)).chain(looks) {
        for b in range.iter() {
            arms[b.0 as usize] = arm;
        }
    }
    for (b, arm) in plan.bindings.iter().zip(arms) {
        let length = b.kind == BindingKind::Style
            && StyleId::from_bit(u32::from(b.id)).is_some_and(reads_length);
        if !length {
            continue;
        }
        match returns(plan, plan.code(b.expr), arm) {
            Some(strings) => said.extend(
                strings
                    .into_iter()
                    .map(|id| js_trim(plan.str(exact_plan::StrId(id)))),
            ),
            None => every = true,
        }
    }
    let reads = every || scale::names().any(|n| said.contains(n));
    scale::names()
        .filter(|_| reads)
        .filter(|n| !plan.theme.iter().any(|r| plan.str(r.name) == *n))
        .collect()
}

/// Whether row `id`'s codec reads a length: a base name the JS target
/// resolves only there. The kernel's `theme::admits` takes a length token on
/// every row that is no colour's or number's, keyword rows too
/// (`color-scheme`, `font-style`, `dynamic-range-limit`), where the codec
/// then refuses the length the name reads: so on such a row a base name is
/// refused either way, and counting it would declare the scale in an app
/// that never uses it (B2 round 1, theme2: a release unchanged by the
/// scale). Asked of the codec itself, with `1px`.
pub fn reads_length(id: StyleId) -> bool {
    use exact_kernel::style::theme::{admits, Kind};
    if !admits(Kind::Length, id) {
        return false;
    }
    let mut probe = exact_kernel::StyleProps::default();
    probe
        .set_dynamic(id, &exact_kernel::StyleValue::Text("1px".into()))
        .is_ok()
        && probe.mask.has(id)
}

/// `text` trimmed as the page's lookup trims it (JavaScript's
/// `String.prototype.trim`): its WhiteSpace (tab, vertical tab, form feed,
/// U+FEFF and every space separator) and LineTerminator (LF, CR, U+2028,
/// U+2029) code points. Rust's `trim` differs: it keeps U+FEFF and strips
/// U+0085 (round 2's re-review, Astra).
fn js_trim(text: &str) -> &str {
    let js_space = |c: char| {
        matches!(
            c,
            '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}'
                    | '\u{2028}'
                    | '\u{2029}'
                    | '\u{202f}'
                    | '\u{205f}'
                    | '\u{3000}'
                    | '\u{feff}'
        )
    };
    text.trim_matches(js_space)
}

/// An abstract value of [`returns`]: the string constants it may be, or
/// any string (`unknown`: read or computed), and its plan type where a read
/// says it (a record's, for `Field`). No string and not unknown is a value
/// that is no base name: a number, a bool, a record, or a string with a part
/// no base name holds (`10px`).
#[derive(Clone, Default, PartialEq)]
struct Value {
    strings: BTreeSet<u32>,
    unknown: bool,
    ty: Option<exact_plan::TypesId>,
    /// A constructed option's or list's element (`Some(x)`, `[12, 16]`),
    /// which a region's frame holds (round 2, Astra 5).
    item: Option<Box<Value>>,
    /// No value at all: `none`'s payload, which a join takes as the other's.
    nothing: bool,
}

impl Value {
    /// No value at all (`none`'s payload): what any join leaves as the other.
    fn nothing() -> Value {
        Value {
            nothing: true,
            ..Value::default()
        }
    }
    fn unknown() -> Value {
        Value {
            unknown: true,
            ..Value::default()
        }
    }
    /// A read of plan type `ty`: a string (or an option of one) may be any.
    fn typed(plan: &Plan, ty: exact_plan::TypesId) -> Value {
        use exact_plan::TypeKind;
        let row = &plan.types[ty.0 as usize];
        let mut v = match row.kind {
            TypeKind::String => Value::unknown(),
            TypeKind::Option => match row.elem {
                Some(elem) => Value::typed(plan, elem),
                None => Value::unknown(),
            },
            _ => Value::default(),
        };
        v.ty = Some(ty);
        v
    }
    fn join(&mut self, other: &Value) {
        if other.nothing {
            return;
        }
        if self.nothing {
            *self = other.clone();
            return;
        }
        self.strings.extend(&other.strings);
        self.unknown |= other.unknown;
        if self.ty != other.ty {
            self.ty = None;
        }
        self.item = match (self.item.take(), &other.item) {
            (Some(mut a), Some(b)) => {
                a.join(b);
                Some(a)
            }
            // One way constructed, another not: its element is unknown.
            (Some(_), None) | (None, Some(_)) => Some(Box::new(Value::unknown())),
            (None, None) => None,
        };
    }
    /// Whether every string it may be holds a part no base name holds, so
    /// no concatenation with it is one (a digit, `px`, `%`).
    /// Trimmed, as the page's lookup trims ([`js_trim`]): a part of only
    /// whitespace proves nothing (round 2, Astra 2 and Grok 1). No string
    /// and not unknown, in a concatenation, is an earlier one proven
    /// base-free.
    fn no_base(&self, plan: &Plan) -> bool {
        !self.unknown
            && self.strings.iter().all(|id| {
                let text = js_trim(plan.str(exact_plan::StrId(*id)));
                !text.is_empty() && !scale::names().any(|n| n.contains(text))
            })
    }
}

/// The string constants (pool ids) `code` may return, when its result is a
/// literal through its branches (`?:`, `match`, `let`; a condition may read
/// anything); `None` when it may be any string: read (a state, a param,
/// data), computed (`Concat`, a call), or code this reading does not follow.
/// A literal's leaves as lowering's `literal` reads them, on the bytecode.
fn returns(plan: &Plan, code: &[u8], arm: Option<exact_plan::ArmsId>) -> Option<BTreeSet<u32>> {
    let mut out = walk(plan, code, arm, 0)?;
    // An option a row is bound to is its value or none: its element counts.
    while let Some(item) = out.item.take() {
        out.strings.extend(&item.strings);
        out.unknown |= item.unknown;
        out.item = item.item;
    }
    (!out.unknown).then_some(out.strings)
}

/// The region whose frame `depth` frames out of `arm` is (every region
/// pushes one: a `when`'s, a `match`'s, an `each`'s), when the plan says.
fn region_at(
    plan: &Plan,
    arm: Option<exact_plan::ArmsId>,
    depth: u64,
) -> Option<&exact_plan::RegionsRow> {
    let mut region = &plan.regions[plan.arms.get(arm?.0 as usize)?.region.0 as usize];
    for _ in 0..depth {
        region = &plan.regions[plan.arms.get(region.arm?.0 as usize)?.region.0 as usize];
    }
    Some(region)
}

/// What a frame `depth` out of `arm` holds, typed by its region's subject:
/// an `each`'s item, the elem of its list; a `match`'s bound value, the
/// elem of its option. Unknown where the plan does not say (`nested` caps a
/// subject read through other frames).
fn frame_value(
    plan: &Plan,
    arm: Option<exact_plan::ArmsId>,
    depth: u64,
    kind: exact_plan::RegionKind,
    nested: u8,
) -> Value {
    let elem = || {
        let region = region_at(plan, arm, depth).filter(|r| r.kind == kind)?;
        let subject = walk(plan, plan.code(region.subject), region.arm, nested + 1)?;
        // A constructed list or option (`[12, 16]`, `Some(size)`): its
        // element as built (round 2, Astra 5).
        if let Some(item) = subject.item {
            return Some(Err(*item));
        }
        let t = &plan.types[subject.ty?.0 as usize];
        let wants = match kind {
            exact_plan::RegionKind::Each => exact_plan::TypeKind::List,
            _ => exact_plan::TypeKind::Option,
        };
        (t.kind == wants).then_some(t.elem).flatten().map(Ok)
    };
    match (nested < 8).then(elem).flatten() {
        Some(Ok(elem)) => Value::typed(plan, elem),
        Some(Err(built)) => built,
        None => Value::unknown(),
    }
}

/// [`returns`]' walk: the joined value `code` may return, its type where
/// every way says one.
fn walk(plan: &Plan, code: &[u8], arm: Option<exact_plan::ArmsId>, nested: u8) -> Option<Value> {
    let program: Vec<_> = exact_runner::vm::instructions(code)
        .collect::<Result<_, _>>()
        .ok()?;
    let at = |pc: usize| program.iter().position(|i| i.pc == pc);
    let mut seen: Vec<Option<(Vec<Value>, Vec<Value>)>> = vec![None; program.len()];
    let mut work = vec![0usize];
    seen[0] = Some((Vec::new(), Vec::new()));
    let mut out: Option<Value> = None;
    // Each instruction's state joined from every way in; one that grows is
    // walked again (a finite lattice: the body's constants).
    while let Some(i) = work.pop() {
        let (mut stack, mut locals) = seen[i].clone()?;
        let ins = program[i];
        let next = i + 1;
        let mut to: Vec<usize> = vec![next];
        let pop = |stack: &mut Vec<Value>| stack.pop();
        match ins.op {
            Opcode::Number | Opcode::Bool | Opcode::Unit => stack.push(Value::default()),
            // `none`: an option with no payload, which joins with a `Some`'s
            // to that one's element (round 2's re-review, Astra).
            Opcode::None => stack.push(Value {
                item: Some(Box::new(Value::nothing())),
                ..Value::default()
            }),
            Opcode::LoadIndex
            | Opcode::PendingResource
            | Opcode::PendingMutation
            | Opcode::FailedResource
            | Opcode::FailedMutation => stack.push(Value::default()),
            Opcode::Str => stack.push(Value {
                strings: [ins.args[0] as u32].into(),
                ..Value::default()
            }),
            Opcode::Some => {
                let v = pop(&mut stack)?;
                stack.push(Value {
                    item: Some(Box::new(v)),
                    ..Value::default()
                });
            }
            // A list literal: its items' join (round 2, Astra 5).
            Opcode::List => {
                let mut item: Option<Value> = None;
                for _ in 0..ins.args[0] {
                    let v = pop(&mut stack)?;
                    match &mut item {
                        Some(i) => i.join(&v),
                        None => item = Some(v),
                    }
                }
                stack.push(Value {
                    item: Some(Box::new(item.unwrap_or_default())),
                    ..Value::default()
                });
            }
            Opcode::Unwrap => {
                let v = pop(&mut stack)?;
                let elem = v.ty.map(|t| &plan.types[t.0 as usize]).and_then(|t| {
                    (t.kind == exact_plan::TypeKind::Option)
                        .then_some(t.elem)
                        .flatten()
                });
                stack.push(match (v.item, elem) {
                    (Some(item), _) => *item,
                    (None, Some(elem)) => Value::typed(plan, elem),
                    (None, None) => Value {
                        ty: None,
                        item: None,
                        ..v
                    },
                });
            }
            // A read says its type: a string one may be any (a state an
            // action sets to `"space." + size`), another is no base name.
            Opcode::LoadSlot => {
                stack.push(Value::typed(plan, plan.slots.get(ins.args[0] as usize)?.ty))
            }
            Opcode::LoadDerive => stack.push(Value::typed(
                plan,
                plan.derives.get(ins.args[0] as usize)?.ty,
            )),
            Opcode::LoadResource => stack.push(Value::typed(
                plan,
                plan.resources.get(ins.args[0] as usize)?.ty,
            )),
            Opcode::LoadItem => stack.push(frame_value(
                plan,
                arm,
                ins.args[0],
                exact_plan::RegionKind::Each,
                nested,
            )),
            Opcode::LoadBound => stack.push(frame_value(
                plan,
                arm,
                ins.args[0],
                exact_plan::RegionKind::Match,
                nested,
            )),
            Opcode::LoadParam | Opcode::FailureResource | Opcode::FailureMutation => {
                stack.push(Value::unknown())
            }
            Opcode::Field => {
                let v = pop(&mut stack)?;
                let record =
                    v.ty.map(|t| &plan.types[t.0 as usize])
                        .filter(|t| t.kind == exact_plan::TypeKind::Record);
                stack.push(match record {
                    Some(t) if (ins.args[0] as u32) < t.fields.len => Value::typed(
                        plan,
                        plan.fields[(t.fields.start + ins.args[0] as u32) as usize].ty,
                    ),
                    _ => Value::unknown(),
                });
            }
            Opcode::Call => {
                let f = exact_plan::Stdlib::from_wire(ins.args[0] as u8)?;
                for _ in 0..f.arity() {
                    pop(&mut stack)?;
                }
                let returns = f.returns();
                stack.push(
                    if returns == "string"
                        || returns == "any"
                        || returns.starts_with("option<string")
                    {
                        Value::unknown()
                    } else {
                        Value::default()
                    },
                );
            }
            Opcode::Neg | Opcode::Not => {
                pop(&mut stack)?;
                stack.push(Value::default());
            }
            Opcode::Add
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::Div
            | Opcode::Rem
            | Opcode::Eq
            | Opcode::Ne
            | Opcode::Lt
            | Opcode::Le
            | Opcode::Gt
            | Opcode::Ge => {
                pop(&mut stack)?;
                pop(&mut stack)?;
                stack.push(Value::default());
            }
            // A string with a part no base name holds (`size + "px"`) is none.
            Opcode::Concat => {
                let (b, a) = (pop(&mut stack)?, pop(&mut stack)?);
                stack.push(if a.no_base(plan) || b.no_base(plan) {
                    Value::default()
                } else {
                    Value::unknown()
                });
            }
            Opcode::Pop => {
                pop(&mut stack)?;
            }
            Opcode::BindLocal => locals.push(pop(&mut stack)?),
            Opcode::LoadLocal => stack.push(locals.get(ins.args[0] as usize)?.clone()),
            Opcode::DropLocal => {
                locals.pop()?;
            }
            Opcode::Jump => to = vec![at(ins.args[0] as usize)?],
            Opcode::JumpIfFalse => {
                pop(&mut stack)?;
                to.push(at(ins.args[0] as usize)?);
            }
            Opcode::JumpIfNone => to.push(at(ins.args[0] as usize)?),
            Opcode::Return => {
                let top = stack.last()?;
                match &mut out {
                    Some(out) => out.join(top),
                    None => out = Some(top.clone()),
                }
                to.clear();
            }
            _ => return None,
        }
        for t in to {
            if t >= program.len() {
                return None;
            }
            match &mut seen[t] {
                Some((s, l)) => {
                    if s.len() != stack.len() || l.len() != locals.len() {
                        return None;
                    }
                    let before = (s.clone(), l.clone());
                    s.iter_mut().zip(&stack).for_each(|(a, b)| a.join(b));
                    l.iter_mut().zip(&locals).for_each(|(a, b)| a.join(b));
                    if (s.clone(), l.clone()) != before {
                        work.push(t);
                    }
                }
                slot => {
                    *slot = Some((stack.clone(), locals.clone()));
                    work.push(t);
                }
            }
        }
    }
    out
}

/// A length token's expression over base names (stage 12 B2) as the CSS
/// the page writes: each base name its property, inside the expression's
/// own `calc()`, `min()`, `max()` or `clamp()`. `None` for any other text.
pub fn expression(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.contains('(') || scale::named_in(value).is_empty() {
        return None;
    }
    scale::substitute_with(value, |name| Some(format!("var({})", property(name))))
}

/// [`expression`]'s px in this build's column, where no property is read
/// (an SVG attribute).
pub fn folded(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.contains('(') || scale::named_in(value).is_empty() {
        return None;
    }
    let px = scale::evaluate(value, metrics::column())?;
    Some(format!("{px}px"))
}

/// A shadow token's text whose lengths are over base names (stage 12 B3) as
/// the CSS the page writes: the row's CSS for the text this build's column
/// folds it to, as css.rs writes a literal's (its colours, every length
/// said), each length written over a base name then its property's `var()`
/// or [`expression`]'s `calc()` over them, for the browser to fold. `None`
/// for any other text, or a kind no shadow is.
pub fn shadow(kind: exact_kernel::style::theme::Kind, value: &str) -> Option<String> {
    use exact_kernel::style::theme::Kind;
    let value = value.trim();
    if !matches!(kind, Kind::BoxShadow | Kind::TextShadow) || scale::named_in(value).is_empty() {
        return None;
    }
    let px = scale::fold_lengths(value, |n| scale::of(n, metrics::column()).map(scale::css))?;
    // The row's CSS writes `inset`, then every length (a box shadow's four,
    // a text shadow's three), then the colour.
    let css = match kind {
        Kind::BoxShadow => exact_kernel::style::BoxShadows::parse(&px)?.css(),
        _ => exact_kernel::style::TextShadow::parse(&px)?.css(),
    };
    let written = top(value, ',');
    let mut out = Vec::new();
    for (i, one) in top(&css, ',').into_iter().enumerate() {
        // The lengths as written, in order: the colour and `inset` aside.
        let lengths: Vec<&str> = written
            .get(i)
            .map(|w| top(w, ' '))
            .unwrap_or_default()
            .into_iter()
            .filter(|w| scale::is_length_item(w))
            .collect();
        let mut at = 0;
        let words: Vec<String> = top(one, ' ')
            .into_iter()
            .map(|w| {
                if !scale::is_length_item(w) {
                    return w.to_string();
                }
                let said = lengths.get(at).copied();
                at += 1;
                match said {
                    Some(b) if scale::is_base(b) => format!("var({})", property(b)),
                    Some(e) => expression(e).unwrap_or_else(|| w.to_string()),
                    None => w.to_string(),
                }
            })
            .collect();
        out.push(words.join(" "));
    }
    Some(out.join(", "))
}

/// `text`'s parts split at `sep` (a comma, or white space for `' '`)
/// outside parentheses, trimmed, none empty.
fn top(text: &str, sep: char) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0u32, 0);
    let at = |c: char| match sep {
        ' ' => c.is_whitespace(),
        _ => c == sep,
    };
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 && at(c) => {
                out.push(text[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(text[start..].trim());
    out.retain(|p| !p.is_empty());
    out
}

/// A base name's custom property: `--exact-space-md`.
pub fn property(name: &str) -> String {
    format!("--exact-{}", name.replace('.', "-"))
}

/// Base name `name`'s CSS in this build's column (the kernel's table).
pub fn value(name: &str) -> String {
    scale::css(scale::of(name, metrics::column()).expect("a base name"))
}

/// The sheet's declarations: `:root{--exact-space-md:16px;…}` for each base
/// name the plan names, nothing when it names none.
pub fn css(plan: &Plan) -> String {
    let names = named(plan);
    if names.is_empty() {
        return String::new();
    }
    let declarations: String = names
        .iter()
        .map(|n| format!("{}:{};", property(n), value(n)))
        .collect();
    format!(":root{{{declarations}}}")
}

/// A bound row's map entries for the base names the plan names: each its
/// property's `var()`, or its length where the row is an SVG attribute,
/// which reads no property.
pub fn pairs(plan: &Plan, attribute: bool) -> Vec<String> {
    named(plan)
        .into_iter()
        .map(|n| {
            let css = if attribute {
                value(n)
            } else {
                format!("var({})", property(n))
            };
            format!(
                "{}:{}",
                serde_json::to_string(n).unwrap(),
                serde_json::to_string(&css).unwrap()
            )
        })
        .collect()
}
