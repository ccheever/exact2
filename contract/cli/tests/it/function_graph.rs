//! Shared function dependencies must not be traversed once per path.

fn app(declarations: &str) -> String {
    format!("{declarations}component App\n  view\n    text \"hello\"\n")
}

#[test]
fn repeated_shared_helpers_do_not_change_the_plan() {
    let mut declarations = String::from("fn f0(x: number): number = x\n");
    // A small source whose dependency graph contains millions of paths.
    // None is expanded into the app; only declaration validation visits it.
    for i in 1..24 {
        declarations.push_str(&format!(
            "fn f{i}(x: number): number = f{}(x) + f{}(x)\n",
            i - 1,
            i - 1
        ));
    }
    let baseline = contract::compile(&app("")).unwrap().encode();
    assert_eq!(
        contract::compile(&app(&declarations)).unwrap().encode(),
        baseline
    );
    // Declaration order must not determine whether a shared graph is accepted.
    let reverse = declarations.lines().rev().collect::<Vec<_>>().join("\n") + "\n";
    assert_eq!(
        contract::compile(&app(&reverse)).unwrap().encode(),
        baseline
    );
}

#[test]
fn shared_completed_subgraphs_do_not_hide_a_later_cycle() {
    for declarations in [
        "fn leaf(x: number): number = x\nfn twice(x: number): number = leaf(x) + leaf(x)\nfn bad(x: number): number = twice(x) + bad(x)\n",
        "fn leaf(x: number): number = x\nfn first(x: number): number = leaf(x) + second(x)\nfn second(x: number): number = leaf(x) + first(x)\n",
        "fn first(x: number): number = second(x)\nfn second(x: number): number = third(x)\nfn third(x: number): number = second(x)\n",
        "fn leaf(x: number): number = x\nfn first(x: number): number = floor(second(x))\nfn second(x: number): number = floor(first(x))\n",
    ] {
        let error = contract::compile(&app(declarations)).unwrap_err();
        assert_eq!(error.id, "type-fn-recursive", "{error}");
        assert!(error.message.contains(" → "), "{error}");
    }
}
