//! TypeScript and JSX, stripped with Oxc.
//!
//! Exact is 4,972 `.ts` and `.tsx` files. Until this existed, none of them
//! could load — the ESM lowering had nothing to lower, because a TypeScript
//! module is not JavaScript.
//!
//! **This is the one transform that cannot preserve the source verbatim.**
//! LLP 0064 §1 values span rewriting because a module's own code reaches the
//! engine exactly as written; erasing types is a rewrite by definition. So
//! TypeScript takes a different path — parse, transform, print — and JavaScript
//! keeps the span-rewriting one. The asymmetry is deliberate: a `.js` module is
//! still byte-identical through the loader.
//!
//! @ref LLP 0028#summary — Oxc is the transform authority
//! @ref LLP 0064#7-the-engines-parser-limits-are-this-transforms-to-route-around — the same argument

use std::path::Path;

use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{TransformOptions, Transformer};

/// Does this specifier name a module that needs type stripping?
pub fn needs_stripping(specifier: &str) -> bool {
    specifier.ends_with(".ts") || specifier.ends_with(".tsx") || specifier.ends_with(".mts")
}

/// The parse configuration for a specifier, by extension.
///
/// From the path rather than from content sniffing: `.ts` and `.js` differ in
/// ways no heuristic settles — most obviously that `<T>x` is a type assertion
/// in one and a JSX element in the other.
pub fn source_type_for(specifier: &str) -> SourceType {
    SourceType::from_path(Path::new(specifier)).unwrap_or_else(|_| SourceType::mjs())
}

/// Strip TypeScript and JSX, returning JavaScript.
///
/// Module syntax is deliberately left alone: `import` and `export` survive for
/// `esm::lower` to handle, because that lowering has properties this one does
/// not — live exports and hoisted imports — and running both would mean two
/// answers to the same question.
pub fn strip(source: &str, specifier: &str) -> Result<String, String> {
    let path = Path::new(specifier);
    let source_type = source_type_for(specifier);
    let allocator = Allocator::default();

    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(parsed
            .diagnostics
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }
    let mut program = parsed.program;

    // `with_enum_eval` is required by the transformer to lower a TypeScript
    // enum, which unlike everything else here is NOT erasable — an enum emits a
    // runtime object. Without it the transformer panics rather than producing
    // wrong output, which is the right way round.
    let scoping = SemanticBuilder::new()
        .with_excess_capacity(2.0)
        .with_enum_eval(true)
        .build(&program)
        .semantic
        .into_scoping();

    let options = TransformOptions::default();
    let result =
        Transformer::new(&allocator, path, &options).build_with_scoping(scoping, &mut program);
    if !result.diagnostics.is_empty() {
        return Err(result
            .diagnostics
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }

    Ok(Codegen::new()
        .with_options(CodegenOptions::default())
        .build(&program)
        .code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_decide_what_needs_stripping() {
        assert!(needs_stripping("./a.ts"));
        assert!(needs_stripping("./a.tsx"));
        assert!(!needs_stripping("./a.js"));
        assert!(!needs_stripping("./a.mjs"));
    }

    #[test]
    fn type_annotations_are_erased() {
        let out = strip(
            "const n: number = 1;\nfunction f(a: string): void {}",
            "./a.ts",
        )
        .unwrap();
        assert!(!out.contains(": number"), "{out}");
        assert!(!out.contains(": string"), "{out}");
        assert!(!out.contains(": void"), "{out}");
        assert!(out.contains("const n = 1"), "{out}");
    }

    #[test]
    fn type_only_declarations_disappear_entirely() {
        let out = strip(
            "interface Shape { x: number }\ntype Alias = string;\nexport const v = 1;",
            "./a.ts",
        )
        .unwrap();
        assert!(!out.contains("interface"), "{out}");
        assert!(!out.contains("type Alias"), "{out}");
        assert!(
            out.contains("export const v = 1"),
            "module syntax survives: {out}"
        );
    }

    /// Module syntax must survive for `esm::lower`, or the two transforms would
    /// both be answering the same question.
    #[test]
    fn module_syntax_is_left_for_the_esm_lowering() {
        let out = strip(
            "import type { T } from './t';\nimport { real } from './r';\nexport const x: number = real;",
            "./a.ts",
        )
        .unwrap();
        assert!(out.contains("import"), "{out}");
        assert!(out.contains("export"), "{out}");
        // A type-only import is erased; a value import is not.
        assert!(!out.contains("./t"), "type-only import survived: {out}");
        assert!(out.contains("./r"), "{out}");
    }

    #[test]
    fn generics_and_assertions_are_erased() {
        let out = strip(
            "function id<T>(x: T): T { return x; }\nconst y = id<string>('a') as string;",
            "./a.ts",
        )
        .unwrap();
        assert!(!out.contains("<T>"), "{out}");
        assert!(!out.contains(" as string"), "{out}");
        assert!(out.contains("function id(x)"), "{out}");
    }

    #[test]
    fn jsx_becomes_calls() {
        let out = strip("const el = <div className=\"a\">hi</div>;", "./a.tsx").unwrap();
        assert!(!out.contains("<div"), "JSX survived: {out}");
        assert!(
            out.contains("createElement") || out.contains("jsx"),
            "{out}"
        );
    }

    /// An enum emits code — it is not erasable — so it must still work.
    #[test]
    fn enums_still_produce_a_runtime_value() {
        let out = strip("export enum Color { Red, Blue }", "./a.ts").unwrap();
        assert!(out.contains("Color"), "{out}");
        assert!(out.contains("Red"), "{out}");
    }

    #[test]
    fn javascript_passes_through_the_parser_unharmed() {
        let out = strip("export const a = 1;\nconst f = (x) => x * 2;", "./a.js").unwrap();
        assert!(out.contains("export const a = 1"), "{out}");
        assert!(out.contains("=>"), "{out}");
    }

    #[test]
    fn a_type_error_free_syntax_error_is_reported() {
        assert!(strip("const x: = ;", "./a.ts").is_err());
    }
}
