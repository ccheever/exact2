//! ESM lowering, with Oxc as the parser.
//!
//! LLP 0057 §5.2 records that Ibex 2 targets Exact, and Exact is 4,350 ES
//! modules against 79 CommonJS ones — so the loader has to accept `import` and
//! `export` or it cannot load a single one.
//!
//! **Oxc parses; this file splices.** LLP 0028 makes Oxc the transform
//! authority, so the parser is not a choice to re-make. What is a choice is
//! rewriting spans rather than rebuilding an AST: the parser gives exact byte
//! ranges for each import and export, everything outside them is copied
//! verbatim, and a module's own code reaches the engine unchanged. That keeps
//! the transform auditable and leaves source positions recoverable.
//!
//! `hermesc` cannot do this job: its `-commonjs` mode is the one that accepts
//! module syntax, and on the pinned engine it segfaults on every input.
//!
//! **What this does not preserve is specified in LLP 0064 §3**, and every
//! remaining divergence is silent — named imports snapshot, and cycles behave
//! as CommonJS cycles rather than raising a ReferenceError.
//!
//! @ref LLP 0064#3-what-is-not-preserved — the divergences, measured
//! @ref LLP 0028#summary — Oxc is the transform authority
//! @ref LLP 0057#52-what-ibex-2-is-for — why ESM is required at all

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, ImportDeclarationSpecifier, ModuleDeclaration,
    Statement,
};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

/// One rewritten span: everything else in the source is copied untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Splice {
    start: usize,
    end: usize,
    replacement: String,
    /// Imports move to the top of the module body rather than staying where
    /// they were written.
    ///
    /// ES modules evaluate every dependency before the importing module's own
    /// code, and a binding is available above its `import` statement. Leaving a
    /// `require` where the import was written reproduces neither: a module that
    /// used a binding before importing it silently saw `undefined`. Hoisting is
    /// strictly more faithful, not a convenience.
    hoisted: bool,
}

/// Does this source use ES module syntax?
///
/// Answered by the parser, not by scanning for the word `import`: a string
/// containing "export" is not an export, and `import()` is a CommonJS-legal
/// dynamic expression.
pub fn is_module(source: &str) -> bool {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    parsed.program.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::ImportDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportAllDeclaration(_)
        )
    })
}

/// `import.meta` and `import(...)` occurrences, which live inside expressions
/// rather than at the top level.
///
/// Hermes parses neither — it reports `'import.meta' is currently unsupported`
/// and `Invalid expression` for a dynamic import. Oxc parses both, and the
/// transform already stands between them, so the engine's parser limits are
/// this file's to route around rather than the application's to work around.
#[derive(Debug, Clone)]
struct DynamicImport {
    start: usize,
    end: usize,
    args_start: usize,
    args_end: usize,
    literal: Option<String>,
}

#[derive(Default)]
struct ExpressionForms {
    /// Spans of `import.meta`.
    meta: Vec<(usize, usize)>,
    /// Each `import(...)`: the whole expression's range, the range of its
    /// arguments — from the specifier's first byte to the last argument's
    /// end, by the parser's spans and not by searching for `(` — and the
    /// literal specifier when there is one.
    dynamic_imports: Vec<DynamicImport>,
}

impl<'a> Visit<'a> for ExpressionForms {
    fn visit_meta_property(&mut self, it: &oxc_ast::ast::MetaProperty<'a>) {
        if it.meta.name == "import" && it.property.name == "meta" {
            self.meta
                .push((it.span.start as usize, it.span.end as usize));
        }
    }

    fn visit_import_expression(&mut self, it: &oxc_ast::ast::ImportExpression<'a>) {
        // A literal specifier is a static dependency the build can compile
        // ahead of time. A computed one is an expression, and no build can
        // resolve it — see LLP 0064 §7.
        let literal = match &it.source {
            oxc_ast::ast::Expression::StringLiteral(s) => Some(s.value.to_string()),
            _ => None,
        };
        // The argument range comes from the parser. Searching the span for
        // `(` found the one in `import /*(*/('./x')` first and handed Hermes
        // an `import` it cannot parse.
        let args_end = it
            .options
            .as_ref()
            .map(|options| options.span().end)
            .unwrap_or(it.source.span().end) as usize;
        self.dynamic_imports.push(DynamicImport {
            start: it.span.start as usize,
            end: it.span.end as usize,
            args_start: it.source.span().start as usize,
            args_end,
            literal,
        });
        // Keep walking: the argument may itself contain another import().
        oxc_ast_visit::walk::walk_import_expression(self, it);
    }
}

