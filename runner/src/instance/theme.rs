//! A design's app modes in the instance tree (LLP 1121 D5, D7): the tuple
//! around each node, the rows that read a token, and a mode boundary's
//! implied rows. Names resolve where a row's text is parsed
//! (`bridge::set_plan_style`), in the theme table of the node's tuple; the
//! kernel stores only selected values, and the runner keeps each themed row
//! beside its text, to set it again when a slice or a token changes.

use exact_kernel::{StyleId, ViewId};
use exact_plan::{BindingKind, NodesRow, Plan, Value};

/// The inherited rows that hold a colour, which a mode boundary writes
/// again (D7): every one the schema marks inherited, in this order.
pub(super) const INHERITED: [StyleId; 8] = [
    StyleId::TextColor,
    StyleId::CaretColor,
    StyleId::Fill,
    StyleId::Stroke,
    StyleId::AccentColor,
    StyleId::TextShadow,
    StyleId::TextStrokeColor,
    StyleId::SymbolPalette,
];

/// What the nodes being visited are in: their app tuple, and for each
/// [`INHERITED`] row the text of the nearest ancestor's row when that row
/// read a token (`None` when it read none, or no ancestor sets it).
#[derive(Debug, Clone, Default)]
pub struct Around {
    /// The app tuple: the window's, overridden by the pins above.
    pub tuple: u32,
    /// The nearest themed text of each inherited colour row.
    pub inherited: [Option<Value>; 8],
    /// Every themed row is set again and every node visited: the window's
    /// tuple changed (`setMode`) or a pin above the visit did.
    pub retheme: bool,
}

/// One row whose text read a token.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemedRow {
    /// The style row.
    pub style: StyleId,
    /// Its text as set.
    pub text: Value,
    /// The app tuple it was set in.
    pub tuple: u32,
    /// Its author wrote it (LLP 1115.000 D2); a boundary's implied row not.
    pub written: bool,
    /// The tokens it read, as `theme::name_bit`s.
    pub tokens: u64,
}

/// Every row that read a token, by view and row (D7's index): what a
/// token's new value sets again, with no tree visited and no binding
/// evaluated. One map, so a themed list's rows allocate only its growth.
#[derive(Debug, Default)]
pub struct ThemeIndex {
    rows: std::collections::HashMap<(ViewId, u16), ThemedRow>,
    /// The window's app tuple (`setMode`), which a visit starts in.
    pub window: u32,
    /// The window's tuple moved since a visit last started: the next one
    /// sets every themed row again.
    pub moved: bool,
}

impl ThemeIndex {
    /// `view`'s `row`, which read a token.
    pub(super) fn put(&mut self, view: ViewId, row: ThemedRow) {
        self.rows.insert((view, row.style as u16), row);
    }

    /// `view`'s `style`, when it read a token.
    pub fn row(&self, view: ViewId, style: StyleId) -> Option<&ThemedRow> {
        self.rows.get(&(view, style as u16))
    }

    /// `view`'s `style` reads no token now.
    pub(super) fn drop_row(&mut self, view: ViewId, style: StyleId) {
        if !self.rows.is_empty() {
            self.rows.remove(&(view, style as u16));
        }
    }

    /// Forget the views `live` says are gone.
    pub(super) fn retain(&mut self, live: &impl Fn(ViewId) -> bool) {
        self.rows.retain(|(view, _), _| live(*view));
    }

    /// Every themed row, in no order.
    pub fn rows(&self) -> impl Iterator<Item = (ViewId, &ThemedRow)> + '_ {
        self.rows.iter().map(|((view, _), r)| (*view, r))
    }

    /// How many rows read a token.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no row reads a token.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// The index of `row`'s binding of `style`, if it has one.
pub(super) fn binding_of(plan: &Plan, row: &NodesRow, style: StyleId) -> Option<usize> {
    row.bindings.iter().position(|b| {
        let b = plan.binding(b);
        b.kind == BindingKind::Style && b.id == style as u16
    })
}

/// Whether `row` is a mode boundary: it pins modes or sets a scheme (D7).
pub(super) fn boundary(plan: &Plan, row: &NodesRow) -> bool {
    binding_of(plan, row, StyleId::Mode).is_some()
        || binding_of(plan, row, StyleId::ColorScheme).is_some()
}

/// The tuple a node pinning `pin` makes over `around`; `around` with no pin,
/// or one the plan's axes refuse (its row was unset and said so).
pub(super) fn tuple_of(plan: &Plan, pin: Option<&Value>, around: u32) -> u32 {
    pin.and_then(Value::as_str)
        .and_then(|text| crate::bridge::pinned(plan, text, around))
        .unwrap_or(around)
}

/// Bits a binding, the first 64 inline and the rest beside.
#[derive(Debug, Default)]
pub(super) struct Bits(u64, Vec<bool>);

impl Bits {
    pub(super) fn get(&self, i: usize) -> bool {
        match i {
            0..64 => self.0 & (1 << i) != 0,
            _ => self.1.get(i - 64).copied().unwrap_or(false),
        }
    }

    pub(super) fn set(&mut self, i: usize, on: bool) {
        match i {
            0..64 => match on {
                true => self.0 |= 1 << i,
                false => self.0 &= !(1 << i),
            },
            _ if !on && self.1.is_empty() => {}
            _ => {
                if self.1.len() <= i - 64 {
                    self.1.resize(i - 63, false);
                }
                self.1[i - 64] = on;
            }
        }
    }
}

