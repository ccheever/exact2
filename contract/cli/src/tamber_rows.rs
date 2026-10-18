//! `set-row` (LLP 1122 D6, T5; LLP 1121 stage 9 D): one row of the app's
//! own Contract written where its author wrote it. The template rung names
//! the attribute by its place in its file and the file's digest as the
//! tool saw it (the map's `at` and `sources`); the style rung names a
//! `style` and its row. Only the value is replaced; the file then goes
//! through `contract fmt` and the whole app is compiled before anything is
//! written (`tamber_contract::plan`).

use contract_syntax::{Attr, Expr, Node, UnOp};
use exact_kernel::StyleId;

/// One `set-row` at an attribute's place.
#[derive(Debug, Clone)]
pub(crate) struct AtRow {
    /// The attribute's line and column (its name's start).
    pub line: u32,
    pub col: u32,
    /// The file's digest as the tool saw it.
    pub sha256: String,
    /// The attribute's name as written (`padding` for an expansion).
    pub row: String,
    /// A state part's row: its state, without the `:`.
    pub state: Option<String>,
    /// The value as Contract writes it.
    pub value: String,
}

/// The attribute at `line`, `col` in `file`: a template node's or a
/// `style`'s, a state part's row included.
fn attr_at(file: &contract_syntax::File, line: u32, col: u32) -> Option<&Attr> {
    fn walk(nodes: &[Node], line: u32, col: u32) -> Option<&Attr> {
        nodes.iter().find_map(|n| match n {
            Node::Element {
                attrs, children, ..
            } => attrs
                .iter()
                .find(|a| a.written && a.span.line == line && a.span.col == col)
                .or_else(|| walk(children, line, col)),
            Node::Use { children, .. } => walk(children, line, col),
            Node::When {
                then, otherwise, ..
            } => walk(then, line, col).or_else(|| walk(otherwise, line, col)),
            Node::Each { body, .. } => walk(body, line, col),
            Node::Match { some, none, .. } => {
                walk(&some.1, line, col).or_else(|| walk(none, line, col))
            }
            Node::Children { .. } => None,
        })
    }
    let styled = file.styles.iter().flat_map(|s| &s.attrs);
    styled
        .clone()
        .find(|a| a.written && a.span.line == line && a.span.col == col)
        .or_else(|| {
            file.components
                .iter()
                .find_map(|c| walk(&c.view, line, col))
        })
}

/// A refusal and where in its file: the attribute's place where one is
/// known, else the file's start.
#[derive(Debug, Clone)]
pub(crate) struct Refusal {
    pub message: String,
    pub line: u32,
    pub col: u32,
}

impl From<String> for Refusal {
    fn from(message: String) -> Self {
        Refusal {
            message,
            line: 1,
            col: 1,
        }
    }
}

impl Refusal {
    fn at(message: String, line: u32, col: u32) -> Self {
        Refusal { message, line, col }
    }
}

/// Whether a written value is one `set-row` replaces: a literal (a
/// token's or param's name is a string, `"param.fill"`), a negative number.
/// A name in scope (state, a prop) or any expression is the author's.
fn plain(e: &Expr) -> bool {
    match e {
        Expr::Number(..) | Expr::Str(..) => true,
        Expr::Unary(UnOp::Neg, inner, _) => matches!(**inner, Expr::Number(..)),
        _ => false,
    }
}

/// A layout row's refusal (LLP 1122 D14), and where to edit it instead.
fn layout(row: &str) -> String {
    format!("`{row}` is a layout row: layout is the author's (LLP 1122 D14); edit it in the source, or at a param's or token's rung where it reads one")
}

/// Whether `set-row` edits `a`'s row on a style: a paint row (history
/// reads these as a style's own rows; a layout row or a part's is code).
pub(crate) fn paint_row(a: &Attr) -> bool {
    let rows = contract_lower::tags::rows(a);
    !rows.is_empty() && !rows.iter().any(|r| r.affects_layout())
}

