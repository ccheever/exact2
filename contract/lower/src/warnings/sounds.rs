//! `lower-sound-undeclared`: a WAV path, written as a literal or built as a
//! template or a concatenation, that no `sound` line can be. The type pass
//! refuses a literal `playSound` source no `sound` declares
//! (`type-sound-undeclared`); a path written anywhere else (a step's `src`,
//! a `fn` that builds one from a kit's name) reaches the runner, which drops
//! each voice and journals why (LLP 1096 D5), and the app is silent with
//! nothing at build time. The euxy port played its 72 kit samples through
//! `` `assets/audio/${kit}/${slot}.wav` `` with none declared.
//!
//! Only what is certain is named, as in `miswiring.rs` (@ref LLP 1116.010
//! D5, a miswiring the compiler can see is a warning): a literal under
//! `assets/` that no `sound` declares, or a built path whose fixed text names
//! `assets/` and that no declared path completes, each hole taking any text.
//! A built path that one declared path completes is given the benefit of the
//! doubt. An `audio` or `video` source is a media file, not a sound, so it is
//! left alone, as is the binding or `fn` the source names.

use crate::theme::scan::{exprs, stmts};
use crate::LowerError;
use contract_syntax::{BinOp, Expr, Node, Stmt, TemplatePart};
use contract_types::Checked;
use std::collections::BTreeSet;

/// A piece of a built path: fixed text, or a hole any text fills.
#[derive(Debug, PartialEq)]
enum Piece {
    Text(String),
    Hole,
}

/// Every WAV path the file writes or builds that no declared sound can be.
pub(super) fn undeclared(checked: &Checked<'_>, out: &mut Vec<LowerError>) {
    let declared: Vec<&str> = checked
        .file
        .sounds
        .iter()
        .map(|s| s.source.as_str())
        .collect();
    let root = &checked.expanded.root;
    // The view's expressions, and the name each `audio` or `video` source
    // is.
    let (mut view, mut media) = (Vec::new(), BTreeSet::<String>::new());
    let mut stack: Vec<&Node> = root.view.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                let is_media = matches!(tag.as_str(), "audio" | "video");
                let src = |a: &&contract_syntax::Attr| is_media && a.name == "src";
                if is_media {
                    let sources = positional
                        .iter()
                        .chain(attrs.iter().filter(src).map(|a| &a.value));
                    // Only the name the source is: `audio song(kit)` names `song`, not `kit`.
                    for e in sources {
                        if let Expr::Ident(name, _) | Expr::Call(name, _, _) = e {
                            media.insert(name.clone());
                        }
                    }
                } else {
                    view.extend(positional);
                }
                view.extend(attrs.iter().filter(|a| !src(a)).map(|a| &a.value));
                stack.extend(children);
            }
            Node::Use { args, children, .. } => {
                view.extend(args.iter().map(|a| &a.value));
                stack.extend(children);
            }
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => {
                view.push(cond);
                stack.extend(then.iter().chain(otherwise));
            }
            Node::Each {
                list, key, body, ..
            } => {
                view.extend([list, key]);
                stack.extend(body);
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                view.push(subject);
                stack.extend(some.1.iter().chain(none));
            }
            Node::Children { .. } => {}
        }
    }
    let mut seen = BTreeSet::new();
    // The operands of a concatenation already read whole, which are not read
    // again as a shorter path.
    let mut joined = BTreeSet::new();
    let mut check = |e: &Expr| {
        exprs(e, &mut |e| {
            if let Some(warning) = path(e, &declared, &mut joined) {
                let at = warning.span;
                if seen.insert((at.source_id, at.line, at.col, at.end_col)) {
                    out.push(warning);
                }
            }
        })
    };
    for f in checked
        .file
        .fns
        .iter()
        .filter(|f| !media.contains(f.name.as_str()))
    {
        check(&f.body);
    }
    let bindings = root
        .states
        .iter()
        .chain(&root.derives)
        .chain(&root.provides);
    for b in bindings.filter(|b| !media.contains(b.name.as_str())) {
        check(&b.expr);
    }
    for r in &root.resources {
        r.args.iter().for_each(&mut check);
        if let Some(p) = &r.placeholder {
            p.args.iter().for_each(&mut check);
        }
    }
    for s in &root.sequences {
        check(&s.tracks);
        s.options.iter().for_each(|(_, e, _)| check(e));
    }
    // A value assigned to a binding a media source names is a media file too.
    let mut media_values = BTreeSet::new();
    for a in &root.actions {
        assigned(&a.body, &media, &mut media_values);
    }
    for a in &root.actions {
        stmts(&a.body, &mut |e| {
            if !media_values.contains(&(e as *const Expr)) {
                check(e);
            }
        });
    }
    view.into_iter().for_each(check);
}

/// Each value `body` assigns to one of `names`, through `if`, `match` and a
/// call's `then`.
fn assigned(body: &[Stmt], names: &BTreeSet<String>, out: &mut BTreeSet<*const Expr>) {
    for st in body {
        match st {
            Stmt::Assign { target, expr, .. } if names.contains(target) => {
                out.insert(expr as *const Expr);
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                assigned(then, names, out);
                assigned(otherwise, names, out);
            }
            Stmt::Match { some, none, .. } => {
                assigned(&some.1, names, out);
                assigned(none, names, out);
            }
            Stmt::Call { body, .. } => assigned(body, names, out),
            _ => {}
        }
    }
}

