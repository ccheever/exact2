//! The base scale in lowering (LLP 1121 stage 11 B, theme2's condition (c)):
//! the compiler knows the nine base names (`exact_kernel::style::scale`), so
//! a view reading one compiles with no design defining it, as a role's name
//! does, and the kernel reads its length in the process's column. The plan
//! keeps nothing for one: the kernel holds every column.

use exact_kernel::style::{metrics::Column, scale};
use exact_tamber as tamber;

thread_local! {
    /// Every token name the design declares, of any kind, while its styles
    /// are lowered: a base name in it is the design's, never the scale's.
    static DECLARED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The design's declared token names, for [`values`] (`theme::declare_theme`
/// and the lint's scope set them).
pub(crate) fn declare(names: Vec<String>) {
    DECLARED.with(|d| *d.borrow_mut() = names);
}

/// The names [`declare`] last set (the design server's write check puts
/// them back after its own).
pub(crate) fn declared() -> Vec<String> {
    DECLARED.with(|d| d.borrow().clone())
}

/// The base scale's values for `text` when it is a base name the design
/// does not define: each px column's length, distinct, as a token's values
/// over the tuples are (what a row checks). A base varies by column, never
/// by an app axis, so it is no `tamber-inherited-axis`. `None` for any
/// other text.
pub(crate) fn values(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let row = scale::row(text)?;
    if DECLARED.with(|d| d.borrow().iter().any(|n| n == text)) {
        return None;
    }
    let mut values: Vec<String> = Vec::new();
    for (_, column) in scale::COLUMNS
        .iter()
        .filter(|(_, c)| *c != Column::Terminal)
    {
        let v = scale::css(scale::cell(row, *column).value);
        if !values.contains(&v) {
            values.push(v);
        }
    }
    Some(values)
}

/// A length token's value over base names the design leaves unset (LLP
/// 1121 D11, stage 12 B2): its px in each column, by the column's name, as
/// that column's host folds it (`scale::evaluate`, the terminal's cells as
/// px, which its spaces then round as every space there does). `None` for
/// any other text.
pub(crate) fn columns(value: &str) -> Option<Vec<(&'static str, f32)>> {
    let value = value.trim();
    if !value.contains('(') || scale::named_in(value).is_empty() {
        return None;
    }
    scale::COLUMNS
        .iter()
        .map(|(name, column)| Some((*name, scale::evaluate(value, *column)?)))
        .collect()
}

/// Why an expression [`unfolded`] names doesn't fold.
pub(crate) const UNFOLDED: &str =
    "past the kernel's `calc()` limits (32 values, 32 levels) or past the lengths a host holds";

/// The first column a length's expression over base names doesn't fold
/// in (past the kernel's `calc()` limits, `MAX_VALUES` terms): what the
/// build refuses, so no host meets a value it can't read. `None` for one
/// that folds in every column, or any other text.
pub(crate) fn unfolded(value: &str) -> Option<&'static str> {
    let value = value.trim();
    if !value.contains('(') || scale::named_in(value).is_empty() {
        return None;
    }
    scale::COLUMNS
        .iter()
        .find(|(_, column)| scale::evaluate(value, *column).is_none())
        .map(|(name, _)| *name)
}

/// A shadow token's value over base names the design leaves unset (LLP
/// 1121 stage 12 B3): its text in each column, by the column's name, each
/// length over one folded to px as that column's host folds it
/// (`scale::fold_lengths`), `None` in a column it doesn't fold in. `None`
/// for any other text.
pub(crate) fn shadow_columns(value: &str) -> Option<Vec<(&'static str, Option<String>)>> {
    let value = value.trim();
    if scale::named_in(value).is_empty() {
        return None;
    }
    let each =
        |column: Column| scale::fold_lengths(value, |n| scale::of(n, column).map(scale::css));
    Some(
        scale::COLUMNS
            .iter()
            .map(|(name, column)| (*name, each(*column)))
            .collect(),
    )
}

