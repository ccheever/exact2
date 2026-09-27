//! The reader's colours, as tokens.
//!
//! Exact1 had Facet: a theme *capability* with signal semantics, a token
//! store shared with React, `$`-prefixed token references resolved by a
//! registered resolver. None of that exists here and none of it can — a
//! second value graph is refused (`rules/DEFERRED.md` §Motion), Contract's
//! `use` admits no packages (`contract/corpus/rejects.txt` lists `use theme
//! from "@exact/facet-contract"` among the refusals), and nothing runs
//! JavaScript above the data seam.
//!
//! What survives is the idea, in the language's own terms. Contract has
//! `provide`/`inject` — which exact1's own theme file calls "Contract's
//! replacement for React context" — and it has a data seam. So the tokens
//! are data: one record, answered here for an appearance, `provide`d once at
//! the root and `inject`ed by the components that draw. No runtime, no
//! signals, no lookup: the compiler fills each `inject` from the enclosing
//! `provide` (LLP 1017 P4a), and a missing provider is a compile error.
//!
//! Both readers share this palette, which is the other half of the point:
//! two apps that cannot share a `.contract` file can still share their
//! colours, because colours are data.
//!
//! Since LLP 1034 the tokens are *pairs*: each is CSS's `light-dark(a, b)`
//! and the host resolves it against the appearance, so this record is one
//! constant and the app never learns which appearance it is drawing in. The
//! one exception is `icons` — a PNG has no `currentColor` to follow, so the
//! artwork is chosen by the app, and that suffix is the last thing the
//! appearance is needed for (LLP 1034 D3).
//!
//! @ref LLP 1033 D6, LLP 1034 D1/D3

use exact_plan::Value;

/// The reader's tokens. Every field is a CSS `light-dark()` pair: the host
/// resolves it, so there is one record and no appearance in the app.
/// The field order is `shape Theme`'s.
pub struct Tokens;

/// One token: what it is called, and its light and dark colours.
pub struct Token(pub &'static str, pub &'static str);

/// The palette, light half and dark half, in `shape Theme`'s order.
///
/// The dark half is warm rather than blue-black, so the same documents read
/// as the same documents, and its text sits below white-on-black contrast,
/// which is what makes a long document comfortable at night.
pub const TOKENS: [Token; 20] = [
    Token("#ffffff", "#17181b"), // page — behind the document
    Token("#f6f5f1", "#1e1f23"), // chrome — header, index, outline
    Token("#e2e1d9", "#2f3138"), // edge — the hairlines between them
    Token("#22221b", "#d4d1c9"), // ink — running prose
    Token("#16160f", "#f1eee7"), // strongInk — headings
    Token("#8a8a80", "#8b8c92"), // softInk — metadata, markers, captions
    Token("#b0b0a2", "#63656d"), // faintInk — the quietest text there is
    Token("#4a5a3a", "#a2b585"), // accent — the brand mark, a number
    Token("#8a3020", "#e0a189"), // code — a code span's text
    Token("#f4f4f0", "#23252b"), // codeBg — behind a code block
    Token("#ffffff", "#212329"), // field — inside a text field
    Token("#d5d4cc", "#3a3d45"), // fieldEdge — its border
    Token("#e5e8dc", "#2f3a28"), // pill — a chosen row
    Token("#2c3520", "#d6e4c4"), // pillInk — text on a pill
    Token("#e8e7e0", "#2a2c32"), // control — a button that is not a pill
    Token("#d3d3c8", "#3b3e46"), // quoteEdge — the rule on a quote
    Token("#55554a", "#a6a39b"), // quoteInk — a quote's text
    Token("#fdecea", "#3a1e1b"), // warnBg — behind a refusal
    Token("#f5c6c0", "#5d302a"), // warnEdge — its border
    Token("#8a1c12", "#f0a89c"), // warnInk — its text
];

/// `shape Theme` — one constant record of `light-dark()` pairs.
pub fn value() -> Value {
    Value::record(
        TOKENS
            .iter()
            .map(|t| Value::str(&format!("light-dark({}, {})", t.0, t.1)))
            .collect(),
    )
}

/// The artwork suffix an appearance names. The only thing left that needs to
/// know the appearance, because a PNG cannot follow `currentColor`
/// (LLP 1034 D3); the readers derive it in Contract and never ask here.
pub fn icons(appearance: &str) -> &'static str {
    if appearance == "dark" {
        "-dark"
    } else {
        ""
    }
}