/// The warning for `e` when it is a WAV path no declared sound can be.
fn path(e: &Expr, declared: &[&str], joined: &mut BTreeSet<*const Expr>) -> Option<LowerError> {
    if joined.contains(&(e as *const Expr)) {
        return None;
    }
    let span = match e {
        Expr::Str(text, span) => {
            if !text.starts_with("assets/") || !wav(text) || declared.contains(&text.as_str()) {
                return None;
            }
            return Some(LowerError {
                id: "lower-sound-undeclared",
                message: format!(
                    "\"{text}\" is not a declared sound, so each step or `playSound` that plays it is dropped at run time and nothing sounds (the journal says `\"{text}\" is not a declared sound`); declare it at the top level: `sound \"{text}\"`"
                ),
                span: *span,
            });
        }
        Expr::Template(_, span) | Expr::Binary(BinOp::Add, _, _, span) => *span,
        _ => return None,
    };
    let mut pieces = Vec::new();
    built(e, &mut pieces, joined);
    // A sound is under `assets/` (LLP 1096 D1): a built path whose fixed
    // text says nothing of it may name a file of another kind.
    let under_assets = pieces
        .iter()
        .any(|p| matches!(p, Piece::Text(t) if t.contains("assets/")));
    match pieces.last() {
        Some(Piece::Text(last)) if wav(last) && under_assets => {}
        _ => return None,
    }
    if declared.iter().any(|d| fits(&pieces, d)) {
        return None;
    }
    let shown = |hole: &str| -> String {
        pieces
            .iter()
            .map(|p| match p {
                Piece::Text(t) => t.as_str(),
                Piece::Hole => hole,
            })
            .collect()
    };
    let fit = if declared.is_empty() {
        "the app declares no `sound`".to_string()
    } else {
        format!("no `sound` line fits `{}`", shown("*"))
    };
    Some(LowerError {
        id: "lower-sound-undeclared",
        message: format!(
            "`{}` builds a sound's path, and {fit}: each step or `playSound` that plays one is dropped at run time and nothing sounds (the journal says `… is not a declared sound`); declare every file it can name at the top level, a `sound \"…\"` line each",
            shown("${…}")
        ),
        span,
    })
}

/// The pieces of a template or a concatenation, left to right, adjacent
/// texts and holes merged: a literal or a template's text is fixed, any
/// other operand or interpolation a hole. Each operand of a `+` is marked
/// `joined`, so it is not read again alone.
fn built(e: &Expr, out: &mut Vec<Piece>, joined: &mut BTreeSet<*const Expr>) {
    match e {
        Expr::Binary(BinOp::Add, a, b, _) => {
            for operand in [a, b] {
                joined.insert(&**operand as *const Expr);
                built(operand, out, joined);
            }
        }
        Expr::Template(parts, _) => {
            for part in parts {
                push(
                    out,
                    match part {
                        TemplatePart::Text(t) => Piece::Text(t.clone()),
                        TemplatePart::Expr(_) => Piece::Hole,
                    },
                );
            }
        }
        Expr::Str(text, _) => push(out, Piece::Text(text.clone())),
        _ => push(out, Piece::Hole),
    }
}

/// `piece` after `out`'s last, merged with it when both are text or both
/// holes.
fn push(out: &mut Vec<Piece>, piece: Piece) {
    match (out.last_mut(), piece) {
        (Some(Piece::Text(prior)), Piece::Text(text)) => prior.push_str(&text),
        (Some(Piece::Hole), Piece::Hole) => {}
        (_, piece) => out.push(piece),
    }
}

fn wav(text: &str) -> bool {
    text.len() >= 4
        && text.is_char_boundary(text.len() - 4)
        && text[text.len() - 4..].eq_ignore_ascii_case(".wav")
}

/// Whether some filling of the holes makes `path`: each fixed piece in
/// order, the first at its start when no hole leads, the last at its end.
/// The pieces alternate, as [`push`] keeps them.
fn fits(pieces: &[Piece], path: &str) -> bool {
    let mut rest = path;
    let mut open = false;
    let last = pieces.len() - 1;
    for (i, piece) in pieces.iter().enumerate() {
        let Piece::Text(text) = piece else {
            open = true;
            continue;
        };
        let text = text.as_str();
        if i == last {
            return if open {
                rest.ends_with(text)
            } else {
                rest == text
            };
        }
        match (open, rest.find(text)) {
            (false, _) if rest.starts_with(text) => rest = &rest[text.len()..],
            (true, Some(at)) => rest = &rest[at + text.len()..],
            _ => return false,
        }
        open = false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(spec: &str) -> Vec<Piece> {
        let mut out = Vec::new();
        for (i, text) in spec.split('*').enumerate() {
            if i > 0 {
                out.push(Piece::Hole);
            }
            if !text.is_empty() {
                out.push(Piece::Text(text.to_string()));
            }
        }
        out
    }

    #[test]
    fn a_template_fits_a_path_its_holes_can_fill() {
        let kit = pieces("assets/audio/*/*.wav");
        assert!(fits(&kit, "assets/audio/808/kick.wav"));
        assert!(fits(&kit, "assets/audio/a/b/c.wav"));
        assert!(!fits(&kit, "assets/kick.wav"));
        assert!(!fits(&kit, "assets/audio/808/kick.mp3"));
        assert!(fits(&pieces("*/kick.wav"), "assets/kits/808/kick.wav"));
        assert!(!fits(&pieces("assets/*.wav"), "sounds/a.wav"));
        // The fixed text at the end is not taken twice: `x.wav` cannot be `.wav` and `x.wav` both.
        assert!(!fits(&pieces("assets/x.wav*.wav"), "assets/x.wav"));
        assert!(fits(&pieces("assets/x.wav*.wav"), "assets/x.wavy.wav"));
    }
}
