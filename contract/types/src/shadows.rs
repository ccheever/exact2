//! An app's action or `fn` named like a standard function shadows it, as a
//! JavaScript function declared over a global does. A call of it that fails
//! to type says so (the euxy port's action `sortBy(key)` beside a `derive`
//! calling the standard `sortBy(xs, f, "desc")`, refused as "`sortBy` takes
//! at most 1 argument(s), given 3").

use crate::Shapes;
use contract_syntax::File;
use exact_plan::Stdlib;

/// Each action and `fn` named like a standard function, by (its component,
/// empty for a `fn`; its name). An action prop (`action`, its arity
/// inferred) refuses no call's arguments, so none is listed.
pub(crate) fn declare(file: &File, shapes: &mut Shapes) {
    let actions = file.components.iter().flat_map(|c| {
        c.actions
            .iter()
            .map(|a| (&c.name[..], &a.name, "action", a.span))
    });
    let fns = file.fns.iter().map(|f| ("", &f.name, "`fn`", f.span));
    for (component, name, kind, span) in actions.chain(fns) {
        if Stdlib::from_name(name).is_some() {
            let key = (component.to_string(), name.clone());
            shapes.shadows.entry(key).or_insert((kind, span));
        }
    }
}

/// What a refused call of `name` adds when it is the app's `fn`
/// (`component` `None`) or an action of `component` that shadows a standard
/// function: the author most likely meant the standard one. An action the
/// root was expanded with (a child's, `sortBy#1` where the name was taken)
/// is named when one component alone declares it, and otherwise not at all.
pub(crate) fn note(name: &str, component: Option<&str>, shapes: &Shapes) -> String {
    let (base, lifted) = name
        .split_once('#')
        .map_or((name, false), |(base, _)| (base, true));
    let key = |c: &str| (c.to_string(), base.to_string());
    let only = || {
        let mut all = shapes
            .shadows
            .iter()
            .filter(|((c, n), _)| !c.is_empty() && n == base);
        all.next().filter(|_| all.next().is_none()).map(|(_, v)| v)
    };
    let found = match component {
        None => shapes.shadows.get(&key("")),
        Some(c) if !lifted => shapes.shadows.get(&key(c)).or_else(only),
        Some(_) => only(),
    };
    let name = base;
    let Some((kind, at)) = found else {
        return String::new();
    };
    format!(
        "; `{name}` here is your {kind} at line {}, which shadows the standard function `{name}` (`contract vocab {name}`): rename the {kind} to call the standard one",
        at.line
    )
}
