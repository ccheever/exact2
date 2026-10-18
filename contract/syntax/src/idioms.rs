//! What the web's list idioms become in Contract, for the refusals that meet
//! them (LLP 1017.003 §Diagnostics): an agent or a web developer writes
//! `reduce`, `Math.min(...xs)` or `toPrecision` first, and each refusal names
//! the spelling that works or says what to do instead.

/// Why `now()` is refused (`type-now-renamed`), with both repairs: Contract's
/// clock is the web's `performance.now()`, so it wears that name, and the
/// date is the boot instant plus it.
// @ref LLP 1109 D1 — `now()` read as `Date.now()`; deleted, not deprecated.
pub const NOW_REFUSED: &str = "`now()` is not in Contract: write `performanceNow()` for durations and timers (milliseconds since this launch of the app, restarting at 0 on every launch, the web's `performance.now()`), or `time.epochAtZero + performanceNow()` for the date, with `resource time = exactTime() as shape Clock` declared and `shape Clock` holding `epochAtZero: number`";

/// The roster functions written as a call on their first argument, the web's
/// method of the same name and meaning (@ref LLP 1116.015 D2): `s.trim()`,
/// `xs.map(x => …)`. `length` is the property `xs.length`.
pub const METHODS: [&str; 19] = [
    "isEmpty",
    "toString",
    "includes",
    "startsWith",
    "endsWith",
    "trim",
    "toLowerCase",
    "toUpperCase",
    "slice",
    "concat",
    "indexOf",
    "split",
    "replaceAll",
    "at",
    "map",
    "filter",
    "join",
    "toFixed",
    "flat",
];

/// Whether `name` is written `x.name(…)`.
pub fn is_method(name: &str) -> bool {
    METHODS.contains(&name)
}

/// The refusal of `name(x, …)` for a function written on its value: the edit,
/// with the author's receiver when it is a name.
pub fn method_form(name: &str, args: &[crate::Expr]) -> String {
    let recv = match args.first() {
        Some(crate::Expr::Ident(n, _)) => n.as_str(),
        _ => "x",
    };
    if name == "length" {
        return format!(
            "write `{recv}.length`: a list's or text's length is its `.length`, as on the web"
        );
    }
    let rest = if args.len() > 1 { "…" } else { "" };
    format!(
        "write `{recv}.{name}({rest})`: `{name}` is called on its value, as the web's method is"
    )
}

/// Argument `i` (from 0) of a call to `name`, as the author wrote the call: a
/// method's value, then its arguments counted inside its parentheses.
pub fn argument(name: &str, i: usize) -> String {
    match (is_method(name) || name == "length", i) {
        (true, 0) => format!("the value before `.{name}`"),
        (true, _) => format!("argument {i} of `.{name}(…)`"),
        (false, _) => format!("argument {} of `{name}`", i + 1),
    }
}

/// The fix for `recv.name(…)` that is no method, or for `recv.name` read as a
/// field of a value that has none.
pub fn method_fix(name: &str) -> String {
    match name {
        _ if is_method(name) => format!("call it: `.{name}(…)`"),
        "replace" => "write `s.replaceAll(find, with)`: the web's `replaceAll` with a string `find`; Contract has no regular expressions".into(),
        // `Date.now()`, `performance.now()`.
        "now" => NOW_REFUSED.into(),
        // @ref LLP 1116.016 §5 — Exact's function, as `sum` is.
        "sortBy" => "write `sortBy(xs, x => key)`: Exact's own function, not a method of the list (a method is TypeScript's, and TypeScript has no `sortBy`)".into(),
        _ => match refusal(name) {
            Some(why) => why,
            None => match nearest_method(name) {
                Some(m) => format!("did you mean `.{m}(…)`?"),
                None => format!(
                    "`.` reads a record's field, and `{name}` is not one of the methods Contract has (`contract vocab {name}` says what is)"
                ),
            },
        },
    }
}

