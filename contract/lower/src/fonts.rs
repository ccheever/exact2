//! Declared font tables and literal uses. @ref LLP 1019 D1.

use super::*;
use exact_plan::StackMemberKind;
use std::path::Component as PathComponent;

impl Lowerer<'_> {
    pub(super) fn declare_fonts(
        &mut self,
        file: &File,
        asset_root: Option<&Path>,
    ) -> Result<(), LowerError> {
        const GENERICS: &[(&str, u32)] = &[
            ("system-ui", 0),
            ("ui-sans-serif", 1),
            ("sans-serif", 2),
            ("ui-serif", 3),
            ("serif", 4),
            ("ui-monospace", 5),
            ("monospace", 6),
            ("ui-rounded", 7),
        ];
        self.font_stacks.extend(
            GENERICS
                .iter()
                .map(|(name, id)| ((*name).to_string(), StacksId(*id))),
        );
        let root = if file.fonts.is_empty() {
            None
        } else {
            let Some(root) = asset_root else {
                return err(
                    "lower-font-path",
                    "a font source is relative to its app directory; compile this source with `compile_path`",
                    file.fonts[0].span,
                );
            };
            Some(root.canonicalize().map_err(|e| LowerError {
                id: "lower-font-unreadable",
                message: format!("font asset root `{}` is unreadable: {e}", root.display()),
                span: file.fonts[0].span,
            })?)
        };
        for font in &file.fonts {
            if self
                .font_stacks
                .keys()
                .any(|name| name.eq_ignore_ascii_case(&font.name))
            {
                return err(
                    "lower-font-duplicate",
                    format!(
                        "font family `{}` is already declared or is a generic family",
                        font.name
                    ),
                    font.span,
                );
            }
            let mut seen = BTreeMap::new();
            let mut faces = Vec::new();
            let mut variable = Vec::new();
            for face in &font.faces {
                if seen.insert((face.weight, face.italic), ()).is_some() {
                    let weight = match face.weight {
                        Some([low, high]) if low == high => format!("weight {low}"),
                        Some([low, high]) => format!("weights {low} {high}"),
                        None => "its file's weights".into(),
                    };
                    return err(
                        "lower-font-face-duplicate",
                        format!(
                            "font `{}` declares {weight}{} twice",
                            font.name,
                            if face.italic { " italic" } else { "" }
                        ),
                        face.span,
                    );
                }
                let source = Path::new(&face.source);
                let lower = face.source.to_ascii_lowercase();
                if lower.ends_with(".woff2") {
                    return err(
                        "lower-font-format",
                        format!("`{}` is WOFF2; v1 font sources are TTF or OTF", face.source),
                        face.span,
                    );
                }
                let extension = source
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(str::to_ascii_lowercase);
                if !matches!(extension.as_deref(), Some("ttf" | "otf")) {
                    return err(
                        "lower-font-format",
                        format!("`{}` is not a TTF or OTF source", face.source),
                        face.span,
                    );
                }
                if !exact_plan::is_portable_asset_path(&face.source) {
                    return err(
                        "lower-font-path",
                        format!(
                            "font source `{}` must be a portable local relative path",
                            face.source
                        ),
                        face.span,
                    );
                }
                if source.is_absolute()
                    || source.components().any(|c| {
                        matches!(
                            c,
                            PathComponent::ParentDir
                                | PathComponent::RootDir
                                | PathComponent::Prefix(_)
                        )
                    })
                {
                    return err(
                        "lower-font-path",
                        format!(
                            "font source `{}` must stay under the app directory",
                            face.source
                        ),
                        face.span,
                    );
                }
                if !matches!(
                    source.components().next(),
                    Some(PathComponent::Normal(first)) if first == "assets"
                ) {
                    return err(
                        "lower-font-path",
                        format!(
                            "font source `{}` must be under the app's `assets/` directory",
                            face.source
                        ),
                        face.span,
                    );
                }
                let root = root.as_ref().expect("fonts have an asset root");
                let full = root.join(source);
                let canonical = full.canonicalize().map_err(|e| LowerError {
                    id: "lower-font-unreadable",
                    message: format!("font source `{}` is unreadable: {e}", face.source),
                    span: face.span,
                })?;
                if !canonical.starts_with(root) || std::fs::File::open(&canonical).is_err() {
                    return err(
                        "lower-font-unreadable",
                        format!(
                            "font source `{}` is unreadable under the app directory",
                            face.source
                        ),
                        face.span,
                    );
                }
                // `@font-face`'s `auto` descriptors are the file's own
                // ranges: a static face is normal width at 400.
                let axes = std::fs::read(&canonical)
                    .map(|bytes| crate::font_axes::read(&bytes))
                    .unwrap_or_default();
                // (Unread, it is recorded as no length any file has, so the
                // write check never trusts its faces: Grok's D1 round 3.)
                if face.weight.is_none() {
                    let m = std::fs::metadata(&canonical).ok();
                    self.font_files.push((
                        full.clone(),
                        canonical.clone(),
                        m.as_ref().map_or(u64::MAX, |m| m.len()),
                        m.and_then(|m| m.modified().ok()),
                    ));
                }
                let weight = face.weight.unwrap_or_else(|| {
                    axes.wght.map_or([400, 400], |[low, high]| {
                        [
                            low.round().clamp(1.0, 1000.0) as u16,
                            high.round().clamp(1.0, 1000.0) as u16,
                        ]
                    })
                });
                let stretch = axes.wdth.map_or([1000, 1000], |[low, high]| {
                    let tenths = |p: f32| (p * 10.0).round().clamp(1.0, u16::MAX as f32) as u16;
                    [tenths(low), tenths(high)]
                });
                faces.push(exact_plan::builder::Face {
                    source: face.source.as_str(),
                    weight,
                    stretch,
                    italic: face.italic,
                });
                variable.push(axes.wght.is_some());
            }
            join_pinned_weights(&mut faces, &variable);
            let family = self.b.font_family(&font.name, &faces);
            let stack = self
                .b
                .font_stack(&[(StackMemberKind::Family, Some(family))]);
            self.font_stacks.insert(font.name.clone(), stack);
            self.declared_fonts.insert(
                font.name.clone(),
                DeclaredFont {
                    stack,
                    faces: faces.iter().map(|f| (f.weight[1], f.italic)).collect(),
                },
            );
        }
        Ok(())
    }

    /// The declared families a node's `font-family` can name, for the face
    /// checks ([`font_use`]).
    pub(super) fn font_use(&self, attrs: &[Attr]) -> Result<Vec<FontUse>, LowerError> {
        let uses = font_use(&self.declared_fonts, attrs)?;
        debug_assert!(uses
            .iter()
            .all(|u| self
                .declared_fonts
                .iter()
                .any(|(name, f)| f.stack == u.font.stack
                    && self.font_stacks.get(name) == Some(&f.stack))));
        Ok(uses)
    }

    /// `font-family`'s value with each family name replaced by its stack id,
    /// resolved now. A runtime expression stays a string for the host to
    /// resolve (LLP 1019 D2, 2026-10-11).
    pub(super) fn family_stacks(&mut self, value: &Expr) -> Result<Expr, LowerError> {
        if matches!(value, Expr::Str(s, _) if family_clear(s)) {
            return Ok(value.clone());
        }
        let mut folded = value.clone();
        fold_constant_families(&mut folded);
        let mut bundled = None;
        family_arms(&folded, &mut |name, span| {
            if self
                .declared_fonts
                .keys()
                .any(|declared| declared.eq_ignore_ascii_case(name))
            {
                bundled = Some(span);
            }
        })?;
        if !Self::literal_arms(&folded) {
            if let Some(span) = bundled {
                return err(
                    "lower-font-runtime-bundled",
                    "a runtime `font-family` string may name installed or generic families only; use a literal choice for a bundled font",
                    span,
                );
            }
            return Ok(folded);
        }
        let mut out = folded;
        self.replace_arms(&mut out)?;
        // Beside an empty family's `none`, each stack is `some`, one type.
        if has_none(&out) && !matches!(out, Expr::None(_)) {
            some_arms(&mut out);
        }
        Ok(out)
    }

    pub(super) fn literal_arms(e: &Expr) -> bool {
        match e {
            Expr::Str(..) => true,
            Expr::Ternary(_, yes, no, _) => Self::literal_arms(yes) && Self::literal_arms(no),
            Expr::Match { some, none, .. } => Self::literal_arms(some) && Self::literal_arms(none),
            Expr::Let { body, .. } => Self::literal_arms(body),
            _ => false,
        }
    }

    pub(super) fn literal_family_arms(e: &Expr) -> bool {
        let mut folded = e.clone();
        fold_constant_families(&mut folded);
        Self::literal_arms(&folded)
    }

    /// A literal family's stack, declared as `font-family`'s are: `e`, a
    /// string, becomes the stack's number (stage 6 C's type and family
    /// tokens).
    pub(super) fn replace_family(&mut self, e: &mut Expr) -> Result<(), LowerError> {
        self.replace_arms(e)
    }

    fn replace_arms(&mut self, e: &mut Expr) -> Result<(), LowerError> {
        match e {
            // An empty family declares nothing: the row is unset (the
            // kernel's `StyleValue::text_unsets`), as `none` clears it.
            Expr::Str(name, span) if name.trim().is_empty() => *e = Expr::None(*span),
            Expr::Str(name, span) => {
                // A quoted comma belongs to one family, not a cached list.
                let cache_key = format!("\0{name}");
                let stack = if let Some(stack) = self.font_stacks.get(&cache_key) {
                    *stack
                } else {
                    let names = family_names(name, *span)?;
                    let mut members = Vec::new();
                    let mut single_stack = None;
                    for name in names {
                        let single = if let Some((_, stack)) = self
                            .font_stacks
                            .iter()
                            .find(|(key, _)| key.eq_ignore_ascii_case(&name))
                        {
                            *stack
                        } else {
                            let family = self.b.font_family(name.trim_matches('"'), &[]);
                            let stack = self
                                .b
                                .font_stack(&[(StackMemberKind::Family, Some(family))]);
                            self.font_stacks.insert(name, stack);
                            stack
                        };
                        single_stack = Some(single);
                        let plan = self.b.plan();
                        let member =
                            plan.stack_member(plan.stack(single).members.iter().next().unwrap());
                        members.push((member.kind, member.family));
                    }
                    let stack = if members.len() == 1 {
                        single_stack.unwrap()
                    } else {
                        self.b.font_stack(&members)
                    };
                    self.font_stacks.insert(cache_key, stack);
                    stack
                };
                *e = Expr::Number(stack.0 as f64, *span);
            }
            Expr::Ternary(_, yes, no, _) => {
                self.replace_arms(yes)?;
                self.replace_arms(no)?;
            }
            Expr::Match { some, none, .. } => {
                self.replace_arms(some)?;
                self.replace_arms(none)?;
            }
            Expr::Let { body, .. } => self.replace_arms(body)?,
            _ => unreachable!("family_arms admitted only these"),
        }
        Ok(())
    }
}

