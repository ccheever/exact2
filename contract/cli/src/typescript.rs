//! TypeScript's side of the data seam, derived from the plan's signatures.
//! @ref LLP 1027 D2/D5 — no separately maintained argument or result shapes.

use exact_plan::{Plan, TypeKind, TypesId};
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
         // Static types do not replace runtime shape, identity, or grant checks.\n",
    );
    summary(plan, &mut out);
    out.push('\n');
    // Ibex2 owns the storage surface. Include its declaration source directly:
    // this compiler needs neither its Rust runtime nor a JavaScript engine.
    out.push_str(include_str!(
        "../../../vendor/ibex/crates/ibex2/src/bindings/storage.d.ts"
    ));
    out.push('\n');
    for (i, row) in plan.types.iter().enumerate() {
        write!(out, "type T{i} = ").unwrap();
        match row.kind {
            TypeKind::Number => out.push_str("number"),
            TypeKind::Bool => out.push_str("boolean"),
            TypeKind::String => {
                if row.fields.is_empty() {
                    out.push_str("string");
                } else {
                    out.push_str(
                        &row.fields
                            .iter()
                            .map(|f| quoted(plan.str(plan.field(f).name)))
                            .collect::<Vec<_>>()
                            .join(" | "),
                    );
                }
            }
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
         /** A generated backend operation's arguments (distinct from this app's source tuple). */\n\
         export type ArgsOf<O> = O extends { readonly __args?: infer A } ? A : never;\n\
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
         /** A resource-owned WebSocket, for TypeScript on the web JS target and native hosts (LLP 1016.000 D7). Methods are synchronous-mutation-only; refresh or disposal invalidates it. */\n\
         export interface StreamSocket { readonly readyState: number; readonly bufferedAmount: number; send(data: string | ArrayBuffer | ArrayBufferView): void; close(code?: number, reason?: string): void }\n\
         export interface StreamEvent { readonly type: string; readonly data: string | ArrayBuffer; readonly lastEventId: string; readonly coalesced: number; readonly kind?: string; readonly message?: string; readonly status?: number; readonly code?: number; readonly reason?: string; readonly wasClean?: boolean }\n\
         declare global {\n\
           interface RequestInit { exactStream?: (event: StreamEvent, socket?: StreamSocket) => unknown; exactIndependentHttp?: { maxResponseBytes: number }; /** A deadline for the whole exchange, 1 to 3600000 ms: the request is cancelled and the fetch rejects with a FetchError whose `kind` is \"Timeout\". Not with `exactStream`. */ exactTimeout?: number; /** The body is this app file (`app:/…`), read by the host when it sends the request, under `fs.read`, at most 64 MiB; its bytes never enter JavaScript. Not with `body`, nor on a GET or HEAD; no Content-Type is added. */ exactBodyFrom?: string }\n\
           /** An answer that keeps coming: `exactStream` maps each event, and the end, to the answer. */\n\
           function fetch<T>(input: string | URL, init: RequestInit & { exactStream: (event: StreamEvent, socket?: StreamSocket) => T }): Promise<T>;\n\
         }\n\
         export type Sources = { [S in Source]: (args: Args<S>, store: Store, storage: Storage, native?: NativeModule | null) => Result<S> | Promise<Result<S>> };\n\
         export type Answer = <S extends Source>(source: S, args: Args<S>, store: Store, storage: Storage, native?: NativeModule | null) => Result<S> | Promise<Result<S>>;\n\
         /** A send shown over a resource it `refreshes`, in send order: `reply` once its reply has landed. `answered`: it landed and this answer was asked after it, so the answer should include it; call `keep()` to go on showing it when the answer does not include it yet. */\n\
         export type Write = { [S in Source]: { readonly id: number; readonly mutation: string; readonly source: S; readonly args: Args<S>; readonly reply?: Result<S>; readonly answered: boolean; keep(): void } }[Source];\n\
         /** What a resource shows while writes affect it: its answer with them laid over it. Synchronous, with no effects; `undefined` shows the answer and stops showing every answered write. */\n\
         export type Overlay = <S extends Source>(source: S, args: Args<S>, answer: Result<S>, writes: readonly Write[]) => Result<S> | undefined;\n",
    );
    tokens(plan, &mut out);
    Ok(out)
}

/// @ref LLP 1121 D12 — the design's colour tokens as a string-literal
/// union, so a data module returning a token's name is checked
/// (`ColorToken`); none without a design. A synthetic token (an expression
/// a row writes) is no name a module returns.
fn tokens(plan: &Plan, out: &mut String) {
    let mut names: Vec<&str> = Vec::new();
    for row in &plan.theme {
        let name = plan.str(row.name);
        if !name.contains('(') && !names.contains(&name) {
            names.push(name);
        }
    }
    if names.is_empty() {
        return;
    }
    let quoted: Vec<String> = names
        .iter()
        .map(|n| serde_json::to_string(n).expect("a string is JSON"))
        .collect();
    let _ = writeln!(
        out,
        "/** A colour token of the app's design, by its name (LLP 1121 D12). */\nexport type ColorToken = {};",
        quoted.join(" | ")
    );
}

