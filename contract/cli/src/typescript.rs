//! TypeScript's side of the data seam, derived from the plan's signatures.
//! @ref LLP 1027 D2/D5 — no separately maintained argument or result shapes.

use exact_plan::{Plan, TypeKind};
use std::fmt::Write;

/// Generate an `app.d.ts` from a compiled Contract's source signatures.
///
/// `Sources` checks each provider independently; `Answer` checks the module's
/// dispatcher. Types describe JSON values, not Rust's positional records.
/// Runner-owned delivery, viewport and surface records are not app implementation duties.
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
        "../../../vendor/ibex2/src/bindings/storage.d.ts"
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
        if exact_plan::runner_owned_source(name) {
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
         /** A source's answer. A shape has no exported name: name one by its source,\n\
          * `type Recipe = Result<'recipe'>` (a list's element: `Result<'recipes'>[number]`). */\n\
         export type Result<S extends Source> = SourceMap[S]['result'];\n\
         export interface Store {\n\
           get(name: string): string | null;\n\
           set(name: string, value: string): void;\n\
           forget(name: string): void;\n\
           /** Keep a P-256 pair under the `secret.keep <name>` grant (LLP 1069.005 D1b). */\n\
           keepKey(name: string, pair: CryptoKeyPair): Promise<void>;\n\
           /** The pair kept under `name`, its private key non-extractable. */\n\
           key(name: string): Promise<CryptoKeyPair | null>;\n\
         }\n\
         declare global {\n\
           /** Sign in through the system browser (LLP 1069.006): the callback URL, or a rejection whose `status` is 499 cancelled, 403, 409, 428, 501 or 502. */\n\
           function openAuthSession(url: string, options: { callback: string; state: string; ephemeral?: boolean }): Promise<string>;\n\
           /** This carrier's auth callback, known before PAR (LLP 1069.006 D2). */\n\
           function authCallback(): string;\n\
         }\n\
         export interface NativeModule { readonly available: boolean; call(request: Record<string, unknown>): Record<string, unknown>; later(request: Record<string, unknown>): Promise<Record<string, unknown>>; watch(topic: string): void }\n\
         /** One server-sent event of a stream answer, or its end (`type: \"error\"`, LLP 1016.000). */\n\
         export interface StreamEvent { readonly type: string; readonly data: string; readonly lastEventId: string; readonly coalesced: number; readonly kind?: string; readonly message?: string; readonly status?: number }\n\
         declare global {\n\
           interface RequestInit { exactStream?: (event: StreamEvent) => unknown; exactIndependentHttp?: { maxResponseBytes: number }; /** A deadline for the whole exchange, 1 to 3600000 ms: the request is cancelled and the fetch rejects with a FetchError whose `kind` is \"Timeout\". Not with `exactStream`. */ exactTimeout?: number }\n\
           /** An answer that keeps coming: `exactStream` maps each event, and the end, to the answer. */\n\
           function fetch<T>(input: string | URL, init: RequestInit & { exactStream: (event: StreamEvent) => T }): Promise<T>;\n\
         }\n\
         export type Sources = { [S in Source]: (args: Args<S>, store: Store, storage: Storage, native?: NativeModule | null) => Result<S> | Promise<Result<S>> };\n\
         export type Answer = <S extends Source>(source: S, args: Args<S>, store: Store, storage: Storage, native?: NativeModule | null) => Result<S> | Promise<Result<S>>;\n",
    );
    Ok(out)
}
