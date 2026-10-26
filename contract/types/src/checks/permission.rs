//! `requestPermission(id, path, mode)` (LLP 1069.010.000 D7).

use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

/// Three strings, positional; `mode` is `"read"` or `"readwrite"`, as the File
/// System Access API's `FileSystemHandle.requestPermission({mode})` takes it.
pub(super) fn args(
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    const USAGE: &str = "`requestPermission` takes three strings, `requestPermission(\"granted\", path, \"readwrite\")`: the element whose `change` hears `granted` or `denied`, a `doc:/` path, and `\"read\"` or `\"readwrite\"`";
    if args.len() != 3 || args.iter().any(|a| matches!(a, Expr::NamedArg(..))) {
        return err("type-permission-argument", USAGE, span);
    }
    for arg in args {
        let t = infer(arg, scope, shapes)?;
        if Ty::String.unify(&t).is_none() {
            return err("type-permission-argument", USAGE, arg.span());
        }
    }
    if let Expr::Str(mode, at) = &args[2] {
        if mode != "read" && mode != "readwrite" {
            return err("type-permission-argument", USAGE, *at);
        }
    }
    Ok(())
}
