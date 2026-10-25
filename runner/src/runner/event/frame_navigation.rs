//! What an iframe's guest shows, seen from outside (@ref LLP 1020.002 D2):
//! the `FrameNavigation` record of `navigate`, `currententrychange`,
//! `navigatesuccess` and `navigateerror` on an `iframe`. A host sends it as
//! ABI kind 58, `<event>\n<JSON object>`; a field the host leaves out is
//! its type's empty value. The same kind carries a guest's `newwindow` and
//! `download` (@ref LLP 1020.002 D5), with their own records ([`other`]).
use exact_plan::{EventKind, Value};

/// The record, its fields in the compiler's order
/// (`contract/types/src/selection.rs`, `FrameNavigation`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameNavigation {
    pub url: String,
    pub destination: String,
    pub navigation_id: f64,
    pub navigation_type: String,
    pub title: String,
    pub icons: Vec<String>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub loading: bool,
    pub partitioned: bool,
    pub controller: String,
    pub error: String,
    pub message: String,
    pub recovering: bool,
}

impl FrameNavigation {
    /// Kind 58's payload: the event's name (one of the four), a newline,
    /// then the record as a JSON object.
    pub fn parse(payload: &str) -> Option<(EventKind, Self)> {
        let (name, json) = payload.split_once('\n')?;
        let kind = match name {
            "navigate" => EventKind::Navigate,
            "currententrychange" => EventKind::Currententrychange,
            "navigatesuccess" => EventKind::Navigatesuccess,
            "navigateerror" => EventKind::Navigateerror,
            _ => return None,
        };
        let serde_json::Value::Object(o) = serde_json::from_str(json).ok()? else {
            return None;
        };
        let text = |k: &str| {
            o.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let flag = |k: &str| o.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
        let id = o
            .get("navigationId")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let icons = match o.get("icons") {
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        Some((
            kind,
            Self {
                url: text("url"),
                destination: text("destination"),
                navigation_id: if id.is_finite() { id } else { 0.0 },
                navigation_type: text("navigationType"),
                title: text("title"),
                icons,
                can_go_back: flag("canGoBack"),
                can_go_forward: flag("canGoForward"),
                loading: flag("loading"),
                partitioned: flag("partitioned"),
                controller: text("controller"),
                error: text("error"),
                message: text("message"),
                recovering: flag("recovering"),
            },
        ))
    }

    /// The record value an action takes as its optional last parameter.
    pub fn value(&self) -> Value {
        Value::record(vec![
            Value::str(&self.url),
            Value::str(&self.destination),
            Value::Number(self.navigation_id),
            Value::str(&self.navigation_type),
            Value::str(&self.title),
            Value::list(self.icons.iter().map(|i| Value::str(i)).collect()),
            Value::Bool(self.can_go_back),
            Value::Bool(self.can_go_forward),
            Value::Bool(self.loading),
            Value::Bool(self.partitioned),
            Value::str(&self.controller),
            Value::str(&self.error),
            Value::str(&self.message),
            Value::Bool(self.recovering),
        ])
    }
}

/// Kind 58's `newwindow` (a `FrameNewWindow`: `url`, `disposition`),
/// `download` (a `FrameDownload`: `url`, `suggestedFilename`, `state`,
/// `path`) or `capture` (a `FrameCapture`: `path`, `width`, `height`,
/// `error`, `message`, @ref LLP 1020.002 D7) or `userpromptopened` (a
/// `FrameUserPrompt`: `type`, `message`, `defaultValue`, D9), the record in
/// the compiler's order (`contract/types/src/selection.rs`).
pub fn other(payload: &str) -> Option<(EventKind, Value)> {
    let (name, json) = payload.split_once('\n')?;
    let (kind, fields): (_, &[&str]) = match name {
        "newwindow" => (EventKind::Newwindow, &["url", "disposition"]),
        "download" => (
            EventKind::Download,
            &["url", "suggestedFilename", "state", "path"],
        ),
        "capture" => (
            EventKind::Capture,
            &["path", "width", "height", "error", "message"],
        ),
        "userpromptopened" => (
            EventKind::Userpromptopened,
            &["type", "message", "defaultValue"],
        ),
        _ => return None,
    };
    let serde_json::Value::Object(o) = serde_json::from_str(json).ok()? else {
        return None;
    };
    let field = |k: &str| match k {
        "width" | "height" => Value::Number(o.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0)),
        _ => Value::str(o.get(k).and_then(|v| v.as_str()).unwrap_or_default()),
    };
    Some((
        kind,
        Value::record(fields.iter().map(|k| field(k)).collect()),
    ))
}

/// The event's name, as its handler attribute spells it.
pub(super) fn name(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Currententrychange => "currententrychange",
        EventKind::Navigatesuccess => "navigatesuccess",
        EventKind::Navigateerror => "navigateerror",
        EventKind::Newwindow => "newwindow",
        EventKind::Download => "download",
        EventKind::Capture => "capture",
        EventKind::Userpromptopened => "userpromptopened",
        _ => "navigate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_record_decodes_in_the_compilers_order() {
        let (kind, n) = FrameNavigation::parse(
            "navigatesuccess\n{\"url\":\"https://a.test/\",\"navigationId\":3,\"navigationType\":\"push\",\"icons\":[\"https://a.test/i.png\"],\"canGoBack\":true,\"partitioned\":true,\"controller\":\"person\"}",
        )
        .unwrap();
        assert_eq!(kind, EventKind::Navigatesuccess);
        assert_eq!(n.url, "https://a.test/");
        assert_eq!(n.navigation_id, 3.0);
        assert!(n.can_go_back && !n.can_go_forward && n.partitioned && !n.loading);
        assert_eq!(n.error, "");
        let Value::Record(fields) = n.value() else {
            panic!("a record")
        };
        assert_eq!(fields.len(), 14);
        assert!(FrameNavigation::parse("load\n{}").is_none());
        assert!(FrameNavigation::parse("navigate\n[1]").is_none());
    }

    #[test]
    fn a_window_request_a_download_and_a_capture_decode_in_the_compilers_order() {
        let (kind, v) =
            other("newwindow\n{\"url\":\"https://a.test/\",\"disposition\":\"foreground-tab\"}")
                .unwrap();
        assert_eq!(kind, EventKind::Newwindow);
        assert_eq!(
            v,
            Value::record(vec![
                Value::str("https://a.test/"),
                Value::str("foreground-tab")
            ])
        );
        let (kind, v) = other("download\n{\"url\":\"https://a.test/f\",\"suggestedFilename\":\"f.txt\",\"state\":\"progressing\"}").unwrap();
        assert_eq!(kind, EventKind::Download);
        let Value::Record(fields) = v else {
            panic!("a record")
        };
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[3], Value::str(""));
        let (kind, v) =
            other("capture\n{\"path\":\"app:/tmp/a.png\",\"width\":200,\"height\":100}").unwrap();
        assert_eq!(kind, EventKind::Capture);
        assert_eq!(
            v,
            Value::record(vec![
                Value::str("app:/tmp/a.png"),
                Value::Number(200.0),
                Value::Number(100.0),
                Value::str(""),
                Value::str("")
            ])
        );
        let (kind, v) =
            other("userpromptopened\n{\"type\":\"confirm\",\"message\":\"Sure?\"}").unwrap();
        assert_eq!(kind, EventKind::Userpromptopened);
        assert_eq!(
            v,
            Value::record(vec![
                Value::str("confirm"),
                Value::str("Sure?"),
                Value::str("")
            ])
        );
        assert!(other("navigate\n{}").is_none());
        assert!(FrameNavigation::parse("download\n{}").is_none());
    }
}
