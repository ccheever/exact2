//! The view-model's shapes, in the contract's field order: `read` turns the
//! model's JSON into the runner's positional records. Every shape here has a
//! twin `shape` in `app.contract`; the two are kept in step by hand.

use exact_plan::Value;

pub enum Shape {
    Str,
    Num,
    Bool,
    List(&'static Shape),
    Record(&'static [(&'static str, Shape)]),
}

use Shape::{Bool, List, Num, Record, Str};

pub const UI: Shape = Record(&[("version", Num), ("io", Num)]);
pub const VERSION: Shape = Record(&[("version", Num)]);

pub const THEME: Shape = Record(&[
    ("name", Str),
    ("dark", Bool),
    ("bg", Str),
    ("surface", Str),
    ("elevated", Str),
    ("element", Str),
    ("border", Str),
    ("text", Str),
    ("muted", Str),
    ("placeholder", Str),
    ("accent", Str),
    ("warn", Str),
    ("danger", Str),
    ("good", Str),
    ("selected", Str),
    ("hover", Str),
    ("termBg", Str),
    ("termFg", Str),
    ("cursor", Str),
    ("statusBar", Str),
    // Opaque blends the rail uses (ui.rs:836-837) and the tints with alpha.
    ("selectedBg", Str),
    ("hoverBg", Str),
    ("accent09", Str),
    ("bg55", Str),
    ("bg72", Str),
    ("bg78", Str),
    ("elevated94", Str),
    ("warn08", Str),
    ("warn35", Str),
    ("accent12", Str),
    ("accent15", Str),
    ("accent16", Str),
    ("accent18", Str),
    ("accent35", Str),
    ("accent55", Str),
    ("warn60", Str),
    ("warn70", Str),
    ("muted70", Str),
    ("border72", Str),
    ("backdrop", Str),
]);

pub const RAIL_PAGE: Shape = Record(&[
    ("id", Str),
    ("title", Str),
    ("icon", Str),
    ("count", Str),
    ("selected", Bool),
]);

pub const RAIL_TAB: Shape = Record(&[
    ("key", Str),
    ("slug", Str),
    ("title", Str),
    ("machine", Str),
    ("badge", Str),
    ("badgeTint", Str),
    ("badgeBg", Str),
    ("badgeFaded", Bool),
    ("depth", Num),
    ("folder", Bool),
    ("collapsed", Bool),
    ("hasChildren", Bool),
    ("active", Bool),
    ("ancestor", Bool),
    ("status", Str),
    ("statusMarkdown", Bool),
    ("working", Bool),
    ("workingGlyph", Str),
    ("workingHead", Str),
    ("workingHeadColor", Str),
    ("workingRest", Str),
    ("imessage", Bool),
    ("number", Str),
    ("dropBefore", Bool),
    ("dropAfter", Bool),
    ("dropInto", Bool),
]);

pub const RAIL: Shape = Record(&[
    ("width", Num),
    ("nav", Bool),
    ("pages", List(&RAIL_PAGE)),
    ("tabs", List(&RAIL_TAB)),
    ("hint", Str),
    ("updateLabel", Str),
    ("updateTip", Str),
    ("selectedTab", Str),
    ("resizing", Bool),
]);

pub const ROW_ACTION: Shape = Record(&[("id", Str), ("icon", Str), ("label", Str), ("hint", Str)]);

pub const METER: Shape = Record(&[
    ("label", Str),
    ("percent", Num),
    ("text", Str),
    ("reset", Str),
    ("color", Str),
]);

pub const ROW: Shape = Record(&[
    ("id", Str),
    ("header", Str),
    ("badge", Str),
    ("badgeColor", Str),
    ("badgeBg", Str),
    ("title", Str),
    ("imessage", Bool),
    ("prs", List(&Str)),
    ("lines", List(&Str)),
    ("firstLine", Str),
    ("markdownLine", Num),
    ("attention", Bool),
    ("working", Bool),
    ("workingGlyph", Str),
    ("workingHead", Str),
    ("workingHeadColor", Str),
    ("workingRest", Str),
    ("usage", List(&METER)),
    ("usageNote", Str),
    ("archived", Bool),
    ("selected", Bool),
    ("actions", List(&ROW_ACTION)),
]);

pub const MANAGER: Shape = Record(&[
    ("page", Str),
    ("title", Str),
    ("filterSummary", Str),
    ("query", Str),
    ("searching", Bool),
    ("primary", Str),
    ("primaryHint", Str),
    ("rows", List(&ROW)),
    ("empty", Str),
    ("selected", Num),
    ("focused", Bool),
    ("viewOpen", Bool),
]);

pub const MENU_ITEM: Shape = Record(&[
    ("id", Str),
    ("label", Str),
    ("icon", Str),
    ("hint", Str),
    ("checked", Bool),
    ("checkable", Bool),
    ("selected", Bool),
    ("disabled", Bool),
]);

pub const POPUP: Shape = Record(&[
    ("kind", Str),
    ("anchor", Str),
    ("x", Num),
    ("y", Num),
    ("right", Num),
    ("title", Str),
    ("items", List(&MENU_ITEM)),
    ("empty", Str),
]);