/// Whether a replaced literal choice has an empty family's `none` arm.
fn has_none(e: &Expr) -> bool {
    match e {
        Expr::None(_) => true,
        Expr::Ternary(_, yes, no, _) => has_none(yes) || has_none(no),
        Expr::Match { some, none, .. } => has_none(some) || has_none(none),
        Expr::Let { body, .. } => has_none(body),
        _ => false,
    }
}

/// The families of `declared` a node's `font-family` (in `attrs`, its
/// rows as lowered) can name, for the face checks: one for a literal, one
/// per declared arm for a choice, each tuple's for a token. The design
/// server's write check calls this too (`theme::rows`).
pub(crate) fn font_use(
    declared: &BTreeMap<String, DeclaredFont>,
    attrs: &[Attr],
) -> Result<Vec<FontUse>, LowerError> {
    let Some(family) = attrs.iter().find(|a| a.name == "font-family") else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    // A family token's name is no family (stage 6 C): the families of
    // its stack, each tuple's, are what the node uses.
    let mut leaves = Vec::new();
    crate::theme::leaves(&family.value, &mut Vec::new(), &mut leaves);
    let tokens: Vec<Vec<String>> = leaves
        .iter()
        .filter_map(|e| match e {
            Expr::Str(text, _) => crate::theme::measure_values(text).map(|(_, v)| v),
            _ => None,
        })
        .collect();
    if tokens.is_empty() {
        let mut folded = family.value.clone();
        fold_constant_families(&mut folded);
        family_arms(&folded, &mut |name, _| names.push(name.to_string()))?;
    }
    for stack in tokens.iter().flatten() {
        let members = exact_kernel::style::theme::typeface::family_members(stack);
        names.extend(members.into_iter().filter(|(f, _)| *f).map(|(_, m)| m));
    }
    let italic = match attrs.iter().find(|a| a.name == "font-style") {
        None => Some(false),
        Some(a) => match &a.value {
            Expr::Str(s, _) if s == "normal" => Some(false),
            Expr::Str(s, _) if s == "italic" => Some(true),
            Expr::Str(..) => Some(false), // the kernel parser names the invalid value
            _ => None,
        },
    };
    let mut uses = Vec::new();
    for name in names {
        if let Some((_, font)) = declared
            .iter()
            .find(|(declared, _)| declared.eq_ignore_ascii_case(&name))
        {
            uses.push(FontUse {
                font: font.clone(),
                italic,
            });
        }
    }
    Ok(uses)
}