/// Refused unless `a` is a paint row's attribute named `row` (in `state`'s
/// part when one is named); each refusal says why and what to do instead.
/// `all` is the file's lines, for a shorthand's current value as written.
pub(crate) fn check(
    a: &Attr,
    row: &str,
    state: Option<&str>,
    all: &[String],
) -> Result<(), String> {
    let (said, property) = match contract_syntax::state::state_of(&a.name) {
        Some((s, p)) => (Some(s), p),
        None => (None, a.name.as_str()),
    };
    let bare = |name: &str| Attr {
        name: name.to_string(),
        ..a.clone()
    };
    let rows = contract_lower::tags::rows(&bare(property));
    if property != row || said != state {
        // A longhand of the shorthand written there (LLP 1122 D6): the
        // shorthand is edited whole, unless it is a layout row; its
        // current text lets the tool compose the new one.
        let longhand = contract_lower::tags::rows(&bare(row));
        if said == state && !longhand.is_empty() && longhand.iter().all(|r| rows.contains(r)) {
            if rows.iter().any(|r: &StyleId| r.affects_layout()) {
                return Err(layout(property));
            }
            let line = &all[a.span.line as usize - 1];
            let current = value_range(all, a, &format!("`{property}`"))
                .map(|(from, close)| line[from..close].to_string())
                .unwrap_or_else(|why| format!("… ({why})"));
            return Err(format!(
                "`{row}` is written in the shorthand `{property}={current}`: set-row edits `{property}`, its whole value"
            ));
        }
        let named = |s: Option<&str>, r: &str| match s {
            Some(s) => format!("`{r}` in `:{s}`"),
            None => format!("`{r}`"),
        };
        return Err(format!(
            "the attribute there is {}, not {}: the file moved under the selection; select it again",
            named(said, property),
            named(state, row)
        ));
    }
    if rows.is_empty() {
        return Err(format!(
            "`{row}` is no style row but a prop or a handler: set-row edits a row; edit it in the source"
        ));
    }
    if rows.iter().any(|r: &StyleId| r.affects_layout()) {
        return Err(layout(row));
    }
    if !plain(&a.value) {
        return Err(format!(
            "`{row}` is written as an expression: edit it in the source, or at a param's or token's rung where it reads one"
        ));
    }
    Ok(())
}

/// The value range of `a` on its line in `all`: from the value as written
/// (any `-` or parentheses that open it) to the end of its parentheses.
pub(crate) fn value_range(all: &[String], a: &Attr, what: &str) -> Result<(usize, usize), String> {
    let at = |line: u32| line as usize - 1;
    let (v, l) = (a.value.span(), &all[at(a.span.line)]);
    let (start, end) = (v.col as usize - 1, v.end_col as usize - 1);
    let key = (a.span.col as usize - 1).min(l.len());
    let over = || format!("{what} is written over more than one line: edit it by hand");
    if v.line != a.span.line || start < key || end < start || end > l.len() {
        return Err(over());
    }
    let open = crate::tamber_contract::depth(&l[key..end]);
    let mut close = end;
    for _ in 0..open {
        match l[close..].trim_start_matches([' ', '\t']).strip_prefix(')') {
            Some(after) => close = l.len() - after.len(),
            None => {
                return Err(format!(
                    "{what}: its parentheses close on another line: edit it by hand"
                ))
            }
        }
    }
    // The attribute's own parentheses close here; a neighbour's are its own.
    // A sign whose number is below it (`opacity=-` then `0.3`, or `-(`
    // then `0.3)`: the value's span is the sign) is a value over more
    // lines too.
    if crate::tamber_contract::depth(&l[key..close]) != 0
        || l[start..close].trim_end().ends_with('-')
    {
        return Err(over());
    }
    let eq = l[key..start].find('=').map_or(key, |i| key + i + 1);
    let from = eq + (l[eq..start].len() - l[eq..start].trim_start().len());
    Ok((from, close))
}

