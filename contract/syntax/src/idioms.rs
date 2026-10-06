//! What the web's list idioms become in Contract, for the refusals that meet
//! them (LLP 1017.003 §Diagnostics): an agent or a web developer writes
//! `xs.map(f)`, `xs.length`, `reduce`, `Math.min(...xs)` or `toPrecision` first,
//! and each refusal names the spelling that works or says what to do instead.

/// The fix for `recv.name(…)`, or for `recv.name` read as a field of a value
/// that has none: Contract has no methods.
pub fn method_fix(name: &str) -> String {
    match name {
        "map" | "filter" => format!(
            "write `{name}(xs, (x, i) => …)`: Contract spells the web's array methods as roster functions (`includes(s, t)` is `s.includes(t)`)"
        ),
        "join" => "write `join(xs, \", \")`: Contract spells the web's array methods as roster functions (`includes(s, t)` is `s.includes(t)`)".into(),
        "length" => "write `length(xs)`: Contract spells the web's `.length` as a roster function".into(),
        "includes" => "write `includes(s, t)` for text, `includes(xs, x)` for a list (LLP 1088 §9.1)".into(),
        "startsWith" | "endsWith" => format!("write `{name}(s, t)`"),
        "toString" => "write `toString(x)`".into(),
        "trim" => "write `trim(s)`".into(),
        "toLowerCase" => "write `toLowerCase(s)`".into(),
        "slice" => "write `slice(s, start, end)` for text or a list (`end` may be left out)".into(),
        "concat" => "write `concat(xs, ys)`: Contract spells the web's array methods as roster functions".into(),
        "indexOf" => "write `indexOf(s, t)` for text, `indexOf(xs, x)` for a list (LLP 1088 §9.1)".into(),
        "split" => "write `split(s, sep)`: the web's `split` with a string separator; Contract has no regular expressions".into(),
        "replace" | "replaceAll" => "write `replaceAll(s, find, with)`: the web's `replaceAll` with a string `find`; Contract has no regular expressions".into(),
        "at" => "write `at(xs, i)`: Contract spells the web's `xs.at(i)` as a roster function, `some` of the item or `none`".into(),
        "toFixed" => "write `toFixed(x, digits)`, `digits` a whole-number literal from 0 to 100: the web's `x.toFixed(digits)` as a roster function; for money, `formatDecimal(cents, 2)` prints a count of cents exactly (`round(price * 100)` of a price of at most two decimals) (LLP 1102 §3.2)".into(),
        _ => match refusal(name) {
            Some(why) => why,
            None => format!(
                "Contract has no methods: `.` reads a record's field; call a roster function as `{name}(x, …)`"
            ),
        },
    }
}

/// Why a list operation or number formatter the web has is refused, and what
/// to do instead; `None` for any other name.
pub fn refusal(name: &str) -> Option<String> {
    Some(match name {
        "reduce" | "reduceRight" | "find" | "findIndex" | "findLast" | "some" | "every"
        | "sort" | "toSorted" | "flatMap" | "flat" | "forEach" | "reverse" | "toReversed" => format!(
            "`{name}` is refused (LLP 1017.003: lists have `map`, `filter`, `join`, `concat`, `slice`, `includes` and `indexOf`); compute it in the data source and hand the view the result, as the crypto port keeps `lo` and `hi` beside its series"
        ),
        // @ref LLP 1088 D7.4 — the web's string and list spellings an agent
        // reaches for first, each with what to write today.
        "push" | "append" | "unshift" => format!(
            "a list is a value in Contract, not changed in place: `{name}` is `concat`, as in `xs = concat(xs, [x])` (LLP 1088 §9.1)"
        ),
        "substring" | "substr" => format!(
            "`{name}` is not in Contract: write `slice(s, start, end)`, the web's `String.prototype.slice` (LLP 1088 D2)"
        ),
        "padStart" | "padEnd" => format!(
            "`{name}` is not in Contract (LLP 1088 D2 defers it): pad in the data module, or align with CSS (`text-align`, a fixed `width`)"
        ),
        "toUpperCase" => "display casing is CSS: `text-transform=\"uppercase\"`; Contract has no `toUpperCase` (LLP 1088 D2)".into(),
        "Number" | "parseInt" | "parseFloat" => format!(
            "write `parseNumber(s)` for `{name}`: `some(n)` for a decimal number in the text (`\" 12.5 \"`, `\"-3\"`, `\"1e3\"`), `none` for anything else (`\"\"`, `\"12px\"`, hex), so `match parseNumber(s)` handles a field's text (LLP 1102 §3.1)"
        ),
        "len" => "write `length(x)`: Contract spells the web's `.length`, of text or of a list, as a roster function".into(),
        "min" | "max" => format!(
            "write `{name}(a, b)` for two numbers; `Math.{name}(...xs)` over a list is refused (LLP 1017.003): compute it in the data source, as the crypto port keeps `lo` and `hi` beside its series"
        ),
        // The roster's substring search under the web's name since
        // 2026-09-28; a hint, not a second spelling: `contains` does not compile.
        "contains" => "write `includes(s, t)`, or `includes(xs, x)` for a list: the roster's search wears the web's name, `String.prototype.includes` and `Array.prototype.includes` (LLP 1006 §Expressions, renamed from `contains` 2026-09-28); `startsWith(s, t)` and `endsWith(s, t)` are the web's too".into(),
        "toPrecision" | "toExponential" => format!(
            "`{name}` is not in Contract: `toFixed(x, digits)` is the web's fixed-decimal format (`toFixed(2.5, 1)` is `\"2.5\"`), `formatDecimal(cents, 2)` prints a count of cents exactly, and `formatNumber(n, \"compact\")` prints a count as `1.2K` (LLP 1102 §3.2)"
        ),
        _ => return None,
    })
}
