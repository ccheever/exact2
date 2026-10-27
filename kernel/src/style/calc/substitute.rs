//! CSS Environment Variables 1 §3, "substitute an env()": where a variable
//! is undefined, its fallback's tokens replace the whole `env()`, and the
//! value is then read again in its context, so a fallback is checked only
//! where it is used and groups as its tokens do in their place
//! (`2 * env(…, 10px + 5px)` is `2 * 10px + 5px`). What is substituted is
//! tokens, not text: the result is written token by token, with an empty
//! comment between two that would otherwise read back as other tokens (CSS
//! Syntax 3 §9), so `env(…, 1)0px` stays a number beside a length.

use super::super::{Dimension, Env};
use super::parse::{env_parts, var_parts};
use cssparser::{ParseError, Parser, ParserInput, Token};

/// `text` with every `env()` whose variable `env` does not define and that
/// has a fallback replaced by the fallback's tokens; `None` when one is
/// undefined without a fallback, or the text is malformed.
pub(super) fn substitute(text: &str, env: &Env) -> Option<String> {
    let mut tokens = Vec::new();
    walk_text(text, Some(env), &mut tokens)?;
    let mut out = String::with_capacity(text.len());
    for (i, token) in tokens.iter().enumerate() {
        if i > 0 && !apart(&tokens[i - 1], token) {
            out.push_str("/**/");
        }
        out.push_str(token);
    }
    Some(out)
}

fn walk_text(text: &str, env: Option<&Env>, tokens: &mut Vec<String>) -> Option<()> {
    let mut input = ParserInput::new(text);
    walk(&mut Parser::new(&mut input), env, tokens).ok()
}

/// Each token's text in order, a block's opener and closer each one, a
/// comment none (CSS Syntax 3 §4.3.2: it is no token); every `env()`
/// substituted under `env`, when there is one.
fn walk<'i>(
    p: &mut Parser<'i, '_>,
    env: Option<&Env>,
    tokens: &mut Vec<String>,
) -> Result<(), ParseError<'i, ()>> {
    loop {
        let start = p.position();
        let Ok(token) = p.next_including_whitespace_and_comments() else {
            return Ok(());
        };
        let close = match token.clone() {
            Token::Comment(_) => continue,
            Token::Function(name)
                if env.is_some()
                    && (name.eq_ignore_ascii_case("env") || name.eq_ignore_ascii_case("var")) =>
            {
                let env = env.expect("checked");
                let parts = if name.eq_ignore_ascii_case("env") {
                    env_parts
                } else {
                    var_parts
                };
                let (term, fallback) =
                    p.parse_nested_block(|p| parts(p, 0).map_err(|_| p.new_custom_error(())))?;
                match fallback {
                    // Defined: the variable itself, its fallback unused
                    _ if term.value(env).is_some() => {
                        let mut text = String::new();
                        super::text::env(term, &mut text);
                        walk_text(&text, None, tokens).ok_or(p.new_custom_error(()))?
                    }
                    // The fallback's own `env()` may need substituting too
                    Some(fallback) => {
                        walk_text(&fallback, Some(env), tokens).ok_or(p.new_custom_error(()))?
                    }
                    None => return Err(p.new_custom_error(())),
                }
                continue;
            }
            Token::Function(_) | Token::ParenthesisBlock => ")",
            Token::SquareBracketBlock => "]",
            Token::CurlyBracketBlock => "}",
            _ => {
                tokens.push(p.slice_from(start).to_string());
                continue;
            }
        };
        tokens.push(p.slice_from(start).to_string());
        p.parse_nested_block(|p| walk(p, env, tokens))?;
        tokens.push(close.to_string());
    }
}

/// Whether `a` then `b`, written together, still read as `a` then `b`: the
/// first token ends where `a` does, or both are white space, which reads
/// alike as one. The tokenizer keeps no state between tokens, so `b` then
/// reads as itself.
fn apart(a: &str, b: &str) -> bool {
    let white = |t: &str| {
        t.bytes()
            .all(|c| matches!(c, b'\t' | b'\n' | 0x0c | b'\r' | b' '))
    };
    if white(a) && white(b) {
        return true;
    }
    let joined = format!("{a}{b}");
    let mut input = ParserInput::new(&joined);
    let mut p = Parser::new(&mut input);
    p.next_including_whitespace_and_comments().is_ok() && p.position().byte_index() == a.len()
}

/// The `f64` points a substituted length is under `env`, against `basis`.
pub(super) fn value(d: Dimension, env: &Env, basis: Option<f64>) -> Option<f64> {
    Some(match d.resolve(env) {
        Dimension::Points(v) => f64::from(v),
        Dimension::Percent(p) => basis? * f64::from(p) / 100.0,
        Dimension::Calc(p, x) => basis? * f64::from(p) / 100.0 + f64::from(x),
        Dimension::Compare(c) => c.wide(env, basis)?,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A separator goes only where two tokens would read back as others
    #[test]
    fn tokens_written_together_stay_apart() {
        crate::link_segments();
        let env = Env::new(0.0, 0.0, 0.0, 0.0);
        let segment = |fallback: &str| format!("env(viewport-segment-width 1 0, {fallback})");
        for (text, substituted) in [
            (
                format!("calc(env(safe-area-inset-top, 1px) + {}0px)", segment("1")),
                "calc(env(safe-area-inset-top) + 1/**/0px)",
            ),
            (
                format!("calc(1px +{} )", segment("2px")),
                "calc(1px +/**/2px )",
            ),
            (format!("calc(1px /{})", segment("*2")), "calc(1px //**/*2)"),
            (format!("{}x", segment("a")), "a/**/x"),
            (segment("/* c */ auto /* d */"), " auto "),
        ] {
            assert_eq!(
                substitute(&text, &env).as_deref(),
                Some(substituted),
                "{text}"
            );
        }
        assert_eq!(
            substitute(
                &format!("env(viewport-segment-width 1 0) {}", segment("1px")),
                &env
            ),
            None
        );
    }
}
