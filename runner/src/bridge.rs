//! Values to kernel rows.
//!
//! @ref LLP 1004 D2 (kernel ordinals get meaning from `exact-kernel`)
//!
//! A binding names a kernel prop or style row by ordinal and yields a
//! [`Value`]. This is the one place a value becomes a kernel write: props
//! through `PropValue` by the prop's declared kind, style rows through the
//! kernel's own generated `StyleProps::set_dynamic`. Nothing is redeclared.

use exact_kernel::{PropId, PropValue, StyleId, StyleProps, StyleValue, StyleValueError};
use exact_plan::Value;
use std::collections::HashMap;

/// Session-scoped CSS family lists, indexed after the plan's compiled stacks.
#[derive(Debug, Default, Clone)]
pub struct RuntimeFonts {
    names: Vec<String>,
    ids: HashMap<String, u16>,
}

impl RuntimeFonts {
    /// Intern a CSS family list, returning its kernel stack id.
    pub fn intern(&mut self, name: &str, compiled: usize) -> Option<u16> {
        if let Some(id) = self.ids.get(name) {
            return Some(*id);
        }
        if runtime_family_members(name).is_empty() {
            return None;
        }
        let id = u16::try_from(compiled.checked_add(self.names.len())?).ok()?;
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        Some(id)
    }

    /// Runtime lists in id order.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// One CSS family name; quoting makes a generic keyword an installed name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFamilyMember {
    /// The family name without CSS quotes.
    pub name: String,
    /// Whether the author quoted this member.
    pub quoted: bool,
}

/// Members of a runtime CSS family list. A malformed list has no members
/// and leaves the binding unset, as an invalid computed CSS value does.
pub fn runtime_family_members(list: &str) -> Vec<RuntimeFamilyMember> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (i, c) in list.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c == ',' {
            out.push(&list[start..i]);
            start = i + 1;
        }
    }
    out.push(&list[start..]);
    if quote.is_some() || escaped || out.len() > 64 {
        return Vec::new();
    }
    out.into_iter()
        .map(|part| {
            let part = part.trim();
            let quoted = part.len() >= 2
                && ((part.starts_with('"') && part.ends_with('"'))
                    || (part.starts_with('\'') && part.ends_with('\'')));
            let name = if quoted {
                &part[1..part.len() - 1]
            } else if part.contains(['"', '\'']) {
                return None;
            } else {
                part
            };
            // Unquoted, a member is CSS identifiers, by the rule a compiled
            // literal meets: a browser refuses the whole declaration otherwise.
            if name.is_empty() || (!quoted && !exact_plan::is_css_family_name(name)) {
                return None;
            }
            Some(RuntimeFamilyMember {
                name: name.to_string(),
                quoted,
            })
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

/// Why a binding's value could not become a kernel row.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    UnknownProp(u16),
    UnknownStyle(u16),
    PropKind {
        prop: PropId,
        value: Value,
    },
    /// A value that says the row has none (`backgroundMaterial="none"`):
    /// cleared, with nothing refused.
    Unset(PropId),
    Style(StyleValueError),
    StyleKind {
        style: StyleId,
        value: Value,
    },
    FontStack {
        index: u32,
        stacks: usize,
    },
}

/// @ref LLP 1121 D10 — a computed `symbol:` source naming an icon token, by
/// its name or (`sf/<name>`) by its SF Symbol, written as the source form a
/// host reads, as the compiler writes a literal one; any other value as it is.
pub fn icon_source(plan: &exact_plan::Plan, prop: PropId, value: PropValue) -> PropValue {
    let PropValue::Str(text) = &value else {
        return value;
    };
    let Some(name) = text
        .strip_prefix("symbol:")
        .filter(|_| prop == PropId::ImageSource)
    else {
        return value;
    };
    let s = |id| plan.strings.get(id as usize).unwrap_or("");
    let icon = match name.strip_prefix("sf/") {
        Some(sf) => plan
            .icons
            .iter()
            .find(|i| !sf.is_empty() && s(i.sf.0) == sf),
        None => plan.icons.iter().find(|i| s(i.name.0) == name),
    };
    match icon {
        Some(i) => PropValue::Str(
            format!(
                "symbol:icon|{}|{}|{}|{}",
                s(i.sf.0),
                s(i.view_box.0),
                s(i.path.0),
                s(i.svg.0)
            )
            .into(),
        ),
        None => value,
    }
}

/// A prop value by the prop's declared kind.
pub fn prop_value(id: u16, value: &Value) -> Result<(PropId, PropValue), BridgeError> {
    let prop = PropId::from_wire(id).ok_or(BridgeError::UnknownProp(id))?;
    if prop == PropId::Popover {
        match value {
            Value::Option(None) => return Err(BridgeError::Unset(prop)),
            Value::Option(Some(inner)) => return prop_value(id, inner),
            _ => {}
        }
    }
    let mismatch = || BridgeError::PropKind {
        prop,
        value: value.clone(),
    };
    let out = if prop == PropId::Decorations {
        PropValue::Str(
            crate::decorations::from_value(value)
                .map_err(|_| mismatch())?
                .into(),
        )
    } else if prop == PropId::AtomFaces {
        PropValue::Str(
            crate::decorations::atom_faces(value)
                .map_err(|_| mismatch())?
                .into(),
        )
    } else {
        match (prop.kind(), value) {
            (exact_kernel::PropKind::Str, v @ exact_plan::str_value!()) => {
                PropValue::Str(v.text().into())
            }
            (exact_kernel::PropKind::Str, Value::Number(n)) => {
                PropValue::Str(crate::stdlib::format_number(*n).into())
            }
            (exact_kernel::PropKind::Bool, Value::Bool(b)) => PropValue::Bool(*b),
            (exact_kernel::PropKind::Int, Value::Number(n))
                if n.is_finite()
                    && n.fract() == 0.0
                    && *n >= i64::MIN as f64
                    && *n < -(i64::MIN as f64) =>
            {
                PropValue::Int(*n as i64)
            }
            (exact_kernel::PropKind::Float, Value::Number(n)) => PropValue::Float(*n),
            _ => return Err(mismatch()),
        }
    };
    // `none` is no material, as CSS's `backdrop-filter: none` is no backdrop.
    if prop == PropId::BackgroundMaterial
        && matches!(&out, PropValue::Str(s) if s.as_str() == "none")
    {
        return Err(BridgeError::Unset(prop));
    }
    // A computed `wakelock` outside its two words is refused here, so the
    // row is unset (`none`) and journaled (LLP 1125 D11).
    if prop == PropId::Wakelock
        && !matches!(&out, PropValue::Str(s) if matches!(s.as_str(), "screen" | "none"))
    {
        return Err(mismatch());
    }
    Ok((prop, out))
}

/// Whether `value` is a CSS-wide keyword that leaves row `id` unset
/// ([`StyleValue::unsets`]): the row is cleared, as a class choice's
/// `none` is, and nothing is refused. On `font-family`, `revert` and
/// `revert-layer` are `unset` too, as for an author style the user agent
/// has no rule for; `initial` is the row's initial family ([`set_style`]).
pub fn unsets(id: u16, value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        StyleId::from_bit(id as u32).is_some_and(|style| {
            StyleValue::text_unsets(text, style)
                || (style == StyleId::FontFamily
                    && ["revert", "revert-layer"]
                        .iter()
                        .any(|keyword| text.trim().eq_ignore_ascii_case(keyword)))
        })
    })
}

