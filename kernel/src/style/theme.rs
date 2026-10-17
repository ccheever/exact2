//! An app's `theme` (LLP 1095 D4): its own colour names, and the roles it
//! overrides, resolved where a colour is parsed. A plan's entries are bound
//! while its styles are parsed, as its colour profiles are
//! (`profiled::declarations`): the compiler binds them to check a row, the
//! runner to set one, so `color="brand"` and a string from data naming
//! `brand` read the same colour. A name is read as a whole component (a row,
//! a shadow's colour, a gradient stop, a palette entry), never inside a
//! `light-dark()` half or another function, as LLP 1095 D1 says; names
//! inside expressions are RFC 1121's synthetic tokens, built by the crate.

use std::cell::RefCell;
use std::sync::Arc;

/// A plan's theme as a parse reads it: each entry's name (a role's spelling
/// for an override), the text it stands for, which names no entry, and its
/// kind. Built once per plan; binding one is a pointer swap.
/// A role's spelling matches as roles do, without case; a name exactly. A
/// role drawn in the view's tint is never in it: it stays the role in its
/// row, and a host makes the theme's colour the app's tint.
pub type Table = Arc<[Entry]>;

/// One entry of a [`Table`].
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Its name, or a role's spelling for an override.
    pub name: Box<str>,
    /// The text it stands for, in its kind.
    pub value: Box<str>,
    /// A role's spelling, matched without case.
    pub role: bool,
    /// Which rows read it (LLP 1121 D3).
    pub kind: Kind,
    /// It varies by an app axis (D5): no inherited row reads it, since only
    /// a colour is chosen again at a mode boundary (D7).
    pub varies: bool,
}

/// A token's kind (LLP 1121 D3), as the plan's `theme.kind` numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A colour: read wherever a colour is parsed.
    Color,
    /// A length: px text, one `rem` or `em`, or a named length.
    Length,
    /// A number.
    Number,
    /// A `box-shadow` row's value.
    BoxShadow,
    /// A `text-shadow` row's value.
    TextShadow,
}

impl Kind {
    /// The kind the plan's `theme.kind` numbers; `None` for one this build
    /// does not know.
    pub fn of(code: u8) -> Option<Kind> {
        match code {
            0 => Some(Kind::Color),
            1 => Some(Kind::Length),
            2 => Some(Kind::Number),
            3 => Some(Kind::BoxShadow),
            4 => Some(Kind::TextShadow),
            _ => None,
        }
    }
}

thread_local! {
    static ENTRIES: RefCell<Option<Table>> = const { RefCell::new(None) };
    /// Inside an entry's own parse: what it stands for names no entry.
    static INSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The entries parses read since [`take_named`] last asked, by
    /// [`name_bit`].
    static NAMED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Inside [`recorded`]: the entries parses read, by name.
    static RECORD: RefCell<Option<Vec<Box<str>>>> = const { RefCell::new(None) };
}

/// The entries parses read since the last call, as [`name_bit`]s; 0 for
/// none (LLP 1121 D7): the runner keeps such a row beside its tokens, to set
/// it again when one's slice or value changes.
pub fn take_named() -> u64 {
    NAMED.with(|n| n.replace(0))
}

/// `f`, and the entries its parses read, by name as the table has them (a
/// role's spelling for an override read so): what an inspection shows, where
/// [`take_named`]'s bits may stand for more than one name.
/// What [`take_named`] would answer is left as it was.
pub fn recorded<T>(f: impl FnOnce() -> T) -> (T, Vec<Box<str>>) {
    let named = NAMED.with(|n| n.get());
    let previous = RECORD.with(|r| r.replace(Some(Vec::new())));
    let out = f();
    let names = RECORD.with(|r| r.replace(previous)).unwrap_or_default();
    NAMED.with(|n| n.set(named));
    (out, names)
}

/// One bit of 64 standing for an entry's name, as it is in a table: what a
/// row that read it and an edit of it are matched by (two names may share
/// one; an edit then sets a row again that it did not need to).
pub fn name_bit(name: &str) -> u64 {
    let h = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    1 << (h % 64)
}

/// A parse scope's theme, restored on return or unwinding. The scope is
/// the thread's: every style of a plan is parsed on the thread that bound
/// it (a parse moved to a worker takes its table with it).
pub struct ThemeScope {
    previous: Option<Table>,
    thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// A table of colours from `(name, colour text, a role's spelling)`.
pub fn table<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str, bool)>) -> Table {
    kinded(
        entries
            .into_iter()
            .map(|(n, v, r)| (n, v, r, Kind::Color, false)),
    )
}

/// A table from `(name, text, a role's spelling, kind, whether it varies)`.
pub fn kinded<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a str, bool, Kind, bool)>,
) -> Table {
    entries
        .into_iter()
        .map(|(n, v, r, kind, varies)| Entry {
            name: n.into(),
            value: v.into(),
            role: r,
            kind,
            varies,
        })
        .collect()
}

/// Whether a theme is bound, and `text` names one of its entries: a name
/// exactly, a role's spelling without case.
pub fn names(text: &str) -> bool {
    ENTRIES.with(|e| {
        e.borrow().as_ref().is_some_and(|t| {
            t.iter()
                .any(|e| *e.name == *text || e.role && e.name.eq_ignore_ascii_case(text))
        })
    })
}

/// Whether a theme with any entry is bound.
pub fn bound() -> bool {
    ENTRIES.with(|e| e.borrow().as_ref().is_some_and(|t| !t.is_empty()))
}

/// Bind a plan's table while its styles are parsed.
pub fn bind(table: &Table) -> ThemeScope {
    ThemeScope {
        previous: ENTRIES.with(|e| e.replace(Some(table.clone()))),
        thread: std::marker::PhantomData,
    }
}

