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
        | "sort" | "toSorted" | "slice" | "flatMap" | "flat" | "concat" | "indexOf"
        | "forEach" | "reverse" | "toReversed" => format!(
            "`{name}` is refused (LLP 1017.003: lists have only `map`, `filter` and `join`); compute it in the data source and hand the view the result, as the crypto port keeps `lo` and `hi` beside its series"
        ),
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