/// [`set_style`] for a binding of `plan`: a `-exact-platform-color()` only as one
/// of the plan's own string literals (LLP 1095 D3), so data, a template or
/// a concatenation never chooses which platform colour a host looks up. A
/// literal chosen by state (a ternary's arm, a class) is still the plan's.
pub fn set_plan_style(
    patch: &mut StyleProps,
    id: u16,
    value: &Value,
    plan: &exact_plan::Plan,
) -> Result<StyleId, BridgeError> {
    if let v @ exact_plan::str_value!() = value {
        let text = v.text();
        if text.contains("-exact-platform-color(") && !plan.strings.iter().any(|s| s == text) {
            let style = StyleId::from_bit(id as u32).ok_or(BridgeError::UnknownStyle(id))?;
            return Err(BridgeError::StyleKind {
                style,
                value: value.clone(),
            });
        }
    }
    // Profiles are parsed in the owning plan, even before a candidate is accepted.
    // Ordinary styles allocate no declaration table.
    let _profiles = matches!(value, exact_plan::str_value!())
        .then(|| {
            value
                .text()
                .as_bytes()
                .windows(8)
                .any(|w| w.eq_ignore_ascii_case(b"color(--"))
        })
        .unwrap_or(false)
        // A theme entry may stand for a profile's colour.
        .then_some(())
        .or_else(|| (!plan.theme.is_empty() && !plan.profiles.is_empty()).then_some(()))
        .map(|()| {
            exact_kernel::style::profiled::declarations(
                plan.profiles
                    .iter()
                    .map(|row| (plan.str(row.name), plan.str(row.src), plan.str(row.intent))),
            )
        });
    // @ref LLP 1121 D5 — a pin's text is the plan's axes: the row holds the
    // full tuple it makes over the tuple around the node, plus one.
    if id == StyleId::Mode as u16 {
        let tuple = value
            .as_str()
            .and_then(|text| pinned(plan, text, tuple()))
            .ok_or(BridgeError::StyleKind {
                style: StyleId::Mode,
                value: value.clone(),
            })?;
        return set_style(
            patch,
            id,
            &Value::Number(f64::from(tuple + 1)),
            plan.stacks.len(),
        );
    }
    // @ref LLP 1095 D4 — the plan's theme, for a colour that names an entry.
    let _theme = (!plan.theme.is_empty() && matches!(value, exact_plan::str_value!()))
        .then(|| exact_kernel::style::theme::bind(&theme_table(plan)));
    set_style(patch, id, value, plan.stacks.len())
}

