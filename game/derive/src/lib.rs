//! Rust groups delimit bodies and attributes; angle depth separates fields
//! whose types contain commas. No second Rust syntax tree is needed.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use proc_macro::{Delimiter, Spacing, TokenStream, TokenTree};

mod kind;

/// Generate a component bundle and shared/exclusive row views over existing storage.
#[proc_macro_derive(Kind, attributes(read, child))]
pub fn kind(input: TokenStream) -> TokenStream {
    match kind::expand(input) {
        Ok(s) => s.parse().unwrap_or_else(|_| {
            r#"compile_error!("Kind could not generate valid Rust");"#
                .parse()
                .unwrap()
        }),
        Err(e) => format!("::core::compile_error!({e:?});").parse().unwrap(),
    }
}

/// Implement the streaming data contract for a nongeneric struct or enum.
#[proc_macro_derive(Data, attributes(data))]
pub fn data(input: TokenStream) -> TokenStream {
    derive(input, None)
}

/// Implement data and use the type's spelling as its component name.
#[proc_macro_derive(Component, attributes(data))]
pub fn component(input: TokenStream) -> TokenStream {
    derive(input, Some("Component"))
}

/// Implement data and use the type's spelling as its resource name.
#[proc_macro_derive(Resource, attributes(data))]
pub fn resource(input: TokenStream) -> TokenStream {
    derive(input, Some("Resource"))
}

struct Field {
    name: String,
    skip: bool,
    ty: TokenStream,
    shared: bool,
    child: Option<(String, String)>,
}
struct Body {
    fields: Vec<Field>,
    shape: Shape,
}
#[derive(PartialEq)]
enum Shape {
    Unit,
    Named,
    Tuple,
}
struct Arm {
    name: String,
    body: Body,
}