/// `font-family="inherit"` (or `unset`), as a reset writes it: a written
/// clear, which names no family and passes a control's start (LLP 1104 D4).
fn family_clear(s: &str) -> bool {
    ["inherit", "unset"].contains(&s.trim().to_ascii_lowercase().as_str())
}

/// Visit the literal family names for validation, including ones beside
/// runtime arms; the remaining string is resolved when it changes.
fn family_arms(e: &Expr, visit: &mut dyn FnMut(&str, Span)) -> Result<(), LowerError> {
    match e {
        Expr::Str(name, _) if name.trim().is_empty() || family_clear(name) => Ok(()),
        Expr::Str(name, span) => {
            for member in family_names(name, *span)? {
                visit(&member, *span);
            }
            Ok(())
        }
        Expr::Ternary(_, yes, no, _) => {
            family_arms(yes, visit)?;
            family_arms(no, visit)
        }
        Expr::Match { some, none, .. } => {
            family_arms(some, visit)?;
            family_arms(none, visit)
        }
        Expr::Let { body, .. } => family_arms(body, visit),
        _ => Ok(()),
    }
}

/// Each stack number of a replaced literal choice as `some(n)`.
fn some_arms(e: &mut Expr) {
    match e {
        Expr::Number(_, span) => {
            let span = *span;
            *e = Expr::Some(Box::new(e.clone()), span);
        }
        Expr::Ternary(_, yes, no, _) => {
            some_arms(yes);
            some_arms(no);
        }
        Expr::Match { some, none, .. } => {
            some_arms(some);
            some_arms(none);
        }
        Expr::Let { body, .. } => some_arms(body),
        _ => {}
    }
}