/// The full app tuple `text`'s pins (`axis=value`, space- or comma-
/// separated) make over `base`, axis by axis (LLP 1121 D5); `None` when a
/// pin names no axis or value of `plan`'s.
pub fn pinned(plan: &exact_plan::Plan, text: &str, base: u32) -> Option<u32> {
    let axes: Vec<(&str, Vec<&str>)> = plan
        .axes
        .iter()
        .map(|a| (plan.str(a.name), plan.str(a.values).split('|').collect()))
        .collect();
    // The base's digit per axis, the first axis most significant.
    let mut digits = vec![0u32; axes.len()];
    let mut rest = base;
    for (i, (_, values)) in axes.iter().enumerate().rev() {
        digits[i] = rest % values.len() as u32;
        rest /= values.len() as u32;
    }
    // Each axis once, as the compiler checks a literal pin.
    let mut pinned = 0u64;
    for pin in text.split([' ', ',']).filter(|p| !p.is_empty()) {
        let (axis, value) = pin.split_once('=')?;
        let i = axes.iter().position(|(name, _)| *name == axis)?;
        if i >= 64 || pinned & (1 << i) != 0 {
            return None;
        }
        pinned |= 1 << i;
        digits[i] = axes[i].1.iter().position(|v| *v == value)? as u32;
    }
    (pinned != 0).then(|| {
        axes.iter()
            .zip(&digits)
            .fold(0, |tuple, ((_, values), d)| tuple * values.len() as u32 + d)
    })
}

/// A plan's theme and strings buffers and the theme's length: which plan a
/// cached table is.
type PlanAt = (usize, usize, usize);

/// One token's edit: its plan, its name, its default tuple's value and,
/// when it varies, every tuple's.
type Edit = (PlanAt, String, String, Vec<String>);

