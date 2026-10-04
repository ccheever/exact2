//! Exhaustive source-range traversal for compiler-owned AST transformations.

use crate::{ast::*, Span};

/// Visit every source range, without changing the AST's non-location fields.
/// Destructuring is exhaustive so new syntax must account for its locations.
pub trait VisitSpans {
    /// Apply `visit` to each range, including nested expressions and types.
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span));
}

impl VisitSpans for Span {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        visit(self);
    }
}
impl<T: VisitSpans> VisitSpans for Vec<T> {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        for item in self {
            item.visit_spans(visit);
        }
    }
}
impl<T: VisitSpans> VisitSpans for Option<T> {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        if let Some(item) = self {
            item.visit_spans(visit);
        }
    }
}
impl<T: VisitSpans> VisitSpans for Box<T> {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        (**self).visit_spans(visit);
    }
}
impl<A: VisitSpans, B: VisitSpans> VisitSpans for (A, B) {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        self.0.visit_spans(visit);
        self.1.visit_spans(visit);
    }
}
impl<A: VisitSpans, B: VisitSpans, C: VisitSpans> VisitSpans for (A, B, C) {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        self.0.visit_spans(visit);
        self.1.visit_spans(visit);
        self.2.visit_spans(visit);
    }
}

impl<K: VisitSpans + Ord, V: VisitSpans> VisitSpans for std::collections::BTreeMap<K, V> {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        *self = std::mem::take(self)
            .into_iter()
            .map(|(mut key, mut value)| {
                key.visit_spans(visit);
                value.visit_spans(visit);
                (key, value)
            })
            .collect();
    }
}

macro_rules! leaves {
    ($($ty:ty),* $(,)?) => { $(
        impl VisitSpans for $ty {
            fn visit_spans(&mut self, _: &mut impl FnMut(&mut Span)) {}
        }
    )* };
}
leaves!(String, bool, f64, u16, u32, usize, BinOp, UnOp, TaskKind);

macro_rules! structs {
    ($($ty:ident { $($field:ident),* $(,)? })*) => { $(
        impl VisitSpans for $ty {
            fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
                let Self { $($field),* } = self;
                $($field.visit_spans(visit);)*
            }
        }
    )* };
}
structs! {
    NameSpans { names, sources }
    File { names, routes, uses, fonts, shapes, styles, keyframes, timelines, fns, tests, launch, components }
    KeyframesDecl { name, frames, span }
    TimelineDecl { name, span }
    KeyframeDecl { selectors, attrs, span }
    RoutesDecl { slot, rows, span }
    RouteDecl { name, pattern, parent, tab, notfound, fields, span }
    FontDecl { name, faces, span }
    FontFaceDecl { weight, italic, source, span }
    TestDecl { name, steps, span }
    FnDecl { name, params, ret, body, span }
    UseDecl { name, path, span }
    StyleDecl { name, attrs, span }
    ShapeDecl { name, fields, span }
    Field { name, ty, span }
    Component { name, props, injects, provides, slot, states, derives, resources, mutations,
        actions, tasks, view, span }
    Binding { name, expr, span }
    ResourceDecl { name, source, args, identity, shape, placeholder, span }
    Placeholder { source, args, span }
    MutationDecl { name, shape, refreshes, then, span }
    Param { name, ty, span }
    Action { name, params, body, span }
    Task { name, kind, timer, span }
    Attr { name, value, span }
}

macro_rules! record_variants {
    ($ty:ident { $($variant:ident { $($field:ident),* $(,)? }),* $(,)? }) => {
        impl VisitSpans for $ty {
            fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
                match self { $(
                    Self::$variant { $($field),* } => { $($field.visit_spans(visit);)* }
                )* }
            }
        }
    };
}
record_variants! {
    Step {
        Tap { target, hover, modifiers, span }, Drag { target, dx, dy, press, over, hold, span },
        Size { width, height, span }, Epoch { value, span }, TimeZone { zone, span },
        Locale { tag, span }, Seed { seed, span }, Type { target, text, span }, Key { target, key, span },
        Pick { target, paths, span }, Clipboard { target, edit, text, span },
        Clock { arg, span }, Screenshot { path, span }, ExpectTree { target, present, span },
        ExpectText { target, value, span }, ExpectState { name, value, span },
    }
}
record_variants! {
    Stmt {
        Let { name, expr, span }, Assign { target, expr, span }, Command { name, args, span },
        Send { target, source, args, span }, Refresh { target, span },
        If { cond, then, otherwise, span }, Match { subject, some, none, span },
    }
}
record_variants! {
    Node {
        Element { tag, positional, attrs, children, span, instance }, Use { name, args, children, span },
        Children { span },
        When { tag, cond, then, otherwise, span }, Each { tag, var, index, list, key, body, span },
        Match { tag, subject, some, none, span },
    }
}
impl VisitSpans for TypeExpr {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        match self {
            Self::Named(_, span) => visit(span),
            Self::Option(inner, span) | Self::List(inner, span) => {
                inner.visit_spans(visit);
                visit(span);
            }
        }
    }
}
impl VisitSpans for TemplatePart {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        match self {
            Self::Text(_) => {}
            Self::Expr(expr) => expr.visit_spans(visit),
        }
    }
}
impl VisitSpans for Expr {
    fn visit_spans(&mut self, visit: &mut impl FnMut(&mut Span)) {
        match self {
            Self::Number(_, span)
            | Self::Str(_, span)
            | Self::Bool(_, span)
            | Self::None(span)
            | Self::EmptyList(span)
            | Self::Ident(_, span) => visit(span),
            Self::Template(parts, span) => {
                parts.visit_spans(visit);
                visit(span);
            }
            Self::NamedArg(_, inner, span)
            | Self::Typed(inner, _, span)
            | Self::Some(inner, span)
            | Self::Member(inner, _, span)
            | Self::Unary(_, inner, span) => {
                inner.visit_spans(visit);
                visit(span);
            }
            Self::Call(_, args, span) => {
                args.visit_spans(visit);
                visit(span);
            }
            Self::Binary(_, lhs, rhs, span) => {
                lhs.visit_spans(visit);
                rhs.visit_spans(visit);
                visit(span);
            }
            Self::Ternary(cond, then, otherwise, span) => {
                cond.visit_spans(visit);
                then.visit_spans(visit);
                otherwise.visit_spans(visit);
                visit(span);
            }
            Self::Match {
                subject,
                var: _,
                some,
                none,
                span,
            } => {
                subject.visit_spans(visit);
                some.visit_spans(visit);
                none.visit_spans(visit);
                visit(span);
            }
            Self::Let {
                name: _,
                value,
                body,
                span,
            } => {
                value.visit_spans(visit);
                body.visit_spans(visit);
                visit(span);
            }
            Self::Arrow {
                params: _,
                body,
                span,
            } => {
                body.visit_spans(visit);
                visit(span);
            }
        }
    }
}