fn expression_forms(program: &oxc_ast::ast::Program) -> ExpressionForms {
    let mut forms = ExpressionForms::default();
    forms.visit_program(program);
    forms
}

/// Every static dependency a module declares: imports and re-exports alike.
///
/// From the parser, not a scan. `export { a } from './x'` and
/// `export * from './x'` are dependencies as much as `import` is, and a text
/// scan for the word "import" finds them in strings and comments while missing
/// these two entirely.
///
/// Dynamic `import()` is NOT here — see `dynamic_dependencies`, which is
/// separate because a dynamic import is conditional where a static one is not.
pub fn dependencies(source: &str, specifier: &str) -> Vec<String> {
    let allocator = Allocator::default();
    // The specifier decides how to parse. Reading a TypeScript module as
    // JavaScript fails, and this function ignores diagnostics — so it would
    // return NO dependencies rather than an error, and the build would quietly
    // compile one module and fail at run time. That is exactly what happened.
    let source_type = crate::typescript::source_type_for(specifier);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let mut found = Vec::new();
    for statement in &parsed.program.body {
        let Some(module) = as_module_declaration(statement) else {
            continue;
        };
        match module {
            ModuleDeclaration::ImportDeclaration(d) => found.push(d.source.value.to_string()),
            ModuleDeclaration::ExportAllDeclaration(d) => found.push(d.source.value.to_string()),
            ModuleDeclaration::ExportNamedDeclaration(d) => {
                if let Some(source_module) = &d.source {
                    found.push(source_module.value.to_string());
                }
            }
            _ => {}
        }
    }
    found
}

/// Literal specifiers of dynamic `import()` calls.
///
/// Separate from `dependencies` because they are CONDITIONAL. A static import
/// that does not resolve is a bug and should fail a build; a dynamic one may
/// legitimately reference something absent — the call rejects, and code that
/// guards it is correct. Failing the build on it would refuse a valid program.
///
/// Computed specifiers are not here: no build can resolve an expression.
pub fn dynamic_dependencies(source: &str, specifier: &str) -> Vec<String> {
    let allocator = Allocator::default();
    let source_type = crate::typescript::source_type_for(specifier);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    expression_forms(&parsed.program)
        .dynamic_imports
        .into_iter()
        .filter_map(|import| import.literal)
        .collect()
}

/// Exported bindings this module both declares mutable and assigns to.
///
/// These are the only bindings LLP 0064 §3.1 can get wrong: an importer writing
/// `import { n }` snapshots, so a later reassignment is invisible to it. The
/// divergence is silent, and this makes it loud — a build can name the module
/// and the binding and point at `import * as`, which reads through and is live.
///
/// Measured on Exact: 3 `export let` against 12,677 immutable exports. That is
/// why this reports rather than rewrites — the fix that would close §3.1
/// rewrites every usage site in the module, and the payoff is three
/// declarations.
///
/// Approximate on purpose. A shadowed inner `n` counts as an assignment, so
/// this can warn where the export is in fact never reassigned. For a warning
/// that is the right direction to err, and a precise answer needs the scope
/// analysis §5 defers.
pub fn mutable_exports(source: &str, specifier: &str) -> Vec<String> {
    let allocator = Allocator::default();
    let source_type = crate::typescript::source_type_for(specifier);
    let parsed = Parser::new(&allocator, source, source_type).parse();

    let mut exported_mutable: Vec<String> = Vec::new();
    for statement in &parsed.program.body {
        let Some(ModuleDeclaration::ExportNamedDeclaration(declaration)) =
            as_module_declaration(statement)
        else {
            continue;
        };
        let Some(Declaration::VariableDeclaration(variable)) = &declaration.declaration else {
            continue;
        };
        if variable.kind.is_const() {
            continue;
        }
        for declarator in &variable.declarations {
            collect_pattern_names(&declarator.id, &mut exported_mutable);
        }
    }
    if exported_mutable.is_empty() {
        return Vec::new();
    }

    // Assignment is detected textually over the source, which is enough to
    // separate "declared mutable" from "actually reassigned" without a full
    // semantic pass.
    exported_mutable
        .into_iter()
        .filter(|name| assigns_to(source, name))
        .collect()
}