thread_local! {
    /// The last plan's theme tables, one an app tuple as it is first bound,
    /// kept so a themed list's thousand rows bind one by pointer instead of
    /// building it each.
    static THEME_TABLE: std::cell::RefCell<Option<(PlanAt, Vec<Option<exact_kernel::style::theme::Table>>)>> =
        const { std::cell::RefCell::new(None) };
    /// The app tuple of the rows being set (LLP 1121 D5): the window's,
    /// overridden by the nearest `-exact-mode` pin; the runner sets it
    /// around a node.
    static TUPLE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Tokens edited since a plan was built (LLP 1121 D13), by the plan and
    /// the token's name: the default tuple's value and, when it varies,
    /// every tuple's.
    static EDITS: std::cell::RefCell<Vec<Edit>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// A token's value from now on, over the plan's (LLP 1121 D13's edit): its
/// default tuple's and, when it varies, every tuple's. The tables are built
/// again when next bound. Whether it differs from what the token held.
pub fn edit_token(plan: &exact_plan::Plan, name: &str, value: &str, slices: &[&str]) -> bool {
    let at = plan_at(plan);
    EDITS.with(|edits| {
        let mut edits = edits.borrow_mut();
        let held = match edits.iter().find(|(p, n, ..)| *p == at && n == name) {
            Some((_, _, v, s)) => Some((v.clone(), s.clone())),
            None => plan
                .theme
                .iter()
                .find(|r| plan.str(r.name) == name)
                .map(|r| {
                    let slices =
                        &plan.theme_slices[r.slices.start as usize..][..r.slices.len as usize];
                    (
                        plan.str(r.value).to_string(),
                        slices
                            .iter()
                            .map(|s| plan.str(s.value).to_string())
                            .collect(),
                    )
                }),
        };
        let moved = held.is_none_or(|(v, s): (String, Vec<String>)| {
            v != value || s.iter().map(String::as_str).ne(slices.iter().copied())
        });
        edits.retain(|(p, n, ..)| *p != at || n != name);
        edits.push((
            at,
            name.into(),
            value.into(),
            slices.iter().map(|s| (*s).into()).collect(),
        ));
        THEME_TABLE.with(|cache| cache.borrow_mut().take());
        moved
    })
}

/// A plan's buffers: which plan a cached table or an edit is.
fn plan_at(plan: &exact_plan::Plan) -> PlanAt {
    (
        plan.theme.as_ptr() as usize,
        plan.theme.len(),
        plan.strings.identity(),
    )
}

/// Set the app tuple rows are set in; the one before.
pub fn set_tuple(tuple: u32) -> u32 {
    TUPLE.with(|t| t.replace(tuple))
}

/// The app tuple rows are set in ([`set_tuple`]).
pub fn tuple() -> u32 {
    TUPLE.with(|t| t.get())
}

/// How many full app tuples `plan`'s axes make (1 with none).
pub fn tuples(plan: &exact_plan::Plan) -> u32 {
    plan.axes
        .iter()
        .map(|a| plan.str(a.values).split('|').count() as u32)
        .product()
}

/// `plan`'s theme as a parse binds it, in the current app tuple
/// ([`theme_table_at`]).
pub fn theme_table(plan: &exact_plan::Plan) -> exact_kernel::style::theme::Table {
    theme_table_at(plan, tuple())
}

/// `plan`'s theme as a parse binds it in app tuple `tuple`: each entry under
/// its name, its value that tuple's slice when it varies (LLP 1121 D5), and
/// an override under its role's spelling too. Built when the plan differs
/// from the last one this thread bound (known by its buffers' addresses,
/// nothing scanned per row) or the tuple was not bound yet, else the one
/// built before.
pub fn theme_table_at(plan: &exact_plan::Plan, tuple: u32) -> exact_kernel::style::theme::Table {
    use exact_kernel::style::roles;
    // The plan's own buffers: a cached table built from them is this plan's.
    let at = plan_at(plan);
    THEME_TABLE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.as_ref().is_none_or(|(key, _)| *key != at) {
            *cache = Some((at, Vec::new()));
        }
        let (_, tables) = cache.as_mut().expect("set above");
        let slot = tuple as usize;
        if let Some(Some(table)) = tables.get(slot) {
            return table.clone();
        }
        let edits: Vec<_> = EDITS.with(|e| {
            e.borrow()
                .iter()
                .filter(|(p, ..)| *p == at)
                .cloned()
                .collect()
        });
        use exact_kernel::style::theme::Kind;
        // A type or family token's family as the plan's stack (stage 6 C):
        // the compiler declared one for each tuple's.
        let stacks = plan_stacks(plan);
        let stack = |family: &str| {
            let key = exact_kernel::style::theme::typeface::family_key_of(family);
            stacks.iter().find(|(k, _)| *k == key).map(|(_, id)| *id)
        };
        let entries: Vec<_> = plan
            .theme
            .iter()
            .flat_map(|row| {
                // A kind this build does not know is no entry: its rows stand.
                let kind = Kind::of(row.kind);
                let (name, role) = (plan.str(row.name), plan.str(row.role));
                let edit = edits.iter().find(|(_, n, ..)| n == name);
                let value = match (edit, row.slices.len) {
                    (Some((_, _, value, slices)), _) => {
                        slices.get(tuple as usize).unwrap_or(value).as_str()
                    }
                    (None, 0) => plan.str(row.value),
                    (None, len) => plan.str(
                        plan.theme_slices[(row.slices.start + tuple.min(len - 1)) as usize].value,
                    ),
                };
                // A tint role stays the role in its row (`ColorValue::themed`).
                let tint = roles::role(role).is_some_and(roles::tint);
                // Whether it varies by an app axis, as edited (D7: no inherited
                // row reads such a length or number).
                let varies =
                    edit.map_or(row.slices.len > 0, |(_, _, _, slices)| !slices.is_empty());
                let value: std::borrow::Cow<str> = match kind {
                    Some(k @ (Kind::Type | Kind::Family)) => {
                        exact_kernel::style::theme::typeface::entry(k, value, stack)
                            .map_or(std::borrow::Cow::Borrowed(value), std::borrow::Cow::Owned)
                    }
                    _ => std::borrow::Cow::Borrowed(value),
                };
                kind.filter(|_| !tint)
                    .map(|kind| (name, value.clone(), false, kind, varies))
                    .into_iter()
                    .chain(
                        (!role.is_empty() && !tint && kind == Some(Kind::Color)).then_some((
                            role,
                            value,
                            true,
                            Kind::Color,
                            varies,
                        )),
                    )
            })
            .collect();
        let table = exact_kernel::style::theme::kinded(
            entries
                .iter()
                .map(|(n, v, r, k, va)| (*n, v.as_ref(), *r, *k, *va)),
        );
        if tables.len() <= slot {
            tables.resize(slot + 1, None);
        }
        tables[slot] = Some(table.clone());
        table
    })
}

/// Whether `text` names one of `plan`'s type or family tokens (stage 6 C):
/// a `font-family` row's string the kernel reads as the token's stack,
/// never interned as an installed family.
pub fn names_font_token(plan: &exact_plan::Plan, text: &str) -> bool {
    use exact_kernel::style::theme::Kind;
    let text = text.trim();
    plan.theme.iter().any(|r| {
        matches!(Kind::of(r.kind), Some(Kind::Type | Kind::Family)) && plan.str(r.name) == text
    })
}

