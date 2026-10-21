use super::*;
use std::cell::Cell;

thread_local! {
    static STRING_ARM_VISITS: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(super) fn record_string_arm_visit() {
    STRING_ARM_VISITS.with(|visits| {
        if let Some(count) = visits.get() {
            assert!(count < 1_000, "shared initializer exceeded 1,000 visits");
            visits.set(Some(count + 1));
        }
    });
}

#[test]
fn shared_numeric_lets_are_checked_once_per_binding() {
    let span = Span::point(1, 1);
    let mut expr = Expr::Ident("a29".into(), span);
    for depth in (0..30).rev() {
        let value = if depth == 0 {
            Expr::Number(0.5, span)
        } else {
            let name = format!("a{}", depth - 1);
            Expr::Ternary(
                Box::new(Expr::Bool(true, span)),
                Box::new(Expr::Ident(name.clone(), span)),
                Box::new(Expr::Ident(name, span)),
                span,
            )
        };
        expr = Expr::Let {
            name: format!("a{depth}"),
            value: Box::new(value),
            body: Box::new(expr),
            span,
        };
    }
    struct ResetVisits;
    impl Drop for ResetVisits {
        fn drop(&mut self) {
            STRING_ARM_VISITS.with(|visits| visits.set(None));
        }
    }
    let _reset = ResetVisits;
    STRING_ARM_VISITS.with(|visits| visits.set(Some(0)));
    assert_eq!(
        computed_string_arm(
            &expr,
            &Scope::default(),
            &Shapes::default(),
            &[StyleId::Opacity],
            &[]
        ),
        None
    );
    let visits = STRING_ARM_VISITS.with(Cell::get).unwrap();
    assert!(visits < 128, "shared initializer expanded {visits} times");
}

#[test]
fn expanded_let_checks_only_the_branch_its_body_uses() {
    let span = Span::point(1, 1);
    let string_span = Span::point(1, 12);
    let choice = Expr::Ternary(
        Box::new(Expr::Bool(true, span)),
        Box::new(Expr::Number(0.5, span)),
        Box::new(Expr::Ident("label".into(), string_span)),
        span,
    );
    let mut scope = Scope::default();
    scope.push(vec![("label".into(), Ref::Local(0), Ty::String)]);
    let expr = Expr::Let {
        name: "choice".into(),
        value: Box::new(choice.clone()),
        body: Box::new(Expr::Ident("choice".into(), span)),
        span,
    };
    assert_eq!(
        computed_string_arm(&expr, &scope, &Shapes::default(), &[StyleId::Opacity], &[]),
        Some(string_span)
    );
    let unused = Expr::Let {
        name: "choice".into(),
        value: Box::new(choice),
        body: Box::new(Expr::Number(0.5, span)),
        span,
    };
    assert_eq!(
        computed_string_arm(
            &unused,
            &scope,
            &Shapes::default(),
            &[StyleId::Opacity],
            &[]
        ),
        None
    );
}

#[test]
fn mixed_types_inside_computed_arms_and_match_subjects_are_found() {
    let span = Span::point(1, 1);
    let mixed_span = Span::point(1, 12);
    let mut scope = Scope::default();
    scope.push(vec![("label".into(), Ref::Local(0), Ty::String)]);
    let mixed = Expr::Ternary(
        Box::new(Expr::Bool(true, span)),
        Box::new(Expr::Ident("label".into(), span)),
        Box::new(Expr::Number(1.0, span)),
        mixed_span,
    );
    let nested = Expr::Ternary(
        Box::new(Expr::Bool(true, span)),
        Box::new(Expr::Number(0.5, span)),
        Box::new(Expr::Call("toString".into(), vec![mixed.clone()], span)),
        span,
    );
    assert_eq!(
        computed_string_arm(
            &nested,
            &scope,
            &Shapes::default(),
            &[StyleId::Opacity],
            &[]
        ),
        Some(mixed_span)
    );
    let subject = Expr::Ternary(
        Box::new(Expr::Bool(true, span)),
        Box::new(Expr::Some(
            Box::new(Expr::Ident("label".into(), span)),
            span,
        )),
        Box::new(Expr::Some(Box::new(Expr::Number(1.0, span)), span)),
        mixed_span,
    );
    let matched = Expr::Match {
        subject: Box::new(subject),
        var: "x".into(),
        some: Box::new(Expr::Ident("x".into(), span)),
        none: Box::new(Expr::Number(0.0, span)),
        span,
    };
    assert_eq!(
        computed_string_arm(
            &matched,
            &scope,
            &Shapes::default(),
            &[StyleId::Opacity],
            &[]
        ),
        Some(mixed_span)
    );
}