fn derive(input: TokenStream, marker: Option<&str>) -> TokenStream {
    match expand(input, marker) {
        Ok(s) => s.parse().expect("derive emitted Rust"),
        Err(e) => format!("::core::compile_error!({e:?});").parse().unwrap(),
    }
}
fn punct(t: &TokenTree, c: char) -> bool {
    matches!(t, TokenTree::Punct(p) if p.as_char() == c)
}
fn strip(tokens: &[TokenTree], field: bool) -> Result<(&[TokenTree], bool), String> {
    let mut i = 0;
    let mut skip = false;
    while i + 1 < tokens.len() && punct(&tokens[i], '#') {
        if let TokenTree::Group(g) = &tokens[i + 1] {
            let a: Vec<_> = g.stream().into_iter().collect();
            if !field
                && a.first()
                    .is_some_and(|t| matches!(t.to_string().as_str(), "live" | "restart"))
            {
                return Err("live/restart attribute is only meaningful on a field".into());
            }
            if a.first().is_some_and(|t| t.to_string() == "read") && (!field || a.len() != 1) {
                return Err("expected #[read] on a Kind field, without arguments".into());
            }
            if !field && a.first().is_some_and(|t| t.to_string() == "child") {
                return Err("child attribute belongs on a Kind component field".into());
            }
            if a.first().is_some_and(|t| t.to_string() == "data") {
                if !field {
                    return Err("data attribute is only meaningful on a field".into());
                }
                if a.len() != 2
                    || !matches!(&a[1], TokenTree::Group(g)
                    if g.delimiter() == Delimiter::Parenthesis && g.stream().to_string() == "skip")
                {
                    return Err("unknown data attribute; expected data(skip)".into());
                }
                skip = true;
            }
        }
        i += 2;
    }
    if tokens.get(i).is_some_and(|t| t.to_string() == "pub") {
        i += 1;
        if matches!(tokens.get(i), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
        {
            i += 1;
        }
    }
    Ok((&tokens[i..], skip))
}
fn split(stream: TokenStream) -> Result<Vec<Vec<TokenTree>>, String> {
    let tokens: Vec<_> = stream.into_iter().collect();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut depth = 0usize;
    // Groups contain const expressions; at this level types have no comparisons.
    // Each > in >> closes one level, but a joint -> closes none.
    while i < tokens.len() {
        if matches!(&tokens[i], TokenTree::Punct(p) if p.as_char() == '-' && p.spacing() == Spacing::Joint)
            && tokens.get(i + 1).is_some_and(|t| punct(t, '>'))
        {
            i += 2;
            continue;
        }
        if punct(&tokens[i], '<') {
            depth += 1;
        } else if punct(&tokens[i], '>') {
            depth = depth.checked_sub(1).ok_or("unbalanced generic depth")?;
        } else if punct(&tokens[i], ',') && depth == 0 {
            if start != i {
                out.push(tokens[start..i].to_vec());
            }
            start = i + 1;
        }
        i += 1;
    }
    if depth != 0 {
        return Err("unbalanced generic depth".into());
    }
    if start != tokens.len() {
        out.push(tokens[start..].to_vec());
    }
    Ok(out)
}
fn body(group: Option<&TokenTree>) -> Result<Body, String> {
    let Some(TokenTree::Group(g)) = group else {
        return Ok(Body {
            fields: vec![],
            shape: Shape::Unit,
        });
    };
    let shape = if g.delimiter() == Delimiter::Brace {
        Shape::Named
    } else {
        Shape::Tuple
    };
    let mut fields = Vec::new();
    for (i, f) in split(g.stream())?.iter().enumerate() {
        let shared = f.windows(2).any(|w| {
            punct(&w[0], '#')
                && matches!(&w[1], TokenTree::Group(g) if g.stream().to_string() == "read")
        });
        let mut child = None;
        for pair in f.windows(2) {
            if punct(&pair[0], '#') {
                if let TokenTree::Group(g) = &pair[1] {
                    let a: Vec<_> = g.stream().into_iter().collect();
                    if a.first().is_some_and(|t| t.to_string() == "child") {
                        let expected = "expected #[child(\"name\", saved_id_field)] exactly once";
                        let Some(TokenTree::Group(args)) = a.get(1) else {
                            return Err(expected.into());
                        };
                        let args: Vec<_> = args.stream().into_iter().collect();
                        if child.is_some()
                            || a.len() != 2
                            || args.len() != 3
                            || !punct(&args[1], ',')
                            || !matches!(&args[0], TokenTree::Literal(l) if l.to_string().starts_with('"'))
                            || !matches!(&args[2], TokenTree::Ident(_))
                        {
                            return Err(expected.into());
                        }
                        child = Some((args[0].to_string(), args[2].to_string()));
                    }
                }
            }
        }
        let (f, skip) = strip(f, true)?;
        let name = if shape == Shape::Named {
            if !matches!(f.first(), Some(TokenTree::Ident(_)))
                || !f.get(1).is_some_and(|t| punct(t, ':'))
            {
                return Err("expected a named field".into());
            }
            f[0].to_string()
        } else {
            i.to_string()
        };
        let start = if shape == Shape::Named { 2 } else { 0 };
        fields.push(Field {
            name,
            skip,
            ty: f[start..].iter().cloned().collect(),
            shared,
            child,
        });
    }
    Ok(Body { fields, shape })
}
fn expand(input: TokenStream, marker: Option<&str>) -> Result<String, String> {
    let tokens: Vec<_> = input.into_iter().collect();
    let name = tokens
        .windows(2)
        .find(|w| matches!(w[0].to_string().as_str(), "struct" | "enum"))
        .map(|w| w[1].to_string())
        .unwrap_or_default();
    expand_type(&tokens, marker).map_err(|e| format!("{name}: {e}"))
}
fn expand_type(tokens: &[TokenTree], marker: Option<&str>) -> Result<String, String> {
    let (tokens, _) = strip(tokens, false)?;
    let kind = tokens.first().map(ToString::to_string).unwrap_or_default();
    let name = tokens.get(1).map(ToString::to_string).unwrap_or_default();
    if kind != "struct" && kind != "enum" {
        return Err("Data requires a struct or enum".into());
    }
    if tokens
        .get(2)
        .is_some_and(|t| punct(t, '<') || t.to_string() == "where")
        || tokens.iter().any(|t| t.to_string() == "where")
        || has_lifetime(tokens)
    {
        return Err("Data does not support generics or lifetimes".into());
    }
    let (write, read, moving, settle) = if kind == "struct" {
        let b = body(tokens.get(2))?;
        let access: Vec<_> = b
            .fields
            .iter()
            .map(|f| format!("self.{}", f.name))
            .collect();
        (
            write_body(&b, &access),
            read_body(&b, &access),
            moving_body(&b, &access),
            settle_body(&b, &access),
        )
    } else {
        let Some(TokenTree::Group(g)) = tokens.get(2) else {
            return Err("expected enum body".into());
        };
        let mut arms = Vec::new();
        for a in split(g.stream())? {
            let (a, _) = strip(&a, false)?;
            if a.iter().any(|t| punct(t, '=')) {
                return Err(
                    "explicit enum discriminants are unsupported; hashes use declaration order"
                        .into(),
                );
            }
            let arm = a.first().ok_or("empty enum arm")?.to_string();
            arms.push(Arm {
                name: arm,
                body: body(a.get(1))?,
            });
        }
        let mut write = String::from("match self {");
        let mut moving = String::from("match self {");
        let mut settle = String::from("match self {");
        let mut read = String::from("let arm = r.variant()?; match arm.as_str() {");
        for (index, arm) in arms.iter().enumerate() {
            let b = &arm.body;
            let vars: Vec<_> = (0..b.fields.len()).map(|i| format!("v{i}")).collect();
            let pat = pattern(&arm.name, b, &vars);
            let refs: Vec<_> = vars.iter().map(|v| format!("*{v}")).collect();
            let write_vars: Vec<_> = vars
                .iter()
                .zip(&b.fields)
                .map(|(v, f)| if f.skip { "_".into() } else { v.clone() })
                .collect();
            let write_pat = pattern(&arm.name, b, &write_vars);
            moving += &format!("{write_pat} => {},", moving_body(b, &refs));
            settle += &format!("{write_pat} => {},", settle_body(b, &refs));
            write += &format!(
                "{write_pat} => {{ w.variant({:?}, {index}); {} w.end_variant(); }},",
                clean(&arm.name),
                write_body(b, &refs)
            );
            let defaults = vec!["::core::default::Default::default()".to_owned(); b.fields.len()];
            let wildcards = vec!["_".to_owned(); b.fields.len()];
            let bind = if arms.len() == 1 {
                format!("let {pat} = self; (|| -> ::core::result::Result<(), ::exact_game::DataError> {{ {} ::core::result::Result::Ok(()) }})().map_err(|e| e.at(&arm))?;", read_body(b, &refs))
            } else {
                format!("if let {pat} = self {{ (|| -> ::core::result::Result<(), ::exact_game::DataError> {{ {} ::core::result::Result::Ok(()) }})().map_err(|e| e.at(&arm))?; }}", read_body(b, &refs))
            };
            read += &format!(
                "{:?} => {{ if !::core::matches!(self, {}) {{ *self = {}; }} {bind} }},",
                clean(&arm.name),
                pattern(&arm.name, b, &wildcards),
                pattern(&arm.name, b, &defaults)
            );
        }
        write += "}";
        read += "_ => return ::core::result::Result::Err(::exact_game::DataError::new(::std::format!(\"unknown variant {}\", arm))), } r.end_variant()?;";
        moving += "}";
        settle += "}";
        (write, read, moving, settle)
    };
    let mut out = format!("impl ::exact_game::Data for {name} {{ fn moving(&self, now: ::exact_game::Now) -> ::core::primitive::bool {{ let _ = now; {moving} }} fn settle_tick(&self, now: ::exact_game::Now) -> ::core::option::Option<::core::primitive::u64> {{ {settle} }} fn write(&self, w: &mut dyn ::exact_game::Writer) {{ {write} }} fn read(&mut self, r: &mut dyn ::exact_game::Reader) -> ::core::result::Result<(), ::exact_game::DataError> {{ {read} ::core::result::Result::Ok(()) }} }}");
    if let Some(marker) = marker {
        let saved_fields = if marker == "Component" && kind == "struct" {
            let fields = body(tokens.get(2))?.fields;
            let names: Vec<_> = fields
                .iter()
                .filter(|f| !f.skip)
                .map(|f| clean(&f.name))
                .collect();
            format!("const SAVED_FIELDS: &'static [&'static str] = &{names:?};")
        } else {
            String::new()
        };
        out += &format!(
            "impl ::exact_game::{marker} for {name} {{ const NAME: &'static ::core::primitive::str = {:?}; {saved_fields} }}",
            clean(&name)
        );
    }
    Ok(out)
}
fn has_lifetime(tokens: &[TokenTree]) -> bool {
    tokens.iter().any(|t| match t {
        TokenTree::Punct(p) => p.as_char() == '\'',
        TokenTree::Group(g) => has_lifetime(&g.stream().into_iter().collect::<Vec<_>>()),
        _ => false,
    })
}
fn clean(name: &str) -> &str {
    name.strip_prefix("r#").unwrap_or(name)
}
fn pattern(name: &str, b: &Body, values: &[String]) -> String {
    match b.shape {
        Shape::Unit => format!("Self::{name}"),
        Shape::Named => format!(
            "Self::{name} {{ {} }}",
            b.fields
                .iter()
                .zip(values)
                .map(|(f, v)| format!("{}: {v}", f.name))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Shape::Tuple => format!("Self::{name}({})", values.join(",")),
    }
}
fn write_body(b: &Body, access: &[String]) -> String {
    let named = b.shape != Shape::Tuple;
    let mut s = if named {
        "w.begin_struct();".into()
    } else {
        format!(
            "w.begin_seq({});",
            b.fields.iter().filter(|f| !f.skip).count()
        )
    };
    for (f, a) in b.fields.iter().zip(access).filter(|(f, _)| !f.skip) {
        if named {
            s += &format!("w.field({:?});", clean(&f.name));
        } else {
            s += "w.item();";
        }
        s += &format!("::exact_game::Data::write(&{a}, w);");
    }
    s += if named {
        "w.end_struct();"
    } else {
        "w.end_seq();"
    };
    s
}
fn read_body(b: &Body, access: &[String]) -> String {
    let named = b.shape != Shape::Tuple;
    let mut s = String::new();
    for (_, a) in b
        .fields
        .iter()
        .zip(access)
        .filter(|(f, _)| f.skip || !named)
    {
        s += &format!("{a} = ::core::default::Default::default();");
    }
    s += if named {
        "r.begin_struct()?; while let ::core::option::Option::Some(field) = r.field()? { match field.as_str() {"
    } else {
        "r.begin_seq()?; let mut index = 0usize; while r.item()? { match index {"
    };
    for (index, (f, a)) in b
        .fields
        .iter()
        .zip(access)
        .filter(|(f, _)| !f.skip)
        .enumerate()
    {
        let key = if named {
            format!("{:?}", clean(&f.name))
        } else {
            index.to_string()
        };
        s += &format!(
            "{key} => ::exact_game::Data::read(&mut {a}, r).map_err(|e| e.at({:?}))?,",
            clean(&f.name)
        );
    }
    s += if named {
        "_ => r.skip().map_err(|e| e.at(&field))?, }"
    } else {
        "_ => r.skip().map_err(|e| e.at(index))?, }"
    };
    if !named {
        s += "index += 1;";
    }
    s += "}";
    s
}

fn moving_body(b: &Body, access: &[String]) -> String {
    let parts: Vec<_> = b
        .fields
        .iter()
        .zip(access)
        .filter(|(f, _)| !f.skip)
        .map(|(_, a)| format!("::exact_game::Data::moving(&{a}, now)"))
        .collect();
    if parts.is_empty() {
        "false".into()
    } else {
        parts.join(" || ")
    }
}

fn settle_body(b: &Body, access: &[String]) -> String {
    let maxes = b
        .fields
        .iter()
        .zip(access)
        .filter(|(f, _)| !f.skip)
        .map(|(_, a)| format!(".max(::exact_game::Data::settle_tick(&{a}, now)?)"))
        .collect::<String>();
    format!("::core::option::Option::Some(now.tick{maxes})")
}

/// Decode ordered, typed canvas arguments; fields are setup unless marked live.
#[proc_macro_derive(Args, attributes(live, restart))]
pub fn args(input: TokenStream) -> TokenStream {
    let data = expand(input.clone(), None);
    match expand_args(input).and_then(|args| data.map(|data| data + &args)) {
        Ok(s) => s.parse().expect("Args derive emitted Rust"),
        Err(e) => format!("::core::compile_error!({e:?});").parse().unwrap(),
    }
}
fn expand_args(input: TokenStream) -> Result<String, String> {
    let tokens: Vec<_> = input.into_iter().collect();
    let (tokens, _) = strip(&tokens, false)?;
    if tokens.first().is_none_or(|t| t.to_string() != "struct") {
        return Err("Args requires a named struct".into());
    }
    let name = tokens[1].to_string();
    let Some(TokenTree::Group(g)) = tokens.get(2) else {
        return Err(format!("{name}: Args requires a nongeneric named struct"));
    };
    if g.delimiter() != Delimiter::Brace {
        return Err(format!("{name}: Args requires named fields"));
    }
    let mut fields = Vec::new();
    let mut decode = Vec::new();
    let mut values = Vec::new();
    let mut checks = Vec::new();
    let mut changed = vec!["false".to_owned()];
    for (i, tokens) in split(g.stream())?.iter().enumerate() {
        let live = tokens.windows(2).any(|w| {
            punct(&w[0], '#')
                && matches!(&w[1], TokenTree::Group(g) if g.stream().to_string() == "live")
        });
        let restart = tokens.windows(2).any(|w| {
            punct(&w[0], '#')
                && matches!(&w[1], TokenTree::Group(g) if g.stream().to_string() == "restart")
        });
        let (f, _) = strip(tokens, true)?;
        if f.len() != 3 || !punct(&f[1], ':') {
            return Err(format!(
                "{name}: expected a named field with a supported scalar type"
            ));
        }
        let field = f[0].to_string();
        let ty = f[2].to_string();
        for pair in tokens.windows(2) {
            if punct(&pair[0], '#') {
                if let TokenTree::Group(g) = &pair[1] {
                    let a: Vec<_> = g.stream().into_iter().collect();
                    if a.first()
                        .is_some_and(|t| matches!(t.to_string().as_str(), "live" | "restart"))
                        && a.len() != 1
                    {
                        return Err(format!(
                            "{name}.{field}: expected #[live] or #[restart], without arguments"
                        ));
                    }
                }
            }
        }
        if restart && (live || ty != "bool") {
            return Err(format!(
                "{name}.{field}: #[restart] requires a bool and cannot be #[live]"
            ));
        }
        values.push(match ty.as_str() {
            "String" => format!("::exact_game::Value::str(&self.{field})"),
            "bool" => format!("::exact_game::Value::Bool(self.{field})"),
            _ => format!("::exact_game::Value::Number(self.{field} as f64)"),
        });
        if !matches!(
            ty.as_str(),
            "bool" | "u32" | "u64" | "i32" | "i64" | "f32" | "f64" | "String"
        ) {
            return Err(format!("{name}.{field}: unsupported Args type {ty}"));
        }
        let invalid = match ty.as_str() {
            "f32" | "f64" => Some(format!("!self.{field}.is_finite()")),
            "u64" => Some(format!("self.{field} > 9_007_199_254_740_991")),
            "i64" => Some(format!(
                "!(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&self.{field})"
            )),
            _ => None,
        };
        if let Some(invalid) = invalid {
            checks.push(format!("if {invalid} {{ return Err(format!(\"{{}}: expected {{}}\", {:?}, <{ty} as ::exact_game::args::Argument>::EXPECTED)); }}", clean(&field)));
        }
        let kind = if live {
            "Live"
        } else if restart {
            "Restart"
        } else {
            "Setup"
        };
        fields.push(format!(
            "({:?}, ::exact_game::ArgumentKind::{kind})",
            clean(&field)
        ));
        decode.push(format!(
            "{field}: ::exact_game::args::field::<{ty}>(values, {i}, {:?})?",
            clean(&field)
        ));
        if !live {
            changed.push(if ty == "f32" || ty == "f64" {
                format!("self.{field}.to_bits() != next.{field}.to_bits()")
            } else {
                format!("self.{field} != next.{field}")
            });
        }
    }
    Ok(format!(
        "impl ::exact_game::Args for {name} {{
        const FIELDS: &'static [(&'static str, ::exact_game::ArgumentKind)] = &[{}];
        fn decode(values: &[::exact_game::Value]) -> Result<Self, String> {{
            ::exact_game::args::arity(values, Self::FIELDS)?; Ok(Self {{ {} }})
        }}
        fn check_scalars(&self) -> Result<(), String> {{ {} Ok(()) }}
        fn values(&self) -> Vec<::exact_game::Value> {{ vec![{}] }}
        fn setup_changed(&self, next: &Self) -> bool {{ let _ = next; {} }}
    }}",
        fields.join(","),
        decode.join(","),
        checks.join(""),
        values.join(","),
        changed.join(" || ")
    ))
}