fn assigns_to(source: &str, name: &str) -> bool {
    let bytes = source.as_bytes();
    let mut from = 0usize;
    while let Some(at) = source[from..].find(name) {
        let start = from + at;
        let end = start + name.len();
        from = end;
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after = source[end..].trim_start();
        if !before_ok || end < bytes.len() && is_ident_byte(bytes[end]) {
            continue;
        }
        // `n = `, `n += `, `n++`, `n--` — but not `n ==` or `n =>`.
        let assignment =
            (after.starts_with('=') && !after.starts_with("==") && !after.starts_with("=>"))
                || after.starts_with("+=")
                || after.starts_with("-=")
                || after.starts_with("*=")
                || after.starts_with("/=")
                || after.starts_with("++")
                || after.starts_with("--");
        if assignment {
            // A declaration is not a reassignment.
            let preceding = source[..start].trim_end();
            let declares = preceding.ends_with("let")
                || preceding.ends_with("var")
                || preceding.ends_with("const");
            if !declares {
                return true;
            }
        }
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// An expression-level module form. Lowered wherever the text containing it
/// is copied, never as a top-level splice — see `render`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionForm {
    /// `import.meta`, injected per module so `import.meta.url` is this
    /// module's URL while the wrapper text stays identical across modules —
    /// which is what keeps one artifact per distinct source.
    Meta,
    /// `import(...)`, lowered to the module's OWN require so the specifier
    /// resolves relative to this module and the imported module's grants are
    /// looked up under its own resolved name. The argument expression is kept
    /// exactly as written, computed specifiers included; its range is the
    /// parser's.
    DynamicImport { args_start: usize, args_end: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpressionSite {
    start: usize,
    end: usize,
    form: ExpressionForm,
}

/// Copy `source[start..end]`, lowering every expression form inside it.
///
/// Recursive, because the forms nest: `import(new URL('./z', import.meta.url))`
/// is a dynamic import whose argument contains `import.meta`. `sites` is
/// sorted by start, so an outer form is reached before anything inside it,
/// and the inner forms are rendered by the outer form's own recursion — the
/// `site.start < cursor` skip is what keeps this loop from rendering them a
/// second time.
fn render(source: &str, start: usize, end: usize, sites: &[ExpressionSite]) -> String {
    let mut out = String::with_capacity(end - start + 32);
    let mut cursor = start;
    for site in sites {
        if site.start < start || site.end > end || site.start < cursor {
            continue;
        }
        out.push_str(&source[cursor..site.start]);
        match site.form {
            ExpressionForm::Meta => out.push_str("__ibex2_meta"),
            ExpressionForm::DynamicImport {
                args_start,
                args_end,
            } => {
                out.push_str("__ibex2_dynamic_import(require, ");
                out.push_str(&render(source, args_start, args_end, sites));
                out.push(')');
            }
        }
        cursor = site.end;
    }
    out.push_str(&source[cursor..end]);
    out
}

/// Lower ES module syntax to the CommonJS-shaped factory body the loader runs.
///
/// Returns the source unchanged when it uses no module syntax, so a CommonJS
/// module costs a parse and nothing else.
pub fn lower(source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(parsed
            .diagnostics
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }

    let mut splices: Vec<Splice> = Vec::new();
    let mut saw_module_syntax = false;

    // `import.meta` and `import(...)` are expressions, so they come from a
    // visitor rather than the statement walk below. Neither parses in Hermes;
    // both parse in Oxc, and lowering them here is what makes that difference
    // invisible to application code.
    //
    // They are not splices of their own. An expression lives INSIDE something
    // — an exported function's body, a default export's initializer, another
    // dynamic import's argument — and a module declaration's rewrite copies
    // its span verbatim. As top-level splices they collided with the
    // declaration containing them, and every
    // `export async function load() { return import('./x') }` in Exact was
    // refused as "overlapping module declarations". So every copy of source
    // text goes through `render`, which lowers the expression sites the copied
    // range contains.
    let forms = expression_forms(&parsed.program);
    let mut sites: Vec<ExpressionSite> = forms
        .meta
        .iter()
        .map(|(start, end)| ExpressionSite {
            start: *start,
            end: *end,
            form: ExpressionForm::Meta,
        })
        .chain(forms.dynamic_imports.iter().map(|import| ExpressionSite {
            start: import.start,
            end: import.end,
            form: ExpressionForm::DynamicImport {
                args_start: import.args_start,
                args_end: import.args_end,
            },
        }))
        .collect();
    sites.sort_by_key(|site| site.start);
    if !sites.is_empty() {
        saw_module_syntax = true;
    }

    for statement in &parsed.program.body {
        let Some(module) = as_module_declaration(statement) else {
            continue;
        };
        saw_module_syntax = true;
        match module {
            ModuleDeclaration::ImportDeclaration(declaration) => {
                let from = declaration.source.value.as_str();
                let mut parts: Vec<String> = Vec::new();
                match &declaration.specifiers {
                    // `import './side-effect.js'` — evaluate, bind nothing.
                    None => parts.push(format!("require({from:?});")),
                    Some(specifiers) => {
                        let mut named: Vec<String> = Vec::new();
                        for specifier in specifiers {
                            match specifier {
                                ImportDeclarationSpecifier::ImportSpecifier(s) => named.push({
                                    let imported = s.imported.name();
                                    let local = s.local.name.as_str();
                                    if imported == local {
                                        local.to_string()
                                    } else {
                                        format!("{imported}: {local}")
                                    }
                                }),
                                ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                                    // Interop: a CommonJS module has no
                                    // `default`, and treating its
                                    // `module.exports` as one is what every
                                    // bundler settled on.
                                    parts.push(format!(
                                        "const {} = __ibex2_default(require({from:?}));",
                                        s.local.name
                                    ));
                                }
                                ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                                    parts.push(format!(
                                        "const {} = require({from:?});",
                                        s.local.name
                                    ));
                                }
                            }
                        }
                        if !named.is_empty() {
                            parts.push(format!(
                                "const {{ {} }} = require({from:?});",
                                named.join(", ")
                            ));
                        }
                        if parts.is_empty() {
                            parts.push(format!("require({from:?});"));
                        }
                    }
                }
                splices.push(Splice {
                    start: declaration.span.start as usize,
                    end: declaration.span.end as usize,
                    replacement: parts.join(" "),
                    hoisted: true,
                });
            }

            ModuleDeclaration::ExportNamedDeclaration(declaration) => {
                let mut parts: Vec<String> = Vec::new();
                if let Some(inner) = &declaration.declaration {
                    // `export const x = 1` / `export function f() {}` — keep the
                    // declaration exactly as written and publish it after.
                    parts.push(render(
                        source,
                        inner.span().start as usize,
                        inner.span().end as usize,
                        &sites,
                    ));
                    for name in declared_names(inner) {
                        parts.push(live_export(&name, &name));
                    }
                } else {
                    for specifier in &declaration.specifiers {
                        let local = specifier.local.name();
                        let exported = specifier.exported.name();
                        match &declaration.source {
                            // `export { a } from './x'` — a re-export never
                            // binds locally, so it reads through each time.
                            Some(source_module) => parts.push(format!(
                                "Object.defineProperty(exports, {:?}, {{ get: () => require({:?})[{:?}], enumerable: true, configurable: true }});",
                                exported.as_ref(),
                                source_module.value.as_str(),
                                local.as_ref()
                            )),
                            None => parts.push(live_export(&exported, &local)),
                        }
                    }
                }
                splices.push(Splice {
                    start: declaration.span.start as usize,
                    end: declaration.span.end as usize,
                    replacement: parts.join(" "),
                    hoisted: false,
                });
            }

            ModuleDeclaration::ExportDefaultDeclaration(declaration) => {
                let replacement = match &declaration.declaration {
                    // A named function or class keeps its name, so recursion and
                    // self-reference still work.
                    ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                        let text =
                            render(source, f.span.start as usize, f.span.end as usize, &sites);
                        match &f.id {
                            Some(id) => format!("{text} exports.default = {};", id.name),
                            None => format!("exports.default = {text};"),
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(c) => {
                        let text =
                            render(source, c.span.start as usize, c.span.end as usize, &sites);
                        match &c.id {
                            Some(id) => format!("{text} exports.default = {};", id.name),
                            None => format!("exports.default = {text};"),
                        }
                    }
                    other => {
                        let span = other.span();
                        let text = render(source, span.start as usize, span.end as usize, &sites);
                        format!("exports.default = {text};")
                    }
                };
                splices.push(Splice {
                    start: declaration.span.start as usize,
                    end: declaration.span.end as usize,
                    replacement,
                    hoisted: false,
                });
            }

            ModuleDeclaration::ExportAllDeclaration(declaration) => {
                let from = declaration.source.value.as_str();
                let replacement = match &declaration.exported {
                    // `export * as ns from './x'`
                    Some(name) => format!(
                        "Object.defineProperty(exports, {:?}, {{ get: () => require({from:?}), enumerable: true, configurable: true }});",
                        name.name().as_ref()
                    ),
                    // `export * from './x'` — `default` is deliberately not
                    // re-exported, per the spec.
                    None => format!("__ibex2_export_all(exports, require({from:?}));"),
                };
                splices.push(Splice {
                    start: declaration.span.start as usize,
                    end: declaration.span.end as usize,
                    replacement,
                    hoisted: false,
                });
            }

            // TypeScript-only module forms; this runtime does not accept them.
            _ => {}
        }
    }

    if !saw_module_syntax {
        return Ok(source.to_string());
    }

    splices.sort_by_key(|splice| splice.start);

    let mut out = String::with_capacity(source.len() + 256);
    // ES module code is strict by definition, and the factory it becomes is
    // a plain function — which is sloppy unless told otherwise. Without this
    // an undeclared assignment created a global, `010` was eight, and
    // `arguments.callee` worked; and the frozen-global failure of LLP 0062 §3
    // surfaced three modules late instead of as the TypeError strict code
    // would have thrown at the write. CommonJS stays sloppy, as it is in Node.
    out.push_str("\"use strict\";\n");
    // Imports first, in source order: every dependency is evaluated before the
    // importing module's own code, as ES modules require.
    for splice in splices.iter().filter(|splice| splice.hoisted) {
        out.push_str(&splice.replacement);
        out.push('\n');
    }

    let mut cursor = 0usize;
    for splice in &splices {
        if splice.start < cursor {
            return Err("overlapping module declarations".into());
        }
        out.push_str(&render(source, cursor, splice.start, &sites));
        // A hoisted import leaves nothing behind, so line structure is
        // preserved for anything that later maps positions.
        if !splice.hoisted {
            out.push_str(&splice.replacement);
        }
        cursor = splice.end;
    }
    out.push_str(&render(source, cursor, source.len(), &sites));
    Ok(out)
}

/// Publish an export as a live view of its binding.
///
/// `exports.x = x` would snapshot, and ESM exports are live: a module that
/// reassigns an exported `let` after evaluation must have that visible to its
/// importers. A getter is the cheapest way to keep that true, and it is what
/// makes an import cycle see values defined after it read the module.
fn live_export(exported: &str, local: &str) -> String {
    format!(
        "Object.defineProperty(exports, {exported:?}, {{ get: () => {local}, enumerable: true, configurable: true }});"
    )
}

fn as_module_declaration<'a>(statement: &'a Statement<'a>) -> Option<&'a ModuleDeclaration<'a>> {
    statement.as_module_declaration()
}

/// The names a declaration introduces, so each can be published.
fn declared_names(declaration: &Declaration) -> Vec<String> {
    let mut names = Vec::new();
    match declaration {
        Declaration::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                collect_pattern_names(&declarator.id, &mut names);
            }
        }
        Declaration::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                names.push(id.name.to_string());
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                names.push(id.name.to_string());
            }
        }
        _ => {}
    }
    names
}