/// [`bind`] a table built from `entries`, for a caller that binds once.
pub fn declarations<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str, bool)>) -> ThemeScope {
    bind(&table(entries))
}

impl Drop for ThemeScope {
    fn drop(&mut self) {
        ENTRIES.with(|e| {
            e.replace(self.previous.take());
        });
    }
}

/// Record that a parse read entry `name` ([`take_named`], [`recorded`]).
fn note(name: &str) {
    NAMED.with(|n| n.set(n.get() | name_bit(name)));
    RECORD.with(|r| {
        if let Some(names) = r.borrow_mut().as_mut() {
            if !names.iter().any(|n| **n == *name) {
                names.push(name.into());
            }
        }
    });
}

/// The text entry `text` stands for in the bound theme, of any kind, its
/// read not recorded: what a diagnostic quotes.
pub fn declared(text: &str) -> Option<Box<str>> {
    ENTRIES.with(|e| {
        e.borrow()
            .as_ref()?
            .iter()
            .find(|e| *e.name == *text.trim())
            .map(|e| e.value.clone())
    })
}

/// The text a length or number entry `text` names in the bound theme, and
/// its kind (LLP 1121 D3, D7): a row's whole value naming one of a kind
/// `takes` admits, exactly; the read recorded as a colour's is. `None`
/// when it names none.
pub fn measure(text: &str, takes: impl Fn(Kind, bool) -> bool) -> Option<(Kind, Box<str>)> {
    let text = text.trim();
    if text.is_empty() || text.contains([' ', '(', '#']) {
        return None;
    }
    let (kind, name, value) = ENTRIES.with(|e| {
        let e = e.borrow();
        let e = e
            .as_ref()?
            .iter()
            .find(|e| e.kind != Kind::Color && *e.name == *text && takes(e.kind, e.varies))?;
        Some((e.kind, e.name.clone(), e.value.clone()))
    })?;
    note(&name);
    Some((kind, value))
}

/// The rows a number token may name (LLP 1121 D3).
const NUMBER_ROWS: [crate::StyleId; 5] = [
    crate::StyleId::Opacity,
    crate::StyleId::LineHeight,
    crate::StyleId::FlexGrow,
    crate::StyleId::FontWeight,
    crate::StyleId::PressScale,
];

/// Whether row `id` may name a token of `kind` (LLP 1121 D3): a colour a
/// row that holds a colour; a number the rows D3 lists; a length any other
/// row but those that take only a number (`line-height` takes both); a
/// shadow its own row. The row's codec still reads the value: a text style
/// only as a font size.
pub fn admits(kind: Kind, id: crate::StyleId) -> bool {
    use crate::generated::StyleCodec as C;
    let colour = matches!(
        id.codec(),
        C::ColorValue
            | C::KeywordColor
            | C::Rgba8
            | C::Color2
            | C::SymbolPalette
            | C::Paint
            | C::BackgroundImage
            | C::MaskImage
            | C::BoxShadow
            | C::TextShadow
            | C::Filter
            | C::BackdropFilter
    );
    match kind {
        Kind::Color => colour,
        Kind::Number => NUMBER_ROWS.contains(&id),
        Kind::Length => !colour && (id == crate::StyleId::LineHeight || !NUMBER_ROWS.contains(&id)),
        Kind::BoxShadow => id == crate::StyleId::BoxShadow,
        Kind::TextShadow => id == crate::StyleId::TextShadow,
    }
}

/// Row `id`'s value with a length or number token it names replaced by the
/// token's (LLP 1121 D7): a length's text, which the row's codec and the
/// relative side table read as the literal's; a number's number. `None`
/// when it names no such token, or one the row does not take
/// ([`admits`]), and the value stands for the codec to read or refuse.
pub fn measured(
    id: crate::StyleId,
    value: &super::StyleValue,
) -> Option<super::StyleValue<'static>> {
    let super::StyleValue::Text(text) = value else {
        return None;
    };
    // An inherited row reads no token that varies (D7), as the compiler
    // refuses one a literal names.
    let (kind, value) = measure(text, |kind, varies| {
        admits(kind, id) && !(varies && id.inherited())
    })?;
    Some(match kind {
        Kind::Number => super::StyleValue::Number(exact_num::parse_f64(value.trim()).ok()?),
        _ => super::StyleValue::Text(String::from(value).into()),
    })
}

/// `parse` of the colour text a theme name stands for, in the bound theme;
/// `None` when `text` names no entry. A role's spelling matches as roles do,
/// without case. The entry's own text is parsed with the theme unbound, so a
/// plan cannot make a name stand for itself.
pub(crate) fn resolved<T>(text: &str, parse: impl FnOnce(&str) -> T) -> Option<T> {
    let text = text.trim();
    if INSIDE.with(|i| i.get()) || text.is_empty() {
        return None;
    }
    // A synthetic token (LLP 1121 D3) is named by its whole expression,
    // which the build folded; no other text with these is a name.
    let synthetic = text.contains('(');
    if !synthetic && text.contains([' ', '#']) {
        return None;
    }
    let table = ENTRIES.with(|e| e.borrow().clone())?;
    let Entry { name, value, .. } = table.iter().find(|e| {
        e.kind == Kind::Color
            && (*e.name == *text || !synthetic && e.role && e.name.eq_ignore_ascii_case(text))
    })?;
    note(name);
    INSIDE.with(|i| i.set(true));
    struct Out;
    impl Drop for Out {
        fn drop(&mut self) {
            INSIDE.with(|i| i.set(false));
        }
    }
    let _out = Out;
    Some(parse(value))
}