/// Why a token over base names the design leaves unset is refused, with
/// its id: a length's expression that doesn't fold in a column (stage 12
/// B2), or a shadow whose lengths don't fold in one, or whose px there the
/// row's codec refuses, a blur gone negative (B3). The first column it
/// fails in is named, so no host meets a value it can't read. `None` for
/// one every column reads, or any other value.
pub(crate) fn refusal(kind: tamber::Kind, name: &str, v: &str) -> Option<(&'static str, String)> {
    use exact_kernel::style::{BoxShadows, TextShadow};
    if kind == tamber::Kind::Length {
        let column = unfolded(v)?;
        return Some((
            "tamber-kind",
            format!("`{name}` is `{v}`, which doesn't fold to a length on {column}: {UNFOLDED}"),
        ));
    }
    let check = match kind {
        tamber::Kind::BoxShadow => |t: &str| BoxShadows::check(t).map(drop),
        tamber::Kind::TextShadow => |t: &str| TextShadow::check(t).map(drop),
        _ => return None,
    };
    shadow_columns(v)?.into_iter().find_map(|(column, text)| {
        let message = match text {
            None => format!("`{name}` is `{v}`, whose lengths don't fold on {column}: {UNFOLDED}"),
            Some(text) => {
                let why = check(&text).err()?;
                format!("`{name}` is `{v}`, the shadow `{text}` on {column}, which no host draws: {why}")
            }
        };
        Some(("tamber-base-expression", message))
    })
}

/// [`columns`], and a bare base name the design leaves unset's too: what a
/// ranged param checks (stage 12 B2, theme2's "ranges included").
pub(crate) fn ranged(value: &str) -> Option<Vec<(&'static str, f32)>> {
    let value = value.trim();
    if !scale::is_base(value) {
        return columns(value);
    }
    let calc = format!("calc({value})");
    scale::COLUMNS
        .iter()
        .map(|(name, column)| Some((*name, scale::evaluate(&calc, *column)?)))
        .collect()
}

/// px as a length's text: `-2px`.
pub(crate) fn px(n: f32) -> String {
    format!("{}px", exact_num::Shortest32(n))
}

/// Value `v` of a token of `kind` in each column it is over base names in,
/// by the column's name: a length's px, a shadow's text with its lengths
/// px (stage 12 B3). `None` for one over no base name.
fn by_column(kind: tamber::Kind, v: &str) -> Option<Vec<(&'static str, String)>> {
    match kind {
        tamber::Kind::BoxShadow | tamber::Kind::TextShadow => Some(
            shadow_columns(v)?
                .into_iter()
                .filter_map(|(name, text)| Some((name, text?)))
                .collect(),
        ),
        _ => Some(
            columns(v)?
                .into_iter()
                .map(|(name, n)| (name, px(n)))
                .collect(),
        ),
    }
}

/// A token's values, each over base names as its columns' (what a row
/// checks: every column's, as every tuple's), distinct.
pub(crate) fn expanded(kind: tamber::Kind, values: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        let each = match by_column(kind, v) {
            Some(columns) => columns.into_iter().map(|(_, v)| v).collect(),
            None => vec![v.clone()],
        };
        for v in each {
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// Where among `values` (a token's of `kind`, over its tuples) one is
/// `px_text`: the columns a value over base names is it in, said with the
/// value (`on mac (`calc(space.md - 12px)`)`); `None` when none is.
pub(crate) fn place(kind: tamber::Kind, values: &[String], px_text: &str) -> Option<String> {
    values.iter().find_map(|v| {
        let on: Vec<&str> = by_column(kind, v)?
            .into_iter()
            .filter(|(_, text)| text == px_text)
            .map(|(name, _)| name)
            .collect();
        (!on.is_empty()).then(|| format!("on {} (`{}`)", on.join(", "), v.trim()))
    })
}

/// The base names' lengths outside Exact (the DTCG export, `tamber gen`'s
/// CSS), where no host's column applies: the neutral column's, the web's
/// look (round 2, Astra 2), px at the default root, from the kernel's table.
pub(crate) fn sizes() -> impl Iterator<Item = (String, String)> {
    scale::names().map(|n| {
        let px = match scale::of(n, Column::Neutral) {
            Some(scale::ScaleValue::Px(px)) => px,
            _ => scale::RADIUS_FULL,
        };
        (n.to_string(), exact_num::Shortest(px).to_string())
    })
}