/// The file's top (LLP 1116.006 D7): what `app.ts` writes, one line each, so a builder reads
/// these lines and not the declarations below. Each source the data module implements with its
/// arguments and answer, then each named shape those carry, as TypeScript types; comments only.
fn summary(plan: &Plan, out: &mut String) {
    let sources: Vec<_> = plan
        .sources
        .iter()
        .filter(|row| !exact_plan::runner_owned_source(plan.str(row.name)))
        .collect();
    if sources.is_empty() {
        return;
    }
    let params = |row: &exact_plan::SourcesRow| {
        plan.source_params
            [row.params.start as usize..row.params.start as usize + row.params.len as usize]
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>()
    };
    out.push_str(
        "//\n// What app.ts writes, in brief (the declarations follow): `sources: Sources` has one\n\
         // function per source, given its arguments as an array (then store, storage, native) and\n\
         // answering its shape, or a Promise of it.\n",
    );
    let mut shapes = Vec::new();
    for row in &sources {
        let args: Vec<_> = params(row)
            .into_iter()
            .map(|ty| inline(plan, ty, &mut shapes))
            .collect();
        let answer = inline(plan, row.ty, &mut shapes);
        writeln!(
            out,
            "//   {}: (args: [{}]) => {answer}",
            key(plan.str(row.name)),
            args.join(", ")
        )
        .unwrap();
    }
    if shapes.is_empty() {
        return;
    }
    // A shape's own fields name the shapes inside it, which join the list after it.
    let mut lines = Vec::new();
    let mut i = 0;
    while i < shapes.len() {
        let ty = shapes[i];
        let body = record(plan, ty, &mut shapes);
        lines.push((
            ty.0,
            format!("//   {} = {body}", plan.str(plan.types[ty.0 as usize].name)),
        ));
        i += 1;
    }
    lines.sort();
    // An example of naming one: the first source answering a shape, or a list of one.
    let example = sources.iter().find_map(|row| {
        let source = plan.str(row.name);
        let answer = &plan.types[row.ty.0 as usize];
        match answer.kind {
            TypeKind::Record if !plan.str(answer.name).is_empty() => Some(format!(
                "`type {} = Result<'{source}'>`",
                plan.str(answer.name)
            )),
            TypeKind::List => answer
                .elem
                .map(|e| &plan.types[e.0 as usize])
                .filter(|e| e.kind == TypeKind::Record && !plan.str(e.name).is_empty())
                .map(|e| format!("`type {} = Result<'{source}'>[number]`", plan.str(e.name))),
            _ => None,
        }
    });
    match example {
        Some(example) => writeln!(
            out,
            "// The shapes. None is exported: name one by its source, {example}."
        )
        .unwrap(),
        None => out.push_str(
            "// The shapes. None is exported: name one by its source, `Result<'source'>`.\n",
        ),
    }
    for (_, line) in lines {
        out.push_str(&line);
        out.push('\n');
    }
}

/// A type as the summary writes it: a named shape by its name (noted in `shapes` once).
fn inline(plan: &Plan, ty: TypesId, shapes: &mut Vec<TypesId>) -> String {
    let row = &plan.types[ty.0 as usize];
    let mut elem = || row.elem.map(|e| inline(plan, e, shapes));
    match row.kind {
        TypeKind::Number => "number".into(),
        TypeKind::Bool => "boolean".into(),
        TypeKind::String => "string".into(),
        TypeKind::Unit => "null".into(),
        TypeKind::Option => format!("{} | null", elem().unwrap_or_default()),
        TypeKind::List => {
            let nullable = row
                .elem
                .is_some_and(|e| plan.types[e.0 as usize].kind == TypeKind::Option);
            let elem = elem().unwrap_or_default();
            if nullable {
                format!("({elem})[]")
            } else {
                format!("{elem}[]")
            }
        }
        TypeKind::Record => {
            let name = plan.str(row.name);
            if name.is_empty() {
                return record(plan, ty, shapes);
            }
            // The plan may hold a shape more than once: one line per name.
            if !shapes
                .iter()
                .any(|s| plan.str(plan.types[s.0 as usize].name) == name)
            {
                shapes.push(ty);
            }
            name.to_string()
        }
    }
}

/// A record's fields on one line, `{ id: string; done: boolean }`.
fn record(plan: &Plan, ty: TypesId, shapes: &mut Vec<TypesId>) -> String {
    let row = &plan.types[ty.0 as usize];
    let fields: Vec<_> = plan.fields
        [row.fields.start as usize..row.fields.start as usize + row.fields.len as usize]
        .iter()
        .map(|field| {
            format!(
                "{}: {}",
                key(plan.str(field.name)),
                inline(plan, field.ty, shapes)
            )
        })
        .collect();
    if fields.is_empty() {
        "{}".into()
    } else {
        format!("{{ {} }}", fields.join("; "))
    }
}

/// A property name, quoted unless it is an identifier.
fn key(name: &str) -> String {
    let identifier = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if identifier {
        name.to_string()
    } else {
        serde_json::to_string(name).expect("a string is JSON")
    }
}
