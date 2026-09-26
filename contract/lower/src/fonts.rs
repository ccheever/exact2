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
            if font.name.contains(',') {
                return err(
                    "lower-font-family-list",
                    format!(
                        "font family `{}` contains a comma; v1 stacks are single-member",
                        font.name
                    ),
                    font.span,
                );
            }
            if self.font_stacks.contains_key(&font.name) {
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
            for face in &font.faces {
                if seen.insert((face.weight, face.italic), ()).is_some() {
                    return err(
                        "lower-font-face-duplicate",
                        format!(
                            "font `{}` declares weight {}{} twice",
                            font.name,
                            face.weight,
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
                faces.push((face.source.as_str(), face.weight, face.italic));
            }
            let family = self.b.font_family(&font.name, &faces);
            let stack = self
                .b
                .font_stack(&[(StackMemberKind::Family, Some(family))]);
            self.font_stacks.insert(font.name.clone(), stack);
            self.declared_fonts.insert(
                font.name.clone(),
                DeclaredFont {
                    stack,
                    faces: faces.iter().map(|(_, w, i)| (*w, *i)).collect(),
                },
            );
        }
        Ok(())
    }

    /// The declared families a node's `font-family` can name, for the face
    /// checks: one for a literal, one per declared arm for a choice.
    pub(super) fn font_use(&self, attrs: &[Attr]) -> Result<Vec<FontUse>, LowerError> {
        let Some(family) = attrs.iter().find(|a| a.name == "font-family") else {
            return Ok(Vec::new());
        };
        let mut names = Vec::new();
        self.family_arms(&family.value, &mut |name, _| names.push(name.to_string()))?;
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
            if let Some(font) = self.declared_fonts.get(&name) {
                debug_assert_eq!(Some(&font.stack), self.font_stacks.get(&name));
                uses.push(FontUse {
                    font: font.clone(),
                    italic,
                });
            }
        }
        Ok(uses)
    }

    /// `font-family`'s value with each family name replaced by its stack id,
    /// resolved now. @ref LLP 1053 G7 — a literal, or a choice (`?:`,
    /// `match`) whose every arm is one; a runtime string is refused, since
    /// fonts are declared and resolved at compile time.
    pub(super) fn family_stacks(&self, value: &Expr) -> Result<Expr, LowerError> {
        let mut out = value.clone();
        self.family_arms(value, &mut |_, _| {})?;
        self.replace_arms(&mut out);
        Ok(out)
    }

    fn replace_arms(&self, e: &mut Expr) {
        match e {
            Expr::Str(name, span) => {
                let stack = self.font_stacks[name.as_str()];
                *e = Expr::Number(stack.0 as f64, *span);
            }
            Expr::Ternary(_, yes, no, _) => {
                self.replace_arms(yes);
                self.replace_arms(no);
            }
            Expr::Match { some, none, .. } => {
                self.replace_arms(some);
                self.replace_arms(none);
            }
            Expr::Let { body, .. } => self.replace_arms(body),
            _ => unreachable!("family_arms admitted only these"),
        }
    }

    /// Visit every arm's family name, refusing what is not a known literal.
    fn family_arms(&self, e: &Expr, visit: &mut dyn FnMut(&str, Span)) -> Result<(), LowerError> {
        match e {
            Expr::Str(name, span) => {
                if name.contains(',') {
                    return err(
                        "lower-font-family-list",
                        "v1 font stacks are single-member; a comma list is not supported",
                        *span,
                    );
                }
                if !self.font_stacks.contains_key(name) {
                    return err(
                        "lower-font-undeclared",
                        format!("font family `{name}` is neither generic nor declared"),
                        *span,
                    );
                }
                visit(name, *span);
                Ok(())
            }
            Expr::Ternary(_, yes, no, _) => {
                self.family_arms(yes, visit)?;
                self.family_arms(no, visit)
            }
            Expr::Match { some, none, .. } => {
                self.family_arms(some, visit)?;
                self.family_arms(none, visit)
            }
            Expr::Let { body, .. } => self.family_arms(body, visit),
            other => err(
                "lower-font-family-literal",
                "`font-family` takes a family name, or a choice (`?:` or `match`) whose every arm is one; a family computed at runtime cannot be resolved, because fonts are declared and resolved at compile time",
                other.span(),
            ),
        }
    }
}