/// Binding `b`'s value as a `font-family` row reads it (an option's
/// inner value), and the runtime stack it names: a family list interned
/// for the session, but never a type or family token's name, which the
/// kernel reads as the token's stack (stage 6 C).
pub fn family_binding<'v>(
    plan: &exact_plan::Plan,
    fonts: Option<&std::cell::RefCell<RuntimeFonts>>,
    b: &exact_plan::BindingsRow,
    value: &'v Value,
) -> (&'v Value, Option<u16>) {
    if b.id != StyleId::FontFamily as u16 {
        return (value, None);
    }
    let value = match value {
        Value::Option(Some(inner)) => inner.as_ref(),
        _ => value,
    };
    let id = value
        .as_str()
        .filter(|n| !names_font_token(plan, n))
        .and_then(|name| fonts?.borrow_mut().intern(name, plan.stacks.len()));
    (value, id)
}

/// A refusal when binding `b`'s `value` on site `node` names a type or
/// family token that varies by an app axis on a node whose children
/// inherit its rows (stage 6 C, theme2's ruling): the compiler refuses a
/// literal so; a value computed at run time is refused here, as invalid.
pub fn inherited_varying(
    plan: &exact_plan::Plan,
    b: &exact_plan::BindingsRow,
    value: &Value,
    node: exact_plan::NodesId,
) -> Result<(), BridgeError> {
    use exact_kernel::style::theme::{typeface, Kind};
    let Some(row) = StyleId::from_bit(u32::from(b.id)).filter(|r| typeface::admits_type(*r)) else {
        return Ok(());
    };
    let Some(text) = value.as_str().map(str::trim) else {
        return Ok(());
    };
    let varying = plan.theme.iter().any(|r| {
        matches!(Kind::of(r.kind), Some(Kind::Type | Kind::Family))
            && r.slices.len > 0
            && plan.str(r.name) == text
    });
    // Its children: its own nodes, and a choice's or a list's under it. A
    // scan of the plan, so only for a row that varies.
    let children = || {
        plan.nodes.iter().any(|n| n.parent == Some(node))
            || plan.regions.iter().any(|r| r.parent == Some(node))
    };
    if b.kind == exact_plan::BindingKind::Style && varying && children() {
        return Err(BridgeError::StyleKind {
            style: row,
            value: value.clone(),
        });
    }
    Ok(())
}

/// The stack of `plan`'s whose members `family` names, as the compiler
/// declared a family token's (stage 6 C).
pub fn plan_stack(plan: &exact_plan::Plan, family: &str) -> Option<u16> {
    let key = exact_kernel::style::theme::typeface::family_key_of(family);
    plan_stacks(plan)
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, id)| id)
}

/// `plan`'s font stacks by their members' key
/// (`typeface::family_key`), each with its id.
fn plan_stacks(plan: &exact_plan::Plan) -> Vec<(String, u16)> {
    plan.stacks
        .iter()
        .enumerate()
        .map(|(id, stack)| {
            let names: Vec<(bool, &str)> = stack
                .members
                .iter()
                .map(|m| {
                    let member = plan.stack_member(m);
                    match member.family {
                        Some(f) => (true, plan.str(plan.familie(f).name)),
                        None => (false, member.kind.name()),
                    }
                })
                .collect();
            (
                exact_kernel::style::theme::typeface::family_key(names),
                id as u16,
            )
        })
        .collect()
}

/// @ref LLP 1095 D4 — the roles `plan`'s theme replaces, for the session
/// that runs it (LLP 1121 stage 5): where the kernel resolves a role (its
/// own defaults: unsaid text is `CanvasText`) and what crosses to a host, as
/// well as a row naming the role. Installed on the thread for a caller that
/// emits this one plan, while the returned table is held.
pub fn set_role_overrides(
    plan: &exact_plan::Plan,
) -> std::rc::Rc<exact_kernel::style::roles::Roles> {
    let roles = exact_kernel::style::roles::Roles::new(role_overrides(plan));
    exact_kernel::style::roles::install(&roles);
    roles
}

/// The role overrides `plan`'s theme sets, each as a role and its colour;
/// the theme-table cache and a fresh boot's token edits start over for it.
pub fn role_overrides(plan: &exact_plan::Plan) -> Vec<(u8, exact_kernel::ColorValue)> {
    // A plan this thread boots or emits is the one its rows are set from
    // next: the cached table is rebuilt for it (never another plan's whose
    // buffers it reuses).
    THEME_TABLE.with(|cache| cache.borrow_mut().take());
    // A plan booted afresh starts from its own tokens (LLP 1121 D13), though
    // its buffers may be where an edited one's were.
    let at = plan_at(plan);
    EDITS.with(|edits| edits.borrow_mut().retain(|(p, ..)| *p != at));
    let _profiles = exact_kernel::style::profiled::declarations(
        plan.profiles
            .iter()
            .map(|row| (plan.str(row.name), plan.str(row.src), plan.str(row.intent))),
    );
    plan.theme
        .iter()
        .filter_map(|row| {
            let id = exact_kernel::style::roles::role(plan.str(row.role))?;
            Some((id, role_colour(plan.str(row.value))?))
        })
        .collect()
}