fn fold_constant_families(value: &mut Expr) {
    match value {
        Expr::Template(parts, span)
            if parts.iter().all(|part| {
                matches!(
                    part,
                    contract_syntax::TemplatePart::Text(_)
                        | contract_syntax::TemplatePart::Expr(Expr::Str(..))
                )
            }) =>
        {
            let text = parts
                .iter()
                .map(|part| match part {
                    contract_syntax::TemplatePart::Text(text)
                    | contract_syntax::TemplatePart::Expr(Expr::Str(text, _)) => text.as_str(),
                    _ => unreachable!(),
                })
                .collect::<String>();
            *value = Expr::Str(text, *span);
        }
        Expr::Ternary(_, yes, no, _) => {
            fold_constant_families(yes);
            fold_constant_families(no);
        }
        Expr::Match { some, none, .. } => {
            fold_constant_families(some);
            fold_constant_families(none);
        }
        Expr::Let { body, .. } => fold_constant_families(body),
        _ => {}
    }
}

/// CSS family lists, including quoted names containing spaces or commas.
fn family_names(value: &str, span: Span) -> Result<Vec<String>, LowerError> {
    if value.contains('\\') {
        return err(
            "lower-font-family-list",
            "CSS escapes in font-family are not implemented; use the actual family characters",
            span,
        );
    }
    let mut quote = None;
    let mut names = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (i, c) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c == ',' {
            names.push(&value[start..i]);
            start = i + 1;
        }
    }
    names.push(&value[start..]);
    if quote.is_some() || escaped || names.len() > 64 {
        return err(
            "lower-font-family-list",
            "font-family expects at most 64 comma-separated CSS family names with balanced quotes",
            span,
        );
    }
    names
        .into_iter()
        .map(|name| {
            let name = name.trim();
            let quoted = name.starts_with(['\'', '"']);
            let name = if quoted && name.len() >= 2 && name.ends_with(name.chars().next().unwrap())
            {
                &name[1..name.len() - 1]
            } else if name.contains(['\'', '"']) {
                return err(
                    "lower-font-family-list",
                    "a quoted CSS family name must occupy one whole list member",
                    span,
                );
            } else {
                name
            };
            if name.is_empty() || name.contains([';', '{', '}']) {
                return err(
                    "lower-font-family-list",
                    "font-family expects nonempty CSS family names separated by commas",
                    span,
                );
            }
            if !quoted && ["cursive", "fantasy", "math", "emoji", "fangsong"].contains(&name.to_ascii_lowercase().as_str()) {
                return err("lower-font-family-generic", format!("CSS generic family `{name}` has no native mapping in exact2; supported generics are system-ui, ui-sans-serif, sans-serif, ui-serif, serif, ui-monospace, monospace and ui-rounded"), span);
            }
            if !quoted && ["inherit", "initial", "unset", "revert", "revert-layer"].contains(&name.to_ascii_lowercase().as_str()) {
                return err("lower-font-family-wide", format!("CSS-wide font-family value `{name}` is not represented by plan stack ids; `inherit` alone takes the surrounding text's family, or name an explicit family list"), span);
            }
            // The run-time rule (runner `runtime_family_members`) and the browser's.
            if !quoted && !exact_plan::is_css_family_name(name) {
                return err(
                    "lower-font-family-list",
                    format!("unquoted font-family name `{name}` is not a sequence of CSS identifiers; quote it"),
                    span,
                );
            }
            let generic = [
                "system-ui",
                "ui-sans-serif",
                "sans-serif",
                "ui-serif",
                "serif",
                "ui-monospace",
                "monospace",
                "ui-rounded",
            ];
            Ok(
                if quoted && generic.contains(&name.to_ascii_lowercase().as_str()) {
                    format!("\"{name}\"")
                } else if !quoted && name.eq_ignore_ascii_case("-apple-system") {
                    "system-ui".into()
                } else if !quoted && generic.contains(&name.to_ascii_lowercase().as_str()) {
                    name.to_ascii_lowercase()
                } else {
                    name.to_string()
                },
            )
        })
        .collect()
}

