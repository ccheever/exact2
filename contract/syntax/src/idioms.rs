//! What the web's list idioms become in Contract, for the refusals that meet
//! them (LLP 1017.003 §Diagnostics): an agent or a web developer writes
//! `xs.map(f)`, `xs.length`, `reduce`, `Math.min(...xs)` or `toFixed` first,
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
        "includes" => "write `includes(s, t)` for text; `includes` on a list is refused (LLP 1017.003)".into(),
        "startsWith" | "endsWith" => format!("write `{name}(s, t)`"),
        "toString" => "write `toString(x)`".into(),
        "trim" => "write `trim(s)`".into(),
        "toLowerCase" => "write `toLowerCase(s)`".into(),
        "slice" => "write `slice(s, start, end)` for text (`end` may be left out); `slice` on a list is not in Contract yet (LLP 1088 §9)".into(),
        "replace" | "replaceAll" => "write `replaceAll(s, find, with)`: the web's `replaceAll` with a string `find`; Contract has no regular expressions".into(),
        "at" => "write `at(xs, i)`: Contract spells the web's `xs.at(i)` as a roster function, `some` of the item or `none`".into(),
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
            "`{name}` is refused (LLP 1017.003: lists have only `map`, `filter` and `join`); compute it in the data source and hand the view the result, as the crypto port keeps `lo` and `hi` beside its series"
        ),
        // @ref LLP 1088 D7.4 — the web's string and list spellings an agent
        // reaches for first, each with what to write today.
        "concat" | "slice" | "push" => format!(
            "`{name}` on a list is not in Contract yet: building a list in a view waits on LLP 1088 §9's follow-up; build it in the data module and hand the view the result"
        ),
        "split" => "`split` is not in Contract: a view takes its lists from the data module, so split the text there and hand the view the list".into(),
        "indexOf" => "`indexOf` is not in Contract: test text with `includes(s, t)`, `startsWith(s, t)` or `endsWith(s, t)`; a position in a list is the data module's to compute".into(),
        "substring" | "substr" => format!(
            "`{name}` is not in Contract: write `slice(s, start, end)`, the web's `String.prototype.slice` (LLP 1088 D2)"
        ),
        "padStart" | "padEnd" => format!(
            "`{name}` is not in Contract (LLP 1088 D2 defers it): pad in the data module, or align with CSS (`text-align`, a fixed `width`)"
        ),
        "toUpperCase" => "display casing is CSS: `text-transform=\"uppercase\"`; Contract has no `toUpperCase` (LLP 1088 D2)".into(),
        "Number" | "parseInt" | "parseFloat" => format!(
            "Contract does not parse numbers from text (`{name}`, LLP 1088 D2 defers it): have the data module answer a number, or keep the number in state and print it with `toString`"
        ),
        "len" => "write `length(x)`: Contract spells the web's `.length`, of text or of a list, as a roster function".into(),
        "min" | "max" => format!(
            "write `{name}(a, b)` for two numbers; `Math.{name}(...xs)` over a list is refused (LLP 1017.003): compute it in the data source, as the crypto port keeps `lo` and `hi` beside its series"
        ),
        // The roster's substring search under the web's name since
        // 2026-09-28; a hint, not a second spelling: `contains` does not compile.
        "contains" => "write `includes(s, t)`: the roster's substring search wears the web's name, `String.prototype.includes` (LLP 1006 §Expressions, renamed from `contains` 2026-09-28); `startsWith(s, t)` and `endsWith(s, t)` are the web's too".into(),
        "toFixed" | "toPrecision" => format!(
            "`{name}` is refused (LLP 1017.003); round with `floor(v * 100 + 0.5) / 100` and print it with `toString` (or a template), or, for a count, `formatNumber(n, \"compact\")` prints `1.2K` (LLP 1054.000.003)"
        ),
        _ => return None,
    })
}
