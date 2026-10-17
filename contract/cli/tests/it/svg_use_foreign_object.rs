const WARNING: &str = "lower-svg-use-foreign-object";

#[test]
fn svg_use_foreign_object_names_the_use_and_repairs() {
    let src = "component App\n  view\n    svg\n      defs\n        symbol id=\"label\"\n          foreignObject width=100 height=30\n            text \"Monday\"\n      use href=\"#label\"\n";
    let (result, warnings) = contract::compile_warned(src);
    assert!(result.is_ok(), "{result:?}");
    let warnings: Vec<_> = warnings.iter().filter(|w| w.id == WARNING).collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let warning = warnings[0];
    assert_eq!((warning.span.line, warning.span.col), (8, 7));
    for part in [
        "#label",
        "draws nothing",
        "directly",
        "web",
        "box",
        "every host",
    ] {
        assert!(warning.message.contains(part), "{part}: {warning}");
    }
    let repaired = src.replace(
        "      use href=\"#label\"",
        "      foreignObject width=100 height=30\n        text \"Monday\"",
    );
    let (result, warnings) = contract::compile_warned(&repaired);
    assert!(result.is_ok(), "{result:?}");
    assert!(!warnings.iter().any(|w| w.id == WARNING), "{warnings:?}");
}

#[test]
fn svg_use_foreign_object_resolves_only_unambiguous_literal_targets() {
    let mut failures = Vec::new();
    for (view, expected) in [
        ("svg\n  defs\n    g id=\"label\"\n      g\n        foreignObject\n  use href=\"#label\"", 1),
        ("svg\n  foreignObject id=\"label\"\n  use href=\"#label\"", 1),
        ("svg\n  defs id=\"label\"\n    g\n      foreignObject\n  use href=\"#label\"", 1),
        ("svg\n  svg id=\"label\"\n    foreignObject\n  use href=\"#label\"", 1),
        ("column\n  svg\n    use href=\"#label\"\n  svg\n    g id=\"label\"\n      foreignObject", 1),
        ("svg\n  g id=\"label\"\n    if on\n      foreignObject\n  if on\n    use href=\"#label\"", 1),
        ("svg\n  g id=\"label\"\n    foreignObject\n  use href=\"#label\"\n  use href=\"#label\"", 2),
        ("svg\n  foreignObject", 0),
        ("svg\n  g id=\"label\"\n    rect width=10 height=10\n  use href=\"#label\"", 0),
        ("svg\n  foreignObject id=\"label\"\n  use href=ref", 0),
        ("svg\n  foreignObject id=\"label\"\n  use href=(on ? \"#label\" : \"#other\")", 0),
        ("svg\n  foreignObject id=\"label\"\n  use href=\"other.svg#label\"", 0),
        ("svg\n  use href=\"#missing\"", 0),
        ("svg\n  foreignObject id=\"label\"\n  g id=\"label\"\n  use href=\"#label\"", 0),
        ("svg\n  foreignObject id=\"label\"\n  if on\n    g id=\"label\"\n  use href=\"#label\"", 0),
        ("column\n  box id=\"label\"\n  svg\n    foreignObject id=\"label\"\n    use href=\"#label\"", 0),
        ("svg\n  foreignObject id=(on ? \"label\" : \"other\")\n  use href=\"#label\"", 0),
        ("column\n  box id=ref\n  svg\n    foreignObject id=\"label\"\n    use href=\"#label\"", 0),
        ("svg\n  if on\n    foreignObject id=\"label\"\n  use href=\"#label\"", 0),
        ("svg\n  if on\n    g\n  else\n    foreignObject id=\"label\"\n  use href=\"#label\"", 0),
        ("svg\n  for n in [1] key=n\n    foreignObject id=\"label\"\n  use href=\"#label\"", 0),
        ("svg\n  match some(1)\n    case some(n)\n      foreignObject id=\"label\"\n    case none\n      g\n  use href=\"#label\"", 0),
        ("svg\n  foreignObject\n    box id=\"label\"\n  use href=\"#label\"", 0),
        ("svg\n  use id=\"label\" href=\"#label\"", 0),
    ] {
        let view: String = view.lines().map(|line| format!("    {line}\n")).collect();
        let src = format!("component App\n  state on = true\n  state ref = \"#label\"\n  view\n{view}");
        let (result, warnings) = contract::compile_warned(&src);
        let got = warnings.iter().filter(|w| w.id == WARNING).count();
        if result.is_err() || got != expected {
            failures.push(format!("{src}\nexpected {expected}, got {got}; errors: {:?}; warnings: {warnings:?}", result.err()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn svg_use_foreign_object_checks_expanded_components_once_per_source() {
    let src = "component App\n  view\n    column\n      svg\n        foreignObject id=\"label\"\n      Instance()\n      Instance()\ncomponent Instance\n  view\n    svg\n      use href=\"#label\"\n";
    let (result, warnings) = contract::compile_warned(src);
    assert!(result.is_ok(), "{result:?}");
    let lines: Vec<_> = warnings
        .iter()
        .filter(|w| w.id == WARNING)
        .map(|w| w.span.line)
        .collect();
    assert_eq!(lines, [11]);
}
