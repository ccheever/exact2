//! TypeScript's side of the data seam, derived from the plan's signatures.
//! @ref LLP 1027 D2/D5 — no separately maintained argument or result shapes.

use exact_plan::{Plan, TypeKind};
use std::fmt::Write;

/// Generate an `app.d.ts` from a compiled Contract's source signatures.
///
/// `Sources` checks each provider independently; `Answer` checks the module's
/// dispatcher. Types describe JSON values, not Rust's positional records.
/// Runner-owned `exactDelivery` and `exactViewport` are not app implementation duties.
/// @ref LLP 1039 D1
/// This is a build-time artifact, never a runtime import or committed file.
pub fn typescript(plan: &Plan) -> Result<String, String> {
    // Public callers may construct a Plan directly. Refuse malformed tables
    // before following any indices, just as the runtime decoder does.
    Plan::decode(&plan.encode()).map_err(|e| format!("invalid plan: {e:?}"))?;
    let quoted = |s: &str| serde_json::to_string(s).expect("a string is JSON");
    let mut out = String::from(
        "// @generated from Contract source signatures; do not edit.\n\
         // Static types do not replace runtime shape, identity, or grant checks.\n\n",
    );
    // Ibex2 owns the storage surface. Include its declaration source directly:
    // this compiler needs neither its Rust runtime nor a JavaScript engine.
    out.push_str(include_str!(
        "../../../../ibex/crates/ibex2/src/bindings/storage.d.ts"
    ));
    out.push('\n');
    for (i, row) in plan.types.iter().enumerate() {
        write!(out, "type T{i} = ").unwrap();
        match row.kind {
            TypeKind::Number => out.push_str("number"),
            TypeKind::Bool => out.push_str("boolean"),
            TypeKind::String => out.push_str("string"),
            TypeKind::Unit => out.push_str("null"),
            TypeKind::Option | TypeKind::List => {
                let elem = row.elem.ok_or_else(|| format!("type {i} has no element"))?;
                write!(
                    out,
                    "T{}{}",
                    elem.0,
                    if row.kind == TypeKind::Option {
                        " | null"
                    } else {
                        "[]"
                    }
                )
                .unwrap();
            }
            TypeKind::Record => {
                out.push_str("{ ");
                for field in &plan.fields
                    [row.fields.start as usize..row.fields.start as usize + row.fields.len as usize]
                {
                    write!(out, "{}: T{}; ", quoted(plan.str(field.name)), field.ty.0).unwrap();
                }
                out.push('}');
            }
        }
        out.push_str(";\n");
    }
    out.push_str("\nexport interface SourceMap {\n");
    for row in &plan.sources {
        let name = plan.str(row.name);
        if matches!(name, "exactDelivery" | "exactViewport") {
            continue;
        }
        write!(out, "  {}: {{ args: [", quoted(name)).unwrap();
        for (i, param) in plan.source_params
            [row.params.start as usize..row.params.start as usize + row.params.len as usize]
            .iter()
            .enumerate()
        {
            if i != 0 {
                out.push_str(", ");
            }
            write!(out, "T{}", param.ty.0).unwrap();
        }
        writeln!(out, "]; result: T{} }};", row.ty.0).unwrap();
    }
    out.push_str(
        "}\n\n\
         export type Source = keyof SourceMap;\n\
         export type Args<S extends Source> = SourceMap[S]['args'];\n\
         export type Result<S extends Source> = SourceMap[S]['result'];\n\
         export interface Store {\n\
           get(name: string): string | null;\n\
           set(name: string, value: string): void;\n\
           forget(name: string): void;\n\
         }\n\
         export interface NativeModule { call(request: Record<string, unknown>): Record<string, unknown> }\n\
         export type Sources = { [S in Source]: (args: Args<S>, store: Store, storage: Storage, native?: NativeModule | null) => Result<S> | Promise<Result<S>> };\n\
         export type Answer = <S extends Source>(source: S, args: Args<S>, store: Store, storage: Storage, native?: NativeModule | null) => Result<S> | Promise<Result<S>>;\n",
    );
    Ok(out)
}
