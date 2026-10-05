//! `@color-profile` declarations (LLP 1100 D3).

use crate::{LowerError, Lowerer};
use contract_syntax::{Expr, File};

impl Lowerer<'_> {
    pub(crate) fn declare_color_profiles(
        &mut self,
        file: &File,
    ) -> exact_kernel::style::profiled::DeclarationScope {
        let mut declarations = Vec::new();
        for p in &file.color_profiles {
            let mut src = None;
            let mut intent = "relative-colorimetric".to_string();
            for a in &p.attrs {
                match (a.name.as_str(), &a.value) {
                    ("src", Expr::Str(path, _)) if !path.is_empty() => src = Some(path.clone()),
                    ("rendering-intent", Expr::Str(i, _))
                        if ["relative-colorimetric", "absolute-colorimetric", "perceptual", "saturation"].contains(&i.as_str()) =>
                    {
                        intent = i.clone()
                    }
                    (name, _) => self.errors.push(LowerError {
                        id: "lower-color-profile",
                        message: format!(
                            "`color-profile {}`: `{name}` is not a descriptor here; a profile takes `src` (an ICC file in the app's assets) \
                             and `rendering-intent` (relative-colorimetric, absolute-colorimetric, perceptual or saturation)",
                            p.name
                        ),
                        span: a.span,
                    }),
                }
            }
            match src {
                Some(src) => {
                    // Known to the kernel's parser before any row that names it.
                    declarations.push((p.name.clone(), src.clone(), intent.clone()));
                    self.b.color_profile(&p.name, &src, &intent);
                }
                None => self.errors.push(LowerError {
                    id: "lower-color-profile",
                    message: format!(
                        "`color-profile {}` needs `src`, the ICC file: `src=\"assets/brand.icc\"`",
                        p.name
                    ),
                    span: p.span,
                }),
            }
        }
        exact_kernel::style::profiled::declarations(
            declarations.iter().map(|(n, s, i)| (&**n, &**s, &**i)),
        )
    }
}