/// Destructuring exports every name it binds, not just simple identifiers:
/// `export const { a, b } = obj` publishes both.
fn collect_pattern_names(pattern: &oxc_ast::ast::BindingPattern, names: &mut Vec<String>) {
    use oxc_ast::ast::BindingPattern;
    match pattern {
        BindingPattern::BindingIdentifier(id) => names.push(id.name.to_string()),
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_pattern_names(&property.value, names);
            }
            if let Some(rest) = &object.rest {
                collect_pattern_names(&rest.argument, names);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_pattern_names(element, names);
            }
            if let Some(rest) = &array.rest {
                collect_pattern_names(&rest.argument, names);
            }
        }
        BindingPattern::AssignmentPattern(assignment) => {
            collect_pattern_names(&assignment.left, names)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The specifier decides the parse; these exercise the JavaScript path.
    fn dependencies_js(source: &str) -> Vec<String> {
        dependencies(source, "./m.js")
    }
    fn dynamic_dependencies_js(source: &str) -> Vec<String> {
        dynamic_dependencies(source, "./m.js")
    }
    fn mutable_exports_js(source: &str) -> Vec<String> {
        mutable_exports(source, "./m.js")
    }

    fn lowered(source: &str) -> String {
        lower(source).expect("lowers")
    }

    #[test]
    fn commonjs_passes_through_untouched() {
        let source = "const a = require('./a');\nmodule.exports = a;\n";
        assert_eq!(lowered(source), source);
        assert!(!is_module(source));
    }

    /// `import()` is a call expression, legal in CommonJS, and the word
    /// "export" inside a string is not an export. The parser knows; a scan
    /// would not.
    #[test]
    fn module_detection_is_syntactic_not_textual() {
        assert!(!is_module("const p = import('./x');"));
        assert!(!is_module("const s = 'export default nope';"));
        assert!(!is_module("// import { a } from './b'"));
        assert!(is_module("import { a } from './b';"));
        assert!(is_module("export const x = 1;"));
    }

    #[test]
    fn named_imports_become_a_destructure() {
        let out = lowered("import { a, b as c } from './x';\nuse(a, c);\n");
        assert!(
            out.contains(r#"const { a, b: c } = require("./x");"#),
            "{out}"
        );
        assert!(
            out.contains("use(a, c);"),
            "the module's own code is untouched"
        );
    }

    #[test]
    fn a_default_import_goes_through_interop() {
        let out = lowered("import d from './x';\n");
        assert!(
            out.contains(r#"const d = __ibex2_default(require("./x"));"#),
            "{out}"
        );
    }

    #[test]
    fn a_namespace_import_is_the_module_object() {
        let out = lowered("import * as ns from './x';\n");
        assert!(out.contains(r#"const ns = require("./x");"#), "{out}");
    }

    #[test]
    fn a_side_effect_import_binds_nothing() {
        let out = lowered("import './x';\n");
        assert_eq!(out.trim(), "\"use strict\";\nrequire(\"./x\");");
    }

    #[test]
    fn mixed_default_and_named_imports_both_bind() {
        let out = lowered("import d, { a } from './x';\n");
        assert!(out.contains("__ibex2_default(require(\"./x\"))"), "{out}");
        assert!(out.contains(r#"const { a } = require("./x");"#), "{out}");
    }

    /// Exports are LIVE. `exports.x = x` would snapshot, and a module that
    /// reassigns an exported binding after evaluation must have that visible.
    #[test]
    fn exports_are_live_views_not_snapshots() {
        let out = lowered("export let counter = 0;\n");
        assert!(
            out.contains("let counter = 0;"),
            "the declaration survives: {out}"
        );
        assert!(out.contains("get: () => counter"), "not a live view: {out}");
        assert!(
            !out.contains("exports.counter = counter;"),
            "snapshotted: {out}"
        );
    }

    #[test]
    fn exported_functions_and_classes_keep_their_declarations() {
        let out = lowered("export function f() { return f; }\n");
        assert!(out.contains("function f() { return f; }"), "{out}");
        assert!(out.contains("get: () => f"), "{out}");

        let out = lowered("export class C {}\n");
        assert!(out.contains("class C {}"), "{out}");
        assert!(out.contains("get: () => C"), "{out}");
    }

    /// A default-exported function keeps its name, so recursion still works.
    #[test]
    fn a_named_default_export_keeps_its_binding() {
        let out =
            lowered("export default function fact(n) { return n < 2 ? 1 : n * fact(n - 1); }\n");
        assert!(out.contains("function fact(n)"), "{out}");
        assert!(out.contains("exports.default = fact;"), "{out}");
    }

    #[test]
    fn an_anonymous_default_export_is_assigned_directly() {
        let out = lowered("export default 42;\n");
        assert_eq!(out.trim(), "\"use strict\";\nexports.default = 42;");
        let out = lowered("export default { a: 1 };\n");
        assert!(out.contains("exports.default = { a: 1 };"), "{out}");
    }

    #[test]
    fn an_export_list_publishes_each_name() {
        let out = lowered("const a = 1, b = 2;\nexport { a, b as bee };\n");
        assert!(out.contains("get: () => a"), "{out}");
        assert!(
            out.contains(r#"Object.defineProperty(exports, "bee", { get: () => b"#),
            "{out}"
        );
    }

    #[test]
    fn a_re_export_reads_through_to_its_source() {
        let out = lowered("export { a } from './x';\n");
        assert!(out.contains(r#"require("./x")["a"]"#), "{out}");
        assert!(
            !out.contains("const"),
            "a re-export must not bind locally: {out}"
        );
    }

    #[test]
    fn export_star_uses_the_helper_and_export_star_as_does_not() {
        let out = lowered("export * from './x';\n");
        assert!(
            out.contains(r#"__ibex2_export_all(exports, require("./x"));"#),
            "{out}"
        );

        let out = lowered("export * as ns from './x';\n");
        assert!(
            out.contains(r#"Object.defineProperty(exports, "ns""#),
            "{out}"
        );
    }

    #[test]
    fn destructured_exports_publish_every_bound_name() {
        let out = lowered("export const { a, b: c } = obj;\n");
        assert!(out.contains("get: () => a"), "{out}");
        assert!(out.contains("get: () => c"), "{out}");
        let out = lowered("export const [x, ...rest] = arr;\n");
        assert!(out.contains("get: () => x"), "{out}");
        assert!(out.contains("get: () => rest"), "{out}");
    }

    #[test]
    fn several_declarations_all_get_rewritten_in_order() {
        let out = lowered(
            "import { a } from './a';\nimport { b } from './b';\nexport const c = a + b;\n",
        );
        let a_at = out.find("./a").expect("a");
        let b_at = out.find("./b").expect("b");
        assert!(a_at < b_at, "order not preserved: {out}");
        assert!(out.contains("const c = a + b;"), "{out}");
    }

    #[test]
    fn dependencies_include_imports_and_re_exports() {
        let deps = dependencies_js(
            "import a from './a.js';\n\
             import './side.js';\n\
             export { b } from './b.js';\n\
             export * from './c.js';\n\
             const d = require('./d.js');\n\
             import('./lazy.js');\n",
        );
        assert_eq!(
            deps,
            vec!["./a.js", "./side.js", "./b.js", "./c.js"],
            "a dynamic import is conditional and belongs in dynamic_dependencies"
        );
        assert_eq!(
            dynamic_dependencies_js("import('./lazy.js'); import('./' + x);"),
            vec!["./lazy.js"],
            "only literal specifiers; a computed one is unresolvable"
        );
    }

    /// A dynamic import's specifier is an expression, so the build cannot
    /// resolve it and must not claim to.
    #[test]
    fn dynamic_import_is_not_a_static_dependency() {
        assert!(dependencies_js("const m = import('./' + name);").is_empty());
        assert!(dependencies_js("const s = 'import x from y';").is_empty());
    }

    #[test]
    fn mutable_exports_finds_only_bindings_that_are_reassigned() {
        // Declared mutable AND reassigned — the case §3.1 gets wrong.
        assert_eq!(
            mutable_exports_js("export let n = 0;\nexport function bump() { n += 1; }"),
            vec!["n"]
        );
        assert_eq!(mutable_exports_js("export let n = 0;\nn = 5;"), vec!["n"]);

        // Declared mutable but never reassigned: indistinguishable from const
        // to an importer, so not worth a warning.
        assert!(mutable_exports_js("export let n = 0;\nconsole.log(n);").is_empty());

        // const cannot be reassigned at all.
        assert!(mutable_exports_js("export const n = 0;").is_empty());

        // Comparisons and arrows are not assignments.
        assert!(mutable_exports_js("export let n = 0;\nif (n == 1) {}").is_empty());
        assert!(mutable_exports_js("export let n = 0;\nconst f = n => n;").is_empty());

        // A name that merely contains the export's name is a different name.
        assert!(mutable_exports_js("export let n = 0;\nlet number = 1; number = 2;").is_empty());
    }

    #[test]
    fn a_syntax_error_is_reported_rather_than_mangled() {
        let err = lower("import { from './x';").unwrap_err();
        assert!(!err.is_empty());
    }

    /// Exact's route loaders are `export async function load() { return
    /// import('./x') }`: an expression form inside a declaration whose span
    /// the export rewrite copies verbatim. Lowered as two top-level splices
    /// over one range they were refused as overlapping, and 21 of Exact's
    /// modules failed to lower at all.
    #[test]
    fn expression_forms_inside_an_exported_declaration_are_lowered() {
        let out = lowered(
            "export async function load() { return import('./x.js'); }\n\
             export const here = import.meta.url;\n",
        );
        assert!(
            out.contains("__ibex2_dynamic_import(require, './x.js')"),
            "{out}"
        );
        assert!(out.contains("const here = __ibex2_meta.url;"), "{out}");
        assert!(
            out.contains(r#"Object.defineProperty(exports, "load""#),
            "{out}"
        );
        assert!(!out.contains("return import("), "{out}");
        assert!(!out.contains("import.meta"), "{out}");
    }

    #[test]
    fn expression_forms_inside_a_default_export_are_lowered() {
        let out = lowered("export default function f() { return import.meta.url; }\n");
        assert!(out.contains("return __ibex2_meta.url;"), "{out}");
        assert!(out.contains("exports.default = f;"), "{out}");

        let out = lowered("export default () => import('./y.js');\n");
        assert!(
            out.contains("exports.default = () => __ibex2_dynamic_import(require, './y.js');"),
            "{out}"
        );
    }

    /// The forms nest: a dynamic import whose specifier is built from
    /// `import.meta`. The inner form is rendered inside the outer's argument,
    /// once.
    #[test]
    fn nested_expression_forms_are_lowered_inside_out() {
        let out = lowered("export const p = import(new URL('./z.js', import.meta.url).href);\n");
        assert!(
            out.contains(
                "__ibex2_dynamic_import(require, new URL('./z.js', __ibex2_meta.url).href)"
            ),
            "{out}"
        );
        assert_eq!(out.matches("__ibex2_meta").count(), 1, "{out}");
    }

    /// A form outside any declaration still lowers, and at top level a
    /// dynamic import alone is enough to need the wrapper's helpers.
    #[test]
    fn expression_forms_between_declarations_are_lowered() {
        let out = lowered("import { a } from './a';\nconst p = import('./b.js');\nexport { a };\n");
        assert!(
            out.contains("const p = __ibex2_dynamic_import(require, './b.js');"),
            "{out}"
        );
        assert!(out.contains(r#"require("./a")"#), "{out}");
    }

    /// The argument range is the parser's, so a comment before the
    /// parenthesis — or an options argument — does not derail it.
    #[test]
    fn a_dynamic_import_is_lowered_by_span_not_by_searching_for_a_parenthesis() {
        let out = lowered("export const p = import /*(*/('./x.js');\n");
        assert!(
            out.contains("__ibex2_dynamic_import(require, './x.js')"),
            "{out}"
        );
        let out = lowered("export const p = import /* ) */ ('./x.js');\n");
        assert!(
            out.contains("__ibex2_dynamic_import(require, './x.js')"),
            "{out}"
        );
        let out = lowered("export const p = import('./x.json', { with: { type: 'json' } });\n");
        assert!(
            out.contains("__ibex2_dynamic_import(require, './x.json', { with: { type: 'json' } })"),
            "{out}"
        );
    }

    /// A lowered ES module is strict; CommonJS is left as written.
    #[test]
    fn a_lowered_module_is_strict_and_commonjs_is_not() {
        let out = lowered("import { a } from './a';\nexport const b = a;\n");
        assert!(out.starts_with("\"use strict\";\n"), "{out}");
        assert_eq!(
            lowered("const a = require('./a');\n"),
            "const a = require('./a');\n"
        );
    }
}
