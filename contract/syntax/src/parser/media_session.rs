//! The media session's two test steps (LLP 1098 D10): `tap "audio"
//! mediasession "seekto" 600`, the action the platform's handler would call,
//! and `expect mediasession`, a read of `state.mediaSession`.

use super::*;

/// What `tap … mediasession` names: the platform's play and pause, then the
/// six actions an element offers by binding them.
const ACTIONS: [&str; 8] = [
    "play",
    "pause",
    "seekbackward",
    "seekforward",
    "seekto",
    "previoustrack",
    "nexttrack",
    "stop",
];

/// The fields `expect mediasession FIELD ==` reads.
const FIELDS: [&str; 6] = [
    "owner",
    "title",
    "artist",
    "album",
    "artwork",
    "playbackState",
];

impl Parser {
    /// The action, by `ACTIONS`' names, as a quoted string.
    fn media_action(&mut self) -> R<(String, Span)> {
        let at = self.peek().span;
        let action = self.str_lit("a media session action, as \"seekforward\"")?;
        if !ACTIONS.contains(&action.as_str()) {
            return Err(SyntaxError {
                id: "syntax-expected-step",
                message: format!(
                    "`{action}` is not a media session action: {}",
                    ACTIONS.join(", ")
                ),
                span: at,
            });
        }
        Ok((action, at))
    }

    /// After `tap "target"`, at `mediasession`: the action, then its
    /// seconds: required for `seekto` (the `seekTime`), optional for a seek
    /// (its `seekOffset`; absent, the element's own), refused otherwise.
    pub(super) fn tap_media_session(&mut self) -> R<TapForm> {
        self.next();
        let (action, at) = self.media_action()?;
        let given = matches!(
            self.peek_kind(),
            TokenKind::Number(_) | TokenKind::Punct("-")
        );
        if given && !action.starts_with("seek") {
            return self.err(
                "syntax-expected-step",
                format!("`mediasession \"{action}\"` takes no seconds"),
            );
        }
        let seconds = if given {
            Some(self.step_number("seconds")?)
        } else {
            None
        };
        match seconds {
            None if action == "seekto" => Err(SyntaxError {
                id: "syntax-expected-step",
                message: "`mediasession \"seekto\"` takes the time to seek to, in seconds: `mediasession \"seekto\" 600`".into(),
                span: at,
            }),
            Some(n) if !(n.is_finite() && n >= 0.0) => Err(SyntaxError {
                id: "syntax-expected-step",
                message: format!("`mediasession \"{action}\"` takes seconds of 0 or more, not {n}"),
                span: at,
            }),
            _ => Ok(TapForm::MediaSession { action, seconds }),
        }
    }

    /// After `expect mediasession`: `has`/`missing "action"`, or a field
    /// `==` a string (`none` for no owner).
    pub(super) fn expect_media_session(&mut self, span: Span) -> R<Step> {
        const USAGE: &str = "`expect mediasession title == \"…\"` (owner, title, artist, album, artwork, playbackState), or `has \"seekto\"` / `missing \"seekto\"`";
        if self.at_ident("has") || self.at_ident("missing") {
            let present = self.at_ident("has");
            self.next();
            let (action, _) = self.media_action()?;
            return Ok(Step::ExpectMediaSessionAction {
                action,
                present,
                span,
            });
        }
        let field = match self.peek_kind().clone() {
            TokenKind::Ident(w) if FIELDS.contains(&w.as_str()) => {
                self.next();
                w
            }
            _ => return self.err("syntax-expected-step", USAGE),
        };
        self.expect_punct("==")?;
        let value = match self.peek_kind().clone() {
            TokenKind::Ident(w) if w == "none" && field == "owner" => {
                self.next();
                None
            }
            _ => Some(self.str_lit("a string")?),
        };
        Ok(Step::ExpectMediaSession { field, value, span })
    }
}