pub const PICKER_ROW: Shape = Record(&[
    ("id", Str),
    ("chip", Str),
    ("label", Str),
    ("detail", Str),
    ("hint", Str),
    ("second", Str),
    ("selected", Bool),
    ("swatch", Str),
    ("swatchBorder", Str),
    ("accent", Bool),
]);

pub const FIELD: Shape = Record(&[
    ("id", Str),
    ("label", Str),
    ("value", Str),
    ("placeholder", Str),
    ("kind", Str),
    ("focused", Bool),
    ("options", List(&Str)),
    ("hint", Str),
    ("multiline", Bool),
    ("suggestions", List(&PICKER_ROW)),
]);

pub const BUTTON: Shape = Record(&[
    ("id", Str),
    ("label", Str),
    ("hint", Str),
    ("primary", Bool),
    ("disabled", Bool),
]);

pub const OVERLAY: Shape = Record(&[
    ("kind", Str),
    ("width", Num),
    ("top", Bool),
    ("title", Str),
    ("subtitle", Str),
    ("pill", Str),
    ("pillColor", Str),
    ("glyph", Str),
    ("placeholder", Str),
    ("query", Str),
    ("status", Str),
    ("statusError", Bool),
    ("rows", List(&PICKER_ROW)),
    ("index", Num),
    ("footer", Str),
    ("body", Str),
    ("bodyMarkdown", Bool),
    ("blocks", List(&MARKDOWN_BLOCK)),
    ("fields", List(&FIELD)),
    ("buttons", List(&BUTTON)),
    ("hint", Str),
    ("focusId", Str),
]);

pub const MARKDOWN_RUN: Shape = Record(&[
    ("id", Str),
    ("text", Str),
    ("bold", Bool),
    ("italic", Bool),
    ("code", Bool),
    ("strike", Bool),
    ("url", Str),
    ("subdued", Bool),
    ("math", Str),
]);

pub const MARKDOWN_CELL: Shape = Record(&[
    ("id", Str),
    ("runs", List(&MARKDOWN_RUN)),
    ("flow", Bool),
    ("align", Str),
]);

pub const MARKDOWN_TABLE_ROW: Shape = Record(&[
    ("id", Str),
    ("header", Bool),
    ("cells", List(&MARKDOWN_CELL)),
]);

pub const MARKDOWN_BLOCK: Shape = Record(&[
    ("id", Str),
    ("kind", Str),
    ("level", Num),
    ("depth", Num),
    ("marker", Str),
    ("quote", Num),
    ("language", Str),
    ("text", Str),
    ("subdued", Bool),
    ("flow", Bool),
    ("runs", List(&MARKDOWN_RUN)),
    ("rows", List(&MARKDOWN_TABLE_ROW)),
    ("align", List(&Str)),
]);

pub const INDICATOR: Shape = Record(&[
    ("glyph", Str),
    ("head", Str),
    ("headColor", Str),
    ("rest", Str),
    ("restColor", Str),
]);

pub const TRANSCRIPT_ENTRY: Shape = Record(&[
    ("id", Str),
    ("kind", Str),
    ("subdued", Bool),
    ("first", Bool),
    ("blocks", List(&MARKDOWN_BLOCK)),
    ("summary", Str),
    ("live", Str),
    ("liveIndicator", INDICATOR),
    ("collapsed", Bool),
    ("toggleId", Str),
]);

pub const TRANSCRIPT: Shape = Record(&[
    ("visible", Bool),
    ("entries", List(&TRANSCRIPT_ENTRY)),
    ("empty", Str),
    ("error", Str),
    ("working", Str),
    ("workingIndicator", INDICATOR),
    ("provider", Str),
    ("draft", Str),
    ("rows", Num),
    ("placeholder", Str),
    ("editing", Bool),
    ("sendLabel", Str),
    ("canSend", Bool),
    ("scrollTop", Num),
]);

pub const TAB_VIEW: Shape = Record(&[
    ("key", Str),
    ("title", Str),
    ("argv", List(&Str)),
    ("argvJson", Str),
    ("machine", Str),
    ("cwd", Str),
    ("readOnly", Bool),
    ("strip", Str),
    ("stripLabel", Str),
    ("stripDetail", Str),
    ("stripColor", Str),
    ("stripRetry", Bool),
    ("exitBanner", Str),
    ("limited", Bool),
    ("limitedStatus", Str),
    ("limitedProvider", Str),
    ("overlay", Str),
    ("overlayTitle", Str),
    ("overlayDetail", Str),
    ("overlayButtons", List(&BUTTON)),
    ("overlayNote", Str),
    ("overlayReason", Str),
    ("exitButtons", List(&BUTTON)),
    ("panelOpen", Bool),
    ("panelArgv", List(&Str)),
    ("panelCwd", Str),
    ("panelHeight", Num),
    ("panelFocused", Bool),
    ("panelKey", Str),
    ("panelArgvJson", Str),
    ("panelLabel", Str),
    ("transcript", Bool),
    ("conversation", TRANSCRIPT),
    ("dropTarget", Bool),
]);