/// A role override's colour from its row's value text (a pair, or one).
pub fn role_colour(value: &str) -> Option<exact_kernel::ColorValue> {
    exact_kernel::ColorValue::parse_light_dark(value)
        .or_else(|| exact_kernel::Color::parse(value).map(exact_kernel::ColorValue::Fixed))
}

/// [`set_plan_style`] for one of `plan`'s style bindings, with what the
/// binding says of the row: whether its author wrote it on this node
/// (@ref LLP 1115.000 D2). The kernel keeps that beside the row.
pub fn set_binding_style(
    patch: &mut StyleProps,
    binding: &exact_plan::BindingsRow,
    value: &Value,
    plan: &exact_plan::Plan,
) -> Result<StyleId, BridgeError> {
    // @ref LLP 1115.002 D6 — a metric's name is the compiler's, in a sheet
    // it writes: a row its author wrote never reads one, whatever built
    // the value (the compiler refuses the literal).
    if binding.written
        && value
            .as_str()
            .is_some_and(|text| exact_kernel::style::metrics::named(text).is_some())
    {
        let style =
            StyleId::from_bit(binding.id as u32).ok_or(BridgeError::UnknownStyle(binding.id))?;
        return Err(BridgeError::StyleKind {
            style,
            value: value.clone(),
        });
    }
    let style = set_plan_style(patch, binding.id, value, plan)?;
    set_written(patch, binding, style);
    Ok(style)
}

/// A binding whose runtime font stack id has been interned by this session.
pub fn set_runtime_font_style(
    patch: &mut StyleProps,
    binding: &exact_plan::BindingsRow,
    id: u16,
    stacks: usize,
) -> Result<StyleId, BridgeError> {
    let style = set_style(patch, binding.id, &Value::Number(f64::from(id)), stacks)?;
    set_written(patch, binding, style);
    Ok(style)
}

fn set_written(patch: &mut StyleProps, binding: &exact_plan::BindingsRow, style: StyleId) {
    // Both ways: a later binding for the same row is the winner, and an
    // implied winner takes an earlier written bit off (a compiled plan
    // has one binding a row; a built one may not).
    if binding.written {
        patch.written.set(style);
    } else {
        patch.written.clear(style);
    }
}

/// Set style row `id` on `patch` from `value`.
pub fn set_style(
    patch: &mut StyleProps,
    id: u16,
    value: &Value,
    stacks: usize,
) -> Result<StyleId, BridgeError> {
    let style = StyleId::from_bit(id as u32).ok_or(BridgeError::UnknownStyle(id))?;
    if style == StyleId::FontFamily {
        // CSS's `initial` is the row's initial value, not the parent's.
        if value
            .as_str()
            .is_some_and(|text| text.trim().eq_ignore_ascii_case("initial"))
        {
            let initial = StyleProps::default().font_family;
            return set_style(patch, id, &Value::Number(f64::from(initial)), stacks);
        }
        // A type or family token's name, which the kernel reads as the
        // token's stack (stage 6 C).
        if value
            .as_str()
            .is_some_and(|t| exact_kernel::style::theme::names(t.trim()))
        {
            let text = value.as_str().unwrap_or_default().trim();
            patch
                .set_dynamic(style, &StyleValue::Text(text.into()))
                .map_err(|_| BridgeError::StyleKind {
                    style,
                    value: value.clone(),
                })?;
            return Ok(style);
        }
        let index = match value {
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => *n as u32,
            _ => {
                return Err(BridgeError::StyleKind {
                    style,
                    value: value.clone(),
                })
            }
        };
        if index as usize >= stacks {
            return Err(BridgeError::FontStack { index, stacks });
        }
    }
    let style_value = match value {
        Value::Number(n) => StyleValue::Number(*n),
        v @ exact_plan::str_value!() => match v.text() {
            // CSS keywords are ASCII case-insensitive (`margin="0 Auto"`).
            t if style.codec() == exact_kernel::StyleCodec::Dimension
                && t.trim_matches(['\t', '\n', '\u{c}', '\r', ' '])
                    .eq_ignore_ascii_case("auto") =>
            {
                StyleValue::Auto
            }
            // A lone percentage is one; other text ending in `%` is a CSS
            // value (`transform-origin: 0 100%`), as the compiler reads it.
            t => match t
                .strip_suffix('%')
                .and_then(|p| exact_num::parse_f64(p).ok())
            {
                Some(p) => StyleValue::Percent(p),
                _ => StyleValue::Text(t.into()),
            },
        },
        _ => {
            return Err(BridgeError::StyleKind {
                style,
                value: value.clone(),
            })
        }
    };
    patch
        .set_dynamic(style, &style_value)
        .map_err(BridgeError::Style)?;
    Ok(style)
}

