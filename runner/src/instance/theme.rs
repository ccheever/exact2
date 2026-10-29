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
    /// A boundary set it (D7's implied row), not a binding.
    pub implied: bool,
    /// The tokens it read, as `theme::name_bit`s.
    pub tokens: u64,
}

/// A Markdown node's `atomFaces` whose colours read a token (LLP 1045.000
/// D2): the faces as the bridge named them, read again in their tuple when
/// a token changes, as a row's text is.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemedFaces {
    /// The faces' named JSON, the colours as written.
    pub text: exact_kernel::PropValue,
    /// The app tuple they were read in.
    pub tuple: u32,
    /// The tokens they read, as `theme::name_bit`s.
    pub tokens: u64,
}

/// Every row that read a token, by view and row (D7's index): what a
/// token's new value sets again, with no tree visited and no binding
/// evaluated. One map, so a themed list's rows allocate only its growth.
#[derive(Debug, Default)]
pub struct ThemeIndex {
    rows: std::collections::HashMap<(ViewId, u16), ThemedRow>,
    /// Each node's state looks that read a token (LLP 1121 D8): what a
    /// token's new value resolves again, as `rows` are.
    pub(super) looks: std::collections::HashMap<ViewId, super::looks::Looks>,
    /// Each Markdown node's faces that read a token, likewise.
    faces: std::collections::HashMap<ViewId, ThemedFaces>,
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

    /// `view`'s `style`, set again, read `tokens` too: a token's new value
    /// may read one its old value didn't (a shadow over a base the design
    /// sets, LLP 1121 stage 12 B3), which an edit of that one then sets
    /// again. The bits are added, never replaced: a row whose text no
    /// longer reads a token keeps its bit, which costs at most one re-set
    /// that changes nothing (bits are already shared by names, `name_bit`),
    /// where replacing them could drop a read the parse didn't repeat.
    pub(crate) fn also_read(&mut self, view: ViewId, style: StyleId, tokens: u64) {
        if let Some(row) = self.rows.get_mut(&(view, style as u16)) {
            row.tokens |= tokens;
        }
    }

    /// `view`'s state looks, resolved again, read `tokens` too, added as
    /// [`ThemeIndex::also_read`] adds a row's.
    pub(crate) fn looks_read(&mut self, view: ViewId, tokens: u64) {
        if let Some(l) = self.looks.get_mut(&view) {
            l.tokens |= tokens;
        }
    }

    /// `view`'s faces, read again, read `tokens` too, added as
    /// [`ThemeIndex::also_read`] adds a row's.
    pub(crate) fn faces_read(&mut self, view: ViewId, tokens: u64) {
        if let Some(f) = self.faces.get_mut(&view) {
            f.tokens |= tokens;
        }
    }

    /// Each Markdown node's faces that read a token, by view.
    pub fn faces(&self) -> impl Iterator<Item = (ViewId, &ThemedFaces)> + '_ {
        self.faces.iter().map(|(view, f)| (*view, f))
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
        self.looks.retain(|view, _| live(*view));
        self.faces.retain(|view, _| live(*view));
    }

    /// Each node's state looks that read a token, by view.
    pub fn looks(&self) -> impl Iterator<Item = (ViewId, &super::looks::Looks)> + '_ {
        self.looks.iter().map(|(view, l)| (*view, l))
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

/// Per binding of a node: whether its last text read a token, and whether
/// it set its row (a row it unset, or whose value the row refused, is the
/// inherited one's). A bit a binding, the first 64 inline and the rest, for
/// the rare node with more bindings, beside.
#[derive(Debug, Default)]
pub(super) struct Themed {
    named: u64,
    set: u64,
    rest: Option<Box<[Vec<bool>; 2]>>,
}

impl Themed {
    fn bit(&self, inline: u64, list: usize, i: usize) -> bool {
        match i {
            0..64 => inline & (1 << i) != 0,
            _ => (self.rest.as_ref()).is_some_and(|rest| rest[list].get(i - 64) == Some(&true)),
        }
    }

    pub(super) fn get(&self, i: usize) -> bool {
        self.bit(self.named, 0, i)
    }

    pub(super) fn sets(&self, i: usize) -> bool {
        self.bit(self.set, 1, i)
    }

