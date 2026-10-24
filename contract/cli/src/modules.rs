//! Calls to a native module's sources and sends (LLP 1133.002 D13). A call
//! takes keyword arguments only, the interface's options and each
//! platform's words, `apple.desiredAccuracy="threeKilometers"`. After
//! scoping, each call is rewritten into an ordinary data-source call of one
//! record, so the checker, lowering and the seam need nothing new: an option
//! the call writes is `some(value)`, one it omits is `none`, and a platform's
//! words are a record of their own, `none` when the call writes none of them.
//! An omitted word so stays distinct from a written one (D2).

use crate::CompileError;
use contract_syntax::{Expr, Field, File, ModuleDecl, ModuleKind, ShapeDecl, Span, Stmt, TypeExpr};
use std::collections::HashMap;

/// Rewrite every call of a module name in `file`, and declare the records
/// the calls build. Every refusal in one run.
pub(crate) fn desugar(file: &mut File) -> Result<(), Vec<CompileError>> {
    if file.modules.is_empty() {
        return Ok(());
    }
    let modules: HashMap<String, ModuleDecl> = file
        .modules
        .iter()
        .map(|m| (m.name.clone(), m.clone()))
        .collect();
    for m in &file.modules {
        file.shapes.extend(records(m));
    }
    let mut errors = Vec::new();
    let surfaces = surfaces(file);
    for c in &mut file.components {
        for r in &mut c.resources {
            if let Some(p) = &r.placeholder {
                if modules.contains_key(&p.source) {
                    errors.push(error(
                        "contract-module-placeholder",
                        format!(
                            "`{}` is answered on the device, never at build: write `else empty()`",
                            p.source
                        ),
                        p.span,
                    ));
                }
            }
            let Some(m) = modules.get(&r.source) else {
                continue;
            };
            if m.kind == ModuleKind::Send {
                errors.push(error(
                    "contract-module-kind",
                    format!(
                        "`{}` is a send of `{}`: a mutation sends it, `send m = {}(…)`",
                        m.declared, m.package, r.source
                    ),
                    r.span,
                ));
                continue;
            }
            check_answers(m, &r.shape, &r.name, r.span, &mut errors);
            if r.identity.is_some() {
                errors.push(error(
                    "contract-module-with",
                    format!("`{}` takes no `with` context", m.declared),
                    r.span,
                ));
            }
            match call(m, std::mem::take(&mut r.args), r.span, &surfaces) {
                Ok(args) => r.args = args,
                Err(e) => errors.extend(e),
            }
        }
        let shapes: HashMap<String, TypeExpr> = c
            .mutations
            .iter()
            .map(|m| (m.name.clone(), m.shape.clone()))
            .collect();
        for a in &mut c.actions {
            stmts(&mut a.body, &modules, &shapes, &surfaces, &mut errors);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn stmts(
    body: &mut [Stmt],
    modules: &HashMap<String, ModuleDecl>,
    shapes: &HashMap<String, TypeExpr>,
    surfaces: &[String],
    errors: &mut Vec<CompileError>,
) {
    for stmt in body {
        match stmt {
            Stmt::Send {
                target,
                source,
                args,
                span,
            } => {
                let Some(m) = modules.get(source.as_str()) else {
                    continue;
                };
                if m.kind == ModuleKind::Source {
                    errors.push(error(
                        "contract-module-kind",
                        format!(
                            "`{}` is a source of `{}`: a resource reads it, `resource r = {source}(…) as shape …`",
                            m.declared, m.package
                        ),
                        *span,
                    ));
                    continue;
                }
                if let Some(shape) = shapes.get(target.as_str()) {
                    check_answers(m, shape, target, *span, errors);
                }
                match call(m, std::mem::take(args), *span, surfaces) {
                    Ok(rewritten) => *args = rewritten,
                    Err(e) => errors.extend(e),
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                stmts(then, modules, shapes, surfaces, errors);
                stmts(otherwise, modules, shapes, surfaces, errors);
            }
            Stmt::Match { some, none, .. } => {
                stmts(&mut some.1, modules, shapes, surfaces, errors);
                stmts(none, modules, shapes, surfaces, errors);
            }
            Stmt::Call { body, .. } => stmts(body, modules, shapes, surfaces, errors),
            _ => {}
        }
    }
}

/// The records a call of `m` builds: its options, with a field per
/// platform that has words for it, and each platform's words.
fn records(m: &ModuleDecl) -> Vec<ShapeDecl> {
    if takes_nothing(m) {
        return Vec::new();
    }
    let mut fields = m.options.clone();
    let mut out = Vec::new();
    for w in &m.words {
        let shape = words_record(m, &w.platform);
        fields.push(Field {
            name: w.platform.clone(),
            ty: TypeExpr::Option(Box::new(TypeExpr::Named(shape.clone(), m.span)), m.span),
            span: m.span,
        });
        out.push(ShapeDecl {
            name: shape,
            fields: w
                .words
                .iter()
                .map(|f| Field {
                    ty: TypeExpr::Option(Box::new(f.ty.clone()), f.span),
                    ..f.clone()
                })
                .collect(),
            span: m.span,
        });
    }
    out.push(ShapeDecl {
        name: options_record(m),
        fields,
        span: m.span,
    });
    out
}

fn takes_nothing(m: &ModuleDecl) -> bool {
    m.options.is_empty() && m.words.is_empty()
}

fn options_record(m: &ModuleDecl) -> String {
    format!("{}__options", m.name)
}

fn words_record(m: &ModuleDecl, platform: &str) -> String {
    format!("{}__{platform}", m.name)
}

/// `args`, a call's keyword arguments, as the one record the source takes.
/// `surfaces` are the canvases' surface names the file places.
fn call(
    m: &ModuleDecl,
    args: Vec<Expr>,
    span: Span,
    surfaces: &[String],
) -> Result<Vec<Expr>, Vec<CompileError>> {
    let mut errors = Vec::new();
    let mut options: Vec<(String, Expr)> = Vec::new();
    let mut words: Vec<(String, Vec<(String, Expr)>)> = Vec::new();
    for arg in args {
        let Expr::NamedArg(name, value, at) = arg else {
            errors.push(error(
                "contract-module-positional",
                format!(
                    "`{}` takes keyword arguments only, `name=value`; its options are {}",
                    m.declared,
                    list(m.options.iter().map(|f| f.name.as_str()))
                ),
                arg.span(),
            ));
            continue;
        };
        let parts: Vec<&str> = name.split('.').collect();
        match parts.as_slice() {
            [option] => {
                if !m.options.iter().any(|f| f.name == *option) {
                    let platform = m.words.iter().any(|w| w.platform == *option);
                    errors.push(error(
                        "contract-module-argument",
                        if platform {
                            format!(
                                "a platform's words are written one by one, `{option}.word=value`"
                            )
                        } else {
                            format!(
                                "`{}` has no option `{option}`; its options are {}",
                                m.declared,
                                list(m.options.iter().map(|f| f.name.as_str()))
                            )
                        },
                        at,
                    ));
                } else if options.iter().any(|(n, _)| n == option) {
                    errors.push(twice(&name, at));
                } else if let Some(e) = m
                    .options
                    .iter()
                    .find(|f| f.name == *option)
                    .and_then(|f| member(&name, &f.ty, &value))
                    .or_else(|| bounded(m, option, &value, surfaces))
                {
                    errors.push(e);
                } else {
                    options.push((option.to_string(), *value));
                }
            }
            [platform, word] => {
                let Some(w) = m.words.iter().find(|w| w.platform == *platform) else {
                    errors.push(error(
                        "contract-module-argument",
                        format!(
                            "no implementation of `{}` declares words for `{}` on `{platform}`; the platforms with words are {}",
                            m.package,
                            m.declared,
                            list(m.words.iter().map(|w| w.platform.as_str()))
                        ),
                        at,
                    ));
                    continue;
                };
                if !w.words.iter().any(|f| f.name == *word) {
                    errors.push(error(
                        "contract-module-argument",
                        format!(
                            "`{}` declares no word `{word}` for `{}`; its words are {}",
                            w.package,
                            m.declared,
                            list(w.words.iter().map(|f| f.name.as_str()))
                        ),
                        at,
                    ));
                    continue;
                }
                let at_platform = match words.iter_mut().find(|(p, _)| p == platform) {
                    Some((_, given)) => given,
                    None => {
                        words.push((platform.to_string(), Vec::new()));
                        &mut words.last_mut().expect("just pushed").1
                    }
                };
                let declared = w.words.iter().find(|f| f.name == *word).map(|f| &f.ty);
                if at_platform.iter().any(|(n, _)| n == word) {
                    errors.push(twice(&name, at));
                } else if let Some(e) = declared.and_then(|ty| member(&name, ty, &value)) {
                    errors.push(e);
                } else {
                    at_platform.push((word.to_string(), *value));
                }
            }
            _ => errors.push(error(
                "contract-module-argument",
                format!("`{name}` is one option or one platform's word, `platform.word`"),
                at,
            )),
        }
    }
    for f in &m.options {
        let optional = matches!(f.ty, TypeExpr::Option(..));
        if !optional && !options.iter().any(|(n, _)| *n == f.name) {
            errors.push(error(
                "contract-module-argument",
                format!("`{}` needs `{}=…`", m.declared, f.name),
                span,
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if takes_nothing(m) {
        return Ok(Vec::new());
    }
    let take = |given: &mut Vec<(String, Expr)>, f: &Field, wrap: bool| -> Expr {
        match given.iter().position(|(n, _)| *n == f.name) {
            Some(i) => {
                let value = given.remove(i).1;
                if wrap {
                    let at = value.span();
                    Expr::Some(Box::new(value), at)
                } else {
                    value
                }
            }
            None => Expr::None(span),
        }
    };
    let mut fields: Vec<Expr> = m
        .options
        .iter()
        .map(|f| {
            let optional = matches!(f.ty, TypeExpr::Option(..));
            Expr::NamedArg(
                f.name.clone(),
                Box::new(take(&mut options, f, optional)),
                span,
            )
        })
        .collect();
    for w in &m.words {
        let given = words.iter_mut().find(|(p, _)| *p == w.platform);
        let value = match given {
            None => Expr::None(span),
            Some((_, given)) => {
                let record = w
                    .words
                    .iter()
                    .map(|f| Expr::NamedArg(f.name.clone(), Box::new(take(given, f, true)), span))
                    .collect();
                Expr::Some(
                    Box::new(Expr::Call(words_record(m, &w.platform), record, span)),
                    span,
                )
            }
        };
        fields.push(Expr::NamedArg(w.platform.clone(), Box::new(value), span));
    }
    Ok(vec![Expr::Call(options_record(m), fields, span)])
}

/// A literal outside what the declaration admits for `option` (LLP 1133.013
/// D12 items 2 and 3): a number past its `min` or `max` (a poll faster than
/// its floor, a batch past its bound), or a surface no canvas of the file
/// places. A computed one is the runtime's to clamp or refuse.
fn bounded(
    m: &ModuleDecl,
    option: &str,
    value: &Expr,
    surfaces: &[String],
) -> Option<CompileError> {
    let sub: serde_json::Value = serde_json::from_str(&m.substitute).unwrap_or_default();
    match value {
        Expr::Number(n, at) => {
            let b = &sub["bounds"][option];
            let (min, max) = (b["min"].as_f64(), b["max"].as_f64());
            let out = min.is_some_and(|min| *n < min) || max.is_some_and(|max| *n > max);
            out.then(|| {
                let range = match (min, max) {
                    (Some(min), Some(max)) => format!("from {min} to {max}"),
                    (Some(min), None) => format!("at least {min}"),
                    (None, Some(max)) => format!("at most {max}"),
                    (None, None) => String::new(),
                };
                error(
                    "contract-module-bound",
                    format!("`{}`'s `{option}` is {range}, given {n}", m.declared),
                    *at,
                )
            })
        }
        Expr::Str(name, at) => {
            let surface =
                (sub["surfaceOptions"].as_array().into_iter().flatten()).any(|o| o == option);
            (surface && !surfaces.contains(name)).then(|| {
                error(
                    "contract-module-surface",
                    format!(
                        "`{}`'s `{option}` names a canvas's surface; no canvas here places `{name}`, and the file's are {}",
                        m.declared,
                        list(surfaces.iter().map(String::as_str))
                    ),
                    *at,
                )
            })
        }
        _ => None,
    }
}

/// The surface names the canvases of `file` place, `canvas surface=world(…)`.
fn surfaces(file: &File) -> Vec<String> {
    use contract_syntax::Node;
    fn walk(nodes: &[Node], out: &mut Vec<String>) {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    for a in attrs
                        .iter()
                        .filter(|a| tag == "canvas" && a.name == "surface")
                    {
                        if let Expr::Call(name, _, _) = &a.value {
                            out.push(name.clone());
                        }
                    }
                    walk(children, out);
                }
                Node::Use { children, .. } => walk(children, out),
                Node::Each { body, .. } => walk(body, out),
                Node::When {
                    then, otherwise, ..
                } => {
                    walk(then, out);
                    walk(otherwise, out);
                }
                Node::Match { some, none, .. } => {
                    walk(&some.1, out);
                    walk(none, out);
                }
                Node::Children { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    for c in &file.components {
        walk(&c.view, &mut out);
    }
    out
}

/// A string literal outside its argument's vocabulary, refused by the name
/// the call writes rather than the record it becomes.
fn member(name: &str, ty: &TypeExpr, value: &Expr) -> Option<CompileError> {
    let ty = match ty {
        TypeExpr::Option(inner, _) => inner,
        ty => ty,
    };
    let (TypeExpr::Enum(members, _), Expr::Str(given, at)) = (ty, value) else {
        return None;
    };
    (!members.contains(given)).then(|| {
        error(
            "contract-module-argument",
            format!(
                "`{name}` is one of {}, given `{given:?}`",
                list(members.iter().map(String::as_str))
            ),
            *at,
        )
    })
}

/// A resource's or a mutation's `as shape` against what the module answers.
fn check_answers(
    m: &ModuleDecl,
    shape: &TypeExpr,
    name: &str,
    span: Span,
    errors: &mut Vec<CompileError>,
) {
    if !same(shape, &m.answers) {
        errors.push(error(
            "contract-module-answers",
            format!(
                "`{name}` reads `{}`, which answers `{}`: write `as shape {}`",
                m.declared,
                written(&m.answers),
                written(&m.answers)
            ),
            span,
        ));
    }
}

/// Two written types alike, wherever each was written.
fn same(a: &TypeExpr, b: &TypeExpr) -> bool {
    match (a, b) {
        (TypeExpr::Named(a, _), TypeExpr::Named(b, _)) => a == b,
        (TypeExpr::Enum(a, _), TypeExpr::Enum(b, _)) => a == b,
        (TypeExpr::Option(a, _), TypeExpr::Option(b, _))
        | (TypeExpr::List(a, _), TypeExpr::List(b, _)) => same(a, b),
        _ => false,
    }
}

fn written(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Named(n, _) => n.clone(),
        TypeExpr::Enum(members, _) => format!(
            "enum<{}>",
            members
                .iter()
                .map(|m| format!("{m:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Option(t, _) => format!("option<{}>", written(t)),
        TypeExpr::List(t, _) => format!("{}[]", written(t)),
    }
}

fn list<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let names: Vec<String> = names.map(|n| format!("`{n}`")).collect();
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

fn twice(name: &str, at: Span) -> CompileError {
    error(
        "call-argument-twice",
        format!("`{name}` is given twice"),
        at,
    )
}

fn error(id: &str, message: String, span: Span) -> CompileError {
    CompileError {
        pass: "use",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    }
}