/// Several pinned weights of one variable file become one face over their
/// span, so the web declares one `@font-face` and fetches the file once.
/// Every host draws a request inside a range at its own `wght` (LLP 1019
/// D2), so a weight between two of them is drawn as asked rather than at
/// the nearer one; the declared weights draw as before. A span another face
/// of the same style overlaps stays apart, since matching would then choose
/// between them.
fn join_pinned_weights(faces: &mut Vec<exact_plan::builder::Face<'_>>, variable: &[bool]) {
    let pinned =
        |i: usize, f: &exact_plan::builder::Face<'_>| variable[i] && f.weight[0] == f.weight[1];
    let mut keep = vec![true; faces.len()];
    for i in 0..faces.len() {
        if !keep[i] || !pinned(i, &faces[i]) {
            continue;
        }
        let group: Vec<usize> = (i..faces.len())
            .filter(|&j| {
                keep[j]
                    && pinned(j, &faces[j])
                    && faces[j].source == faces[i].source
                    && faces[j].italic == faces[i].italic
            })
            .collect();
        if group.len() < 2 {
            continue;
        }
        let low = group.iter().map(|&j| faces[j].weight[0]).min().unwrap();
        let high = group.iter().map(|&j| faces[j].weight[1]).max().unwrap();
        let overlapped = (0..faces.len()).any(|k| {
            !group.contains(&k)
                && keep[k]
                && faces[k].italic == faces[i].italic
                && faces[k].weight[0] <= high
                && faces[k].weight[1] >= low
        });
        if overlapped {
            continue;
        }
        faces[i].weight = [low, high];
        for &j in &group[1..] {
            keep[j] = false;
        }
    }
    let mut keep = keep.into_iter();
    faces.retain(|_| keep.next().unwrap());
}