pub const TOAST: Shape = Record(&[("text", Str), ("error", Bool), ("dismissible", Bool)]);

pub const WHATS_NEW_ROW: Shape = Record(&[
    ("kind", Str),
    ("id", Str),
    ("date", Str),
    ("title", Str),
    ("commit", Str),
    ("url", Str),
]);

pub const WHATS_NEW: Shape = Record(&[
    ("visible", Bool),
    ("title", Str),
    ("build", Str),
    ("subtitle", Str),
    ("rows", List(&WHATS_NEW_ROW)),
    ("empty", Str),
    ("footer", Str),
    ("closeLabel", Str),
    ("scrollSeq", Num),
    ("scrollBy", Num),
    ("scrollTo", Str),
]);

pub const QR_CELL: Shape = Record(&[("id", Str), ("dark", Bool)]);
pub const QR_ROW: Shape = Record(&[("id", Str), ("cells", List(&QR_CELL))]);

pub const IMESSAGE_PAIR: Shape = Record(&[
    ("visible", Bool),
    ("title", Str),
    ("state", Str),
    ("loading", Str),
    ("error", Str),
    ("instructions", Str),
    ("qr", List(&QR_ROW)),
    ("code", Str),
    ("sendTo", Str),
    ("sendFrom", Str),
    ("url", Str),
    ("retry", Bool),
]);

pub const SECRET_LIFETIME: Shape = Record(&[
    ("id", Str),
    ("label", Str),
    ("seconds", Num),
    ("selected", Bool),
]);

pub const SECRET_CARD: Shape = Record(&[
    ("id", Str),
    ("requestId", Str),
    ("name", Str),
    ("reason", Str),
    ("status", Str),
    ("statusColor", Str),
    ("editing", Bool),
    ("entry", Bool),
    ("actions", Bool),
    ("revoke", Bool),
    ("openId", Str),
    ("dismissId", Str),
    ("revokeId", Str),
    ("provideId", Str),
    ("cancelId", Str),
]);

pub const SECRET_ALERT: Shape = Record(&[
    ("visible", Bool),
    ("title", Str),
    ("name", Str),
    ("reason", Str),
    ("openLabel", Str),
]);

pub const SECRETS: Shape = Record(&[
    ("visible", Bool),
    ("title", Str),
    ("pending", Str),
    ("demo", Str),
    ("error", Str),
    ("empty", Str),
    ("cards", List(&SECRET_CARD)),
    ("masked", Str),
    ("placeholder", Str),
    ("focused", Bool),
    ("provideLabel", Str),
    ("lifetimes", List(&SECRET_LIFETIME)),
    ("busy", Bool),
    ("reset", Str),
    ("alert", SECRET_ALERT),
]);

pub const PHONE_CHIP: Shape = Record(&[("id", Str), ("label", Str), ("selected", Bool)]);

pub const PHONE_PAIR: Shape = Record(&[
    ("visible", Bool),
    ("chips", List(&PHONE_CHIP)),
    ("toggle", Str),
    ("toggleLabel", Str),
    ("toggleOn", Bool),
    ("error", Str),
    ("help", Str),
    ("qr", List(&QR_ROW)),
    ("hint", Str),
    ("address", Str),
    ("note", Str),
]);

pub const VIEW: Shape = Record(&[
    ("theme", THEME),
    ("windowTitle", Str),
    ("loaded", Bool),
    ("rail", RAIL),
    ("manager", MANAGER),
    ("onTab", Bool),
    ("tab", TAB_VIEW),
    ("overlay", OVERLAY),
    ("popup", POPUP),
    ("toast", TOAST),
    ("focusId", Str),
    ("scrollTo", Str),
    ("uiFontSize", Num),
    ("termFontSize", Num),
    ("region", Str),
    ("whatsNew", WHATS_NEW),
    ("pairing", IMESSAGE_PAIR),
    ("phone", PHONE_PAIR),
    ("secrets", SECRETS),
]);

/// The value of `json` in `shape`, missing fields as their zero.
pub fn read(shape: &Shape, json: &serde_json::Value) -> Value {
    match shape {
        Str => match json {
            serde_json::Value::String(s) => Value::str(s),
            serde_json::Value::Number(n) => Value::str(&n.to_string()),
            serde_json::Value::Bool(b) => Value::str(if *b { "true" } else { "false" }),
            _ => Value::str(""),
        },
        Num => Value::Number(json.as_f64().unwrap_or(0.0)),
        Bool => Value::Bool(json.as_bool().unwrap_or(false)),
        List(inner) => Value::list(
            json.as_array()
                .map(|items| items.iter().map(|v| read(inner, v)).collect())
                .unwrap_or_default(),
        ),
        Record(fields) => Value::record(
            fields
                .iter()
                .map(|(name, s)| read(s, json.get(*name).unwrap_or(&serde_json::Value::Null)))
                .collect(),
        ),
    }
}