/// The method a misspelling means: one edit away (or a swapped pair), ignoring case.
fn nearest_method(name: &str) -> Option<&'static str> {
    let a: Vec<char> = name.to_lowercase().chars().collect();
    METHODS.iter().copied().find(|m| {
        let b: Vec<char> = m.to_lowercase().chars().collect();
        let (short, long) = if a.len() <= b.len() {
            (&a, &b)
        } else {
            (&b, &a)
        };
        match long.len() - short.len() {
            0 => {
                let diff: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
                diff.len() == 1
                    || (diff.len() == 2
                        && diff[1] == diff[0] + 1
                        && a[diff[0]] == b[diff[1]]
                        && a[diff[1]] == b[diff[0]])
            }
            1 => (0..long.len()).any(|i| {
                let mut l = long.clone();
                l.remove(i);
                &l == short
            }),
            _ => false,
        }
    })
}

/// Why a list operation or number formatter the web has is refused, and what
/// to do instead; `None` for any other name.
pub fn refusal(name: &str) -> Option<String> {
    Some(match name {
        // The four a view reaches for most are compositions of the roster:
        // say which, then where the rest belongs.
        "find" | "findIndex" | "some" | "every" => {
            let instead = match name {
                "find" => "`first(xs.filter(x => …))`, an `option` to `match`",
                "findIndex" => "`xs.map(x => …).indexOf(true)`, `-1` for none",
                "some" => "`xs.map(x => …).includes(true)`",
                _ => "`not xs.map(x => …).includes(false)`",
            };
            format!(
                "`{name}` is refused (LLP 1017.003: lists have `map`, `filter`, `join`, `concat`, `slice`, `includes` and `indexOf`): write {instead}, or compute it in the data source and hand the view the result"
            )
        }
        // @ref LLP 1116.016 §5 — a comparator's result is left to each
        // engine when it is inconsistent; one key, stable, is the same everywhere.
        "sort" | "toSorted" => format!(
            "`{name}` is refused: a list is never changed in place, and a comparator is not Contract's; write `sortBy(xs, x => x.name)`, a stable sort by one number or string key (`sortBy(xs, x => x.count, \"desc\")` for descending, equal keys keeping their order, so a second key is an earlier `sortBy`). A locale's collation, a stored or canonical order, and rows the view has not loaded are still the data module's: order the list in the data source and name the order where the view reads it (`resource tasks = loadTasksByDueDate()`)"
        ),
        // LLP 1116.016 §7: told only to sort in the data source, a builder
        // writing the view alone dropped the order; named, it kept it.
        "reverse" | "toReversed" => format!(
            "`{name}` is refused (LLP 1017.003: lists have `map`, `filter`, `join`, `concat`, `slice`, `includes` and `indexOf`): order the list in the data source and name the order where the view reads it (`resource tasks = loadTasksByDueDate()`)"
        ),
        "reduce" | "reduceRight" => format!(
            "`{name}` is refused (LLP 1017.003): a total is `sum(xs)`, or `sum(xs.map(x => …))` over a field; compute any other fold in the data source and hand the view the result"
        ),
        // `flat` joined the roster at depth 1 (2026-10-17); `flatMap` is it
        // after `map` when the callback returns a list, and `map` alone when
        // it returns one value, which `flatMap` keeps as it is.
        "flatMap" => "`flatMap` is refused (LLP 1017.003): when the callback returns a list, write `xs.map(x => …).flat()`, which joins them; when it returns one value, `xs.map(x => …)` is the same list".into(),
        "findLast" | "forEach" => format!(
            "`{name}` is refused (LLP 1017.003: lists have `map`, `filter`, `join`, `concat`, `slice`, `includes` and `indexOf`); compute it in the data source and hand the view the result, as the crypto port keeps `lo` and `hi` beside its series"
        ),
        // @ref LLP 1088 D7.4 — the web's string and list spellings an agent
        // reaches for first, each with what to write today.
        "push" | "append" | "unshift" => format!(
            "a list is a value in Contract, not changed in place: `{name}` is `concat`, as in `xs = xs.concat([x])` (LLP 1088 §9.1); a router's is `push(nav, location)`"
        ),
        // @ref LLP 1116.016 D5 — each edit means what the refused form did, or says what differs.
        "localeCompare" => "Contract has no locale order (it would differ by device): `a < b` compares by UTF-16 code unit, as TypeScript's `<` does; an order for people to read is the data source's".into(),
        "floor" | "ceil" | "round" | "sin" | "cos" => format!(
            "write `{name}(n)`: `Math.{name}(n)`, written as a function"
        ),
        // @ref LLP 1027.000 D1 — a draw is a pure function of a seed passed in.
        "random" => "Contract has no ambient randomness (LLP 1027.000 D1): a draw comes from a seed passed to a pure function. For rolls in an expression, copy `roll(seed, k, n)` from docs/recipes/random-rolls.md: `1 + roll(time.seed, rolls, 6)` is a die, with `time` an `exactTime()` resource and `rolls` a counter in state; a deal or a shuffle is the data module's (docs/recipes/random-and-shuffle.md)".into(),
        "String" => "write `x.toString()`, or a template, `` `${x}` ``".into(),
        "useEffect" | "useState" | "useMemo" | "watch" | "effect" | "onChange" => format!(
            "Contract has no `{name}`, nor code that runs because a value changed (LLP 1116.016 D0): an action runs on an event and assigns what changes, a `derive` is a value computed from others, and a timer is a gated `task` (`task t when cond` with `after(ms, action)`)"
        ),
        "substring" | "substr" => format!(
            "`{name}` is not in Contract: write `s.slice(start, end)`, the web's `String.prototype.slice` (LLP 1088 D2)"
        ),
        "padStart" | "padEnd" => format!(
            "`{name}` is not in Contract (LLP 1088 D2 defers it): pad in the data module, or align with CSS (`text-align`, a fixed `width`)"
        ),
        // Round 2's builders looked for `pow` (LLP 1116.000 D5).
        "pow" | "sqrt" | "abs" => format!(
            "`{name}` is not in Contract: `x * x` is a square and `max(x, -x)` a magnitude; compute other maths in the data source and hand the view the result"
        ),
        // LLP 1116.018: repairs changed what an unreadable field shows
        // without saying so; the message names where the two differ.
        "Number" | "parseInt" | "parseFloat" => {
            let differs = match name {
                "Number" => "`Number` reads `\"\"` as 0",
                "parseInt" => "`parseInt` reads `\"12.5\"` and `\"12px\"` as 12",
                _ => "`parseFloat` reads `\"12px\"` as 12 and `\"\"` as `NaN`",
            };
            format!(
                "write `parseNumber(s)` for `{name}`: `some(n)` for a decimal number in the text (`\" 12.5 \"`, `\"-3\"`, `\"1e3\"`), `none` for anything else (`\"\"`, `\"12px\"`, hex), so `match parseNumber(s) {{ case some(n) => n, case none => 0 }}` says what an unreadable field counts as; {differs} (LLP 1102 §3.1)"
            )
        }
        "len" | "count" | "size" => "write `x.length`, of text or of a list".into(),
        "min" | "max" => format!(
            "write `{name}(a, b)` for two numbers; `Math.{name}(...xs)` over a list is refused (LLP 1017.003): compute it in the data source, as the crypto port keeps `lo` and `hi` beside its series"
        ),
        // The roster's substring search under the web's name since
        // 2026-09-28; a hint, not a second spelling: `contains` does not compile.
        "contains" => "write `s.includes(t)`, or `xs.includes(x)` for a list: the web's `String.prototype.includes` and `Array.prototype.includes`; `s.startsWith(t)` and `s.endsWith(t)` are the web's too".into(),
        "toLocaleString" => "`toLocaleString` is not in Contract: `formatNumber(n, \"decimal\")` is `Intl.NumberFormat(\"en-US\")`'s grouped number (`1,234.5`), `formatNumber(n, \"currency\", \"USD\")` its money (`$1,481.47`) and `formatNumber(n, \"percent\")` its percent (`26%`); `formatDate` prints a date (LLP 1116 D8)".into(),
        "toPrecision" | "toExponential" => format!(
            "`{name}` is not in Contract: `x.toFixed(digits)` is the web's fixed-decimal format (`(2.5).toFixed(1)` is `\"2.5\"`), `formatDecimal(cents, 2)` prints a count of cents exactly, and `formatNumber(n, \"compact\")` prints a count as `1.2K` (LLP 1102 §3.2)"
        ),
        _ => return None,
    })
}