/// The `set-row`s at places in one file applied to its text as it is,
/// from the last place up so each place still holds; refused where the
/// file moved since the tool saw it or a place holds another attribute.
/// The caller formats the result.
pub(crate) fn splice(text: &str, rows: &[&AtRow]) -> Result<String, Refusal> {
    let file = contract_syntax::parse(text).map_err(|e| e.message)?;
    let digest = contract::plan_digest(text.as_bytes());
    if let Some(stale) = rows.iter().find(|r| r.sha256 != digest) {
        return Err(Refusal::at(
            format!(
                "the file changed since line {} was selected: select it again once it has reloaded",
                stale.line
            ),
            stale.line,
            stale.col,
        ));
    }
    // One place once: two writes to one attribute would splice the second
    // into the first's text.
    for (i, r) in rows.iter().enumerate() {
        if rows[..i].iter().any(|q| (q.line, q.col) == (r.line, r.col)) {
            return Err(Refusal::at(
                format!(
                    "two set-rows name the attribute at line {}, column {}: send one",
                    r.line, r.col
                ),
                r.line,
                r.col,
            ));
        }
    }
    let newline = text.ends_with('\n');
    let mut all: Vec<String> = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .map(str::to_string)
        .collect();
    let mut order: Vec<&&AtRow> = rows.iter().collect();
    order.sort_by_key(|r| std::cmp::Reverse((r.line, r.col)));
    for r in order {
        let a = attr_at(&file, r.line, r.col).ok_or_else(|| {
            let message = format!(
                "no attribute an author wrote at line {}, column {}: a tag's or the compiler's row is the element's default (write the row on the node), else the file moved; select it again",
                r.line, r.col
            );
            Refusal::at(message, r.line, r.col)
        })?;
        let here = |m: String| Refusal::at(m, r.line, r.col);
        check(a, &r.row, r.state.as_deref(), &all).map_err(here)?;
        let (from, close) = value_range(&all, a, &format!("`{}`", r.row)).map_err(here)?;
        all[r.line as usize - 1].replace_range(from..close, &r.value);
    }
    let mut out = all.join("\n");
    if newline {
        out.push('\n');
    }
    Ok(out)
}

/// The non-state row `row` of `style` set to `value`, where it is written.
pub(crate) fn style_row(
    text: &str,
    style: &str,
    row: &str,
    value: &str,
) -> Result<String, Refusal> {
    let file = contract_syntax::parse(text).map_err(|e| e.message)?;
    let decl = file
        .styles
        .iter()
        .find(|s| s.name == style)
        .ok_or_else(|| format!("no `style {style}` in this file"))?;
    // The row, or the shorthand it is written in (refused below, named).
    let longhand = |a: &Attr| {
        let wanted = contract_lower::tags::rows(&Attr {
            name: row.to_string(),
            ..a.clone()
        });
        let have = contract_lower::tags::rows(a);
        !wanted.is_empty() && wanted.iter().all(|r| have.contains(r))
    };
    let own = |a: &&Attr| a.written && contract_syntax::state::state_of(&a.name).is_none();
    // The row as written (the last of that name: a revert names it), else
    // the last shorthand that writes it, as lowering keeps the last binding.
    let a = decl
        .attrs
        .iter()
        .filter(own)
        .rev()
        .find(|a| a.name == row)
        .or_else(|| decl.attrs.iter().filter(own).rev().find(|a| longhand(a)))
        .ok_or_else(|| Refusal::at(format!("`style {style}` writes no `{row}`: set-row edits a row it has; add it in the source, or set it on the node"), decl.span.line, decl.span.col))?;
    let newline = text.ends_with('\n');
    let mut all: Vec<String> = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .map(str::to_string)
        .collect();
    let here = |m: String| Refusal::at(m, a.span.line, a.span.col);
    check(a, row, None, &all).map_err(here)?;
    let (from, close) =
        value_range(&all, a, &format!("`{row}` in `style {style}`")).map_err(here)?;
    all[a.span.line as usize - 1].replace_range(from..close, value);
    let mut out = all.join("\n");
    if newline {
        out.push('\n');
    }
    Ok(out)
}