/// The plan's `@keyframes` by name, each parsed once per plan (LLP 1055 D5),
/// the first time an `animation` names it: a plan's rules are many and a
/// first frame starts few of them.
#[derive(Debug, Default)]
pub struct KeyframesTable(
    Vec<(
        String,
        String,
        std::cell::OnceCell<Option<exact_motion::Keyframes>>,
    )>,
);

/// How a runner resolves an `animation` list's names against the plan's
/// `@keyframes`, when CSS animations are linked (LLP 1047.001): a plan that
/// animates nothing never names a rule, and links none of the resolution.
pub type KeyframesLink = Option<fn(&KeyframesTable, &mut exact_motion::Animations) -> Vec<String>>;

/// The plan's `keyframes` table. The compiler validated every row; one that
/// no longer parses (a plan from another evaluator) names nothing.
pub fn keyframes(plan: &exact_plan::Plan) -> KeyframesTable {
    KeyframesTable(
        plan.keyframes
            .iter()
            .map(|row| {
                (
                    plan.str(row.name).to_string(),
                    plan.str(row.css).to_string(),
                    std::cell::OnceCell::new(),
                )
            })
            .collect(),
    )
}

impl KeyframesTable {
    /// Give an `animation` row its keyframes; a name no rule has starts no
    /// animation, as in CSS, and is returned as a journal line.
    pub fn resolve(&self, row: &mut exact_motion::Animations) -> Vec<String> {
        row.resolve(|name| {
            self.0
                .iter()
                .filter(|(n, ..)| n == name)
                .find_map(|(_, css, rule)| {
                    rule.get_or_init(|| exact_motion::Keyframes::parse(css).ok())
                        .as_ref()
                })
        })
        .into_iter()
        .map(|name| format!("animation-name `{name}` matches no keyframes: no animation"))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_css_family_members_keep_quoted_commas_and_order() {
        assert_eq!(
            runtime_family_members("Missing, 'A, B', ui-monospace"),
            [
                RuntimeFamilyMember {
                    name: "Missing".into(),
                    quoted: false
                },
                RuntimeFamilyMember {
                    name: "A, B".into(),
                    quoted: true
                },
                RuntimeFamilyMember {
                    name: "ui-monospace".into(),
                    quoted: false
                },
            ]
        );
        assert!(runtime_family_members("'unfinished, serif").is_empty());
        assert!(runtime_family_members("serif,,monospace").is_empty());
    }

    #[test]
    fn invalid_runtime_lists_do_not_intern() {
        let mut fonts = RuntimeFonts::default();
        for value in [
            "inherit",
            "initial",
            "unset",
            "revert",
            "revert-layer",
            "serif,,monospace",
            "'unfinished",
            // Not CSS identifiers: the browser refuses the declaration.
            "Arial;",
            "1Arial",
            "Arial 2",
            "Arial!",
            "Foo/Bar",
            "-1x",
            "Arial, 1x",
            "Arial, initial",
            "default",
            "serif Foo",
            "Monospace Foo",
        ] {
            assert_eq!(fonts.intern(value, 8), None, "{value}");
        }
        assert!(fonts.names().is_empty());
        assert_eq!(fonts.intern("'inherit', Arial", 8), Some(8));
        for value in [
            "'Arial;'",
            "Helvetica Neue",
            "Helvetica\tNeue",
            "-x",
            "--x",
            "_x",
            "a-b_c9",
            "\\31 x",
            "Foo default",
            "inherit Foo",
            "Foo serif",
            "emoji Foo",
            "ui-monospace Foo",
            "Ünï",
        ] {
            assert!(fonts.intern(value, 8).is_some(), "{value}");
        }
    }

    /// CSS-wide keywords: `initial` is the row's initial value, the
    /// kernel's family 0, not the parent's; `revert` on an author style,
    /// with no user-agent rule for the row, is `unset`, as `inherit` is.
    #[test]
    fn initial_is_the_kernel_family_and_revert_inherits() {
        let family = StyleId::FontFamily as u16;
        for keyword in ["initial", "INITIAL", " initial "] {
            let mut style = StyleProps::default();
            set_style(&mut style, family, &Value::Number(4.0), 8).unwrap();
            set_style(&mut style, family, &Value::str(keyword), 8).unwrap();
            assert!(style.mask.has(StyleId::FontFamily), "{keyword}");
            assert_eq!(style.font_family, 0, "{keyword}");
        }
        for keyword in ["inherit", "unset", "revert", "Revert-Layer"] {
            assert!(unsets(family, &Value::str(keyword)), "{keyword}");
        }
        assert!(!unsets(family, &Value::str("initial")));
    }

    #[test]
    fn a_platform_color_is_admitted_only_as_a_plan_literal() {
        let literal = "-exact-platform-color(ios bridgeTestColor, #010203)";
        let mut plan = exact_plan::Plan::default();
        plan.strings.push(literal);
        let color = StyleId::TextColor as u16;
        let mut style = StyleProps::default();
        assert!(set_plan_style(&mut style, color, &Value::str(literal), &plan).is_ok());
        for built in [
            "-exact-platform-color(ios bridgeTestOtherColor, #010203)",
            "linear-gradient(-exact-platform-color(ios bridgeTestColor, #010203), #fff)",
        ] {
            assert!(
                matches!(
                    set_plan_style(&mut style, color, &Value::str(built), &plan),
                    Err(BridgeError::StyleKind { .. })
                ),
                "{built}"
            );
        }
        assert!(set_plan_style(&mut style, color, &Value::str("#fff"), &plan).is_ok());
    }

    /// LLP 1115.002 D6: a metric's name is the compiler's. A row its author
    /// wrote never reads one, whatever built the value; a row of a sheet
    /// the compiler wrote holds the metric's value in this process's column
    /// (the web's, until a host says its platform).
    #[test]
    fn a_metric_is_read_only_on_a_row_the_compiler_wrote() {
        let plan = exact_plan::Plan::default();
        let row = |written| exact_plan::BindingsRow {
            kind: exact_plan::BindingKind::Style,
            id: StyleId::MinHeight as u16,
            expr: Default::default(),
            written,
        };
        let name = Value::str("-exact-grouped-row-min-height");
        let mut style = StyleProps::default();
        assert!(matches!(
            set_binding_style(&mut style, &row(true), &name, &plan),
            Err(BridgeError::StyleKind { .. })
        ));
        assert!(!style.mask.has(StyleId::MinHeight));
        set_binding_style(&mut style, &row(false), &name, &plan).unwrap();
        assert_eq!(style.min_height, exact_kernel::Dimension::Points(52.0));
        assert!(!style.written.has(StyleId::MinHeight), "the platform's");
    }

    /// LLP 1115.000 D2: where two bindings set one row the node has the
    /// winner's bit, an implied winner's too. A compiled plan has one
    /// binding a row, but validation admits two.
    #[test]
    fn a_later_binding_for_the_same_row_brings_its_own_written_bit() {
        let plan = exact_plan::Plan::default();
        let row = |written| exact_plan::BindingsRow {
            kind: exact_plan::BindingKind::Style,
            id: StyleId::TextColor as u16,
            expr: Default::default(),
            written,
        };
        let mut style = StyleProps::default();
        set_binding_style(&mut style, &row(true), &Value::str("#ff0000"), &plan).unwrap();
        assert!(style.written.has(StyleId::TextColor));
        set_binding_style(&mut style, &row(false), &Value::str("#0000ff"), &plan).unwrap();
        assert!(style.mask.has(StyleId::TextColor), "the row is still set");
        assert!(
            !style.written.has(StyleId::TextColor),
            "the implied winner takes the written bit off"
        );
        set_binding_style(&mut style, &row(true), &Value::str("#00ff00"), &plan).unwrap();
        assert!(style.written.has(StyleId::TextColor));
    }

    #[test]
    fn css_percentages_accept_browser_exponents() {
        let mut style = StyleProps::default();
        set_style(
            &mut style,
            StyleId::GridTemplateColumns as u16,
            &Value::str("1e2%"),
            0,
        )
        .unwrap();
        assert_eq!(style.rare.grid_template_columns.css(), "100%");
    }

    /// A computed `none` is no material: the row cleared, nothing journaled
    /// (euxy's capsule, glass at rest and none while a charge holds).
    #[test]
    fn a_computed_none_material_clears_the_row() {
        use crate::{DataError, DataSource, Event, Runner};
        struct NoData;
        impl DataSource for NoData {
            fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
                Err(DataError::UnknownSource(source.into()))
            }
        }
        let plan = contract::compile("component App\n  state on = false\n  action flip\n    on = not on\n  view\n    button press=flip testId=\"cap\" backgroundMaterial=(on ? \"none\" : \"glass\")\n      text \"x\"\n").unwrap();
        let mut r = Runner::boot(
            plan,
            NoData,
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let material = |r: &Runner<NoData>| {
            let k = r.kernel();
            let node = k.node_by_key(k.find_by_test_id("cap")[0]).unwrap();
            (
                node.id,
                node.props
                    .str(PropId::BackgroundMaterial)
                    .map(str::to_owned),
            )
        };
        let (id, glass) = material(&r);
        assert_eq!(glass.as_deref(), Some("glass"));
        r.dispatch(id, Event::Press).unwrap();
        assert_eq!(material(&r).1, None);
        assert!(
            !r.journal().any(|l| l.contains("invalid")),
            "{:?}",
            r.journal().collect::<Vec<_>>()
        );
        r.dispatch(id, Event::Press).unwrap();
        assert_eq!(material(&r).1.as_deref(), Some("glass"));
        assert_eq!(
            prop_value(PropId::BackgroundMaterial as u16, &Value::str("none")),
            Err(BridgeError::Unset(PropId::BackgroundMaterial))
        );
    }
}
