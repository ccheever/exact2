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

    pub(super) fn font_use(&self, attrs: &[Attr]) -> Result<Option<FontUse>, LowerError> {
        let Some(family) = attrs.iter().find(|a| a.name == "font-family") else {
            return Ok(None);
        };
        let Expr::Str(name, _) = &family.value else {
            return err(
                "lower-font-family-literal",
                "`font-family` is literal-only in v1",
                family.span,
            );
        };
        if name.contains(',') {
            return err(
                "lower-font-family-list",
                "v1 font stacks are single-member; a comma list is not supported",
                family.span,
            );
        }
        let Some(stack) = self.font_stacks.get(name) else {
            return err(
                "lower-font-undeclared",
                format!("font family `{name}` is neither generic nor declared"),
                family.span,
            );
        };
        let Some(font) = self.declared_fonts.get(name) else {
            return Ok(None);
        };
        debug_assert_eq!(font.stack, *stack);
        let italic = match attrs.iter().find(|a| a.name == "font-style") {
            None => Some(false),
            Some(a) => match &a.value {
                Expr::Str(s, _) if s == "normal" => Some(false),
                Expr::Str(s, _) if s == "italic" => Some(true),
                Expr::Str(..) => Some(false), // the kernel parser names the invalid value
                _ => None,
            },
        };
        Ok(Some(FontUse {
            font: font.clone(),
            italic,
        }))
    }
}