/// Per binding of a node: whether its last text read a token, and whether
/// it set its row (a row it unset, or whose value the row refused, is the
/// inherited one's).
#[derive(Debug, Default)]
pub(super) struct Themed {
    named: Bits,
    set: Bits,
}

impl Themed {
    pub(super) fn get(&self, i: usize) -> bool {
        self.named.get(i)
    }

    pub(super) fn sets(&self, i: usize) -> bool {
        self.set.get(i)
    }

    pub(super) fn set(&mut self, i: usize, named: bool, set: bool) {
        self.named.set(i, named);
        self.set.set(i, set);
    }
}

/// The app tuple rows are parsed in, restored when dropped (a refused
/// binding returns early).
pub(super) struct Bound(u32);

impl Bound {
    pub(super) fn new(tuple: u32) -> Bound {
        Bound(crate::bridge::set_tuple(tuple))
    }
}

impl Drop for Bound {
    fn drop(&mut self) {
        crate::bridge::set_tuple(self.0);
    }
}

impl super::NodeInst {
    /// Enter this node's children: their tuple is its own, their inherited
    /// colours what it sets or passes on, and a tuple that moved since
    /// `before` sets every themed row below again. The modes it was in.
    pub(super) fn enter(&mut self, u: &mut super::Update<'_>, before: u32) -> Around {
        let plan = u.env.plan;
        if plan.theme.is_empty() {
            return u.theme.clone();
        }
        let row = plan.node(self.node);
        // A row it sets passes its text on when that read a token, else
        // nothing; a row it does not set, its implied row's or the one around.
        let inherited = std::array::from_fn(|k| match binding_of(plan, row, INHERITED[k]) {
            Some(b) if self.themed.sets(b) => {
                self.themed.get(b).then(|| self.last[b].clone()).flatten()
            }
            _ => match self.implied.as_ref().and_then(|implied| implied[k].clone()) {
                Some(text) => Some(text),
                None => u.theme.inherited[k].clone(),
            },
        });
        let saved = u.theme.clone();
        // A pin that moved, or a colour its descendants inherit, which a
        // boundary below sets again in its own tuple.
        let moved = std::mem::take(&mut self.colour_moved);
        u.theme.retheme |= self.tuple != before || moved;
        u.theme.tuple = self.tuple;
        u.theme.inherited = inherited;
        saved
    }

    /// Binding `i` set `style`'s row from `text`: kept beside its token when
    /// the parse read one (`ok`, the row took it), else forgotten.
    pub(super) fn index(
        &mut self,
        u: &mut super::Update<'_>,
        i: usize,
        binding: &exact_plan::BindingsRow,
        text: &Value,
        ok: bool,
    ) {
        let tokens = exact_kernel::style::theme::take_named();
        let named = tokens != 0 && ok;
        let Some(style) = StyleId::from_bit(binding.id as u32) else {
            return;
        };
        self.colour_moved |= INHERITED.contains(&style);
        self.themed.set(i, named, ok);
        match named {
            true => u.ids.theme.put(
                self.view,
                ThemedRow {
                    style,
                    text: text.clone(),
                    tuple: self.tuple,
                    written: binding.written,
                    tokens,
                },
            ),
            false => u.ids.theme.drop_row(self.view, style),
        }
    }

    /// Binding `i`'s `style` row was unset: it reads no token.
    pub(super) fn unthemed(&mut self, u: &mut super::Update<'_>, i: usize, style: StyleId) {
        self.colour_moved |= INHERITED.contains(&style);
        self.themed.set(i, false, false);
        u.ids.theme.drop_row(self.view, style);
    }

    /// A mode boundary's implied rows (D7): each inherited colour row it does
    /// not set, whose nearest ancestor's row read a token, set again from
    /// that text in this node's tuple. `force` sets them whatever they were.
    pub(super) fn boundary_rows(
        &mut self,
        u: &mut super::Update<'_>,
        patch: &mut Option<Box<exact_kernel::StyleProps>>,
        force: bool,
    ) {
        let plan = u.env.plan;
        let row = plan.node(self.node);
        for (k, style) in INHERITED.into_iter().enumerate() {
            // Its own row stands: an implied one is set again when it goes.
            if binding_of(plan, row, style).is_some_and(|b| self.themed.sets(b)) {
                if let Some(implied) = self.implied.as_mut() {
                    implied[k] = None;
                }
                continue;
            }
            let want = u.theme.inherited[k].clone();
            let implied = self.implied.get_or_insert_with(Default::default);
            if want == implied[k] && !force {
                continue;
            }
            match &want {
                Some(text) => {
                    let p = patch.get_or_insert_with(Box::default);
                    exact_kernel::style::theme::take_named();
                    let set = crate::bridge::set_plan_style(p, style as u16, text, plan);
                    let tokens = exact_kernel::style::theme::take_named();
                    if set.is_ok() {
                        u.ids.theme.put(
                            self.view,
                            ThemedRow {
                                style,
                                text: text.clone(),
                                tuple: self.tuple,
                                written: false,
                                tokens,
                            },
                        );
                    }
                }
                None if implied[k].is_some() => {
                    let mut mask = exact_kernel::StyleMask::default();
                    mask.set(style);
                    u.ops.push(exact_kernel::Op::ClearStyle {
                        id: self.view,
                        mask,
                        written: false,
                    });
                    u.ids.theme.drop_row(self.view, style);
                }
                None => {}
            }
            implied[k] = want;
        }
    }
}