    pub(super) fn set(&mut self, i: usize, named: bool, set: bool) {
        let put = |word: &mut u64, on: bool| match on {
            true => *word |= 1 << i,
            false => *word &= !(1 << i),
        };
        match i {
            0..64 => {
                put(&mut self.named, named);
                put(&mut self.set, set);
            }
            _ if !named && !set && self.rest.is_none() => {}
            _ => {
                let rest = self.rest.get_or_insert_with(Default::default);
                for (list, on) in rest.iter_mut().zip([named, set]) {
                    if list.len() <= i - 64 {
                        list.resize(i - 63, false);
                    }
                    list[i - 64] = on;
                }
            }
        }
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
        let inherited = std::array::from_fn(|k| match binding_of(plan, &row, INHERITED[k]) {
            Some(b) if self.themed.sets(b) => self
                .themed
                .get(b)
                .then(|| self.emitted(plan, u.sites, b))
                .flatten(),
            _ => match self
                .rare
                .as_ref()
                .and_then(|r| r.implied.as_ref()?[k].clone())
            {
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

    /// Binding `i`'s `atomFaces` (LLP 1045.000 D2), each face's colours read
    /// as a bound colour row's are, in this node's tuple
    /// (`bridge::atom_face_paint`). Faces that read a token are kept in the
    /// index, read again when the token changes, and their binding is set
    /// again when the tuple moves, as a themed row's is.
    pub(super) fn faces(
        &mut self,
        u: &mut super::Update<'_>,
        i: usize,
        text: exact_kernel::PropValue,
        refused: &mut Vec<String>,
    ) -> exact_kernel::PropValue {
        exact_kernel::style::theme::take_named();
        let painted = crate::bridge::atom_face_paint(
            u.env.plan,
            exact_kernel::PropId::AtomFaces,
            text.clone(),
            refused,
        );
        let tokens = exact_kernel::style::theme::take_named();
        self.themed.set(i, tokens != 0, false);
        match tokens {
            0 => {
                u.ids.theme.faces.remove(&self.view);
            }
            _ => {
                let tuple = self.tuple;
                let faces = ThemedFaces {
                    text,
                    tuple,
                    tokens,
                };
                u.ids.theme.faces.insert(self.view, faces);
            }
        }
        painted
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
        if !ok {
            self.unimply(style);
        }
        match named {
            true => u.ids.theme.put(
                self.view,
                ThemedRow {
                    style,
                    text: text.clone(),
                    tuple: self.tuple,
                    written: binding.written,
                    implied: false,
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
        self.unimply(style);
    }

    /// The binding's clear (or a value the row refused) cleared `style`'s
    /// row, an implied one with it: a boundary sets it again (`boundary_rows`),
    /// unwritten, though what it implies has not changed since.
    fn unimply(&mut self, style: StyleId) {
        let k = INHERITED.iter().position(|s| *s == style);
        if let (Some(k), Some(implied)) = (k, self.rare.as_mut().and_then(|r| r.implied.as_mut())) {
            implied[k] = None;
        }
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
        let (view, tuple) = (self.view, self.tuple);
        for (k, style) in INHERITED.into_iter().enumerate() {
            // Its own row stands: an implied one is set again when it goes.
            if binding_of(plan, &row, style).is_some_and(|b| self.themed.sets(b)) {
                if let Some(implied) = self.rare.as_mut().and_then(|r| r.implied.as_mut()) {
                    implied[k] = None;
                }
                continue;
            }
            let want = u.theme.inherited[k].clone();
            let implied = self.rare().implied.get_or_insert_with(Default::default);
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
                            view,
                            ThemedRow {
                                style,
                                text: text.clone(),
                                tuple,
                                written: false,
                                implied: true,
                                tokens,
                            },
                        );
                    }
                }
                None if implied[k].is_some() => {
                    let mut mask = exact_kernel::StyleMask::default();
                    mask.set(style);
                    u.ops.push(exact_kernel::Op::ClearStyle {
                        id: view,
                        mask,
                        written: false,
                    });
                    u.ids.theme.drop_row(view, style);
                }
                None => {}
            }
            implied[k] = want;
        }
    }
}
