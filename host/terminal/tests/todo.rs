//! The todo fixture (LLP 1101 Q9) driven headless as the agent drives it.

use exact_terminal::host::{Host, Key};

fn boot() -> Host {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/todo/terminal.contract");
    let plan =
        contract::compile_path_terminal(&path).expect("the todo entry passes the terminal profile");
    Host::boot(plan, 60, 16).expect("boots")
}

fn screen(host: &mut Host) -> String {
    host.frame().grid.text()
}

#[test]
fn adds_toggles_and_filters_in_cells() {
    let mut host = boot();
    let first = screen(&mut host);
    assert!(
        first.starts_with(" Todo") && first.lines().next().unwrap().ends_with("2 left"),
        "{first}"
    );
    assert!(
        first.contains(" ╭───"),
        "a rounded border, one cell: {first}"
    );
    assert!(first.contains("│ [x] Read LLP 1101"), "{first}");

    let input = host.by_test_id("new-item").expect("the input");
    host.press(input);
    for c in "Buy oat milk".chars() {
        host.key(Key::Char(c));
    }
    host.key(Key::Named("Enter"));
    let added = screen(&mut host);
    assert!(added.contains("│ [ ] Buy oat milk"), "{added}");
    assert!(added.lines().next().unwrap().ends_with("3 left"), "{added}");

    let item = host.by_test_id("item-2").expect("item 2");
    host.press(item);
    host.key(Key::Char('2'));
    let active = screen(&mut host);
    assert!(
        !active.contains("Write the terminal host"),
        "done items leave the active filter: {active}"
    );
    assert!(active.contains("Buy oat milk"), "{active}");
}

#[test]
fn the_keyboard_reaches_everything() {
    let mut host = boot();
    // Tab through the three filters to the first item; Enter toggles it.
    for _ in 0..4 {
        host.key(Key::Named("Tab"));
    }
    host.key(Key::Named("Enter"));
    assert!(screen(&mut host).contains("│ [ ] Read LLP 1101"));
    // `x` clears done; `n` focuses the field and puts the cursor in it.
    host.press(host.by_test_id("item-2").unwrap());
    host.key(Key::Char('x'));
    assert!(!screen(&mut host).contains("Write the terminal host"));
    host.key(Key::Named("Escape"));
    host.key(Key::Char('n'));
    assert!(
        host.frame().grid.cursor.is_some(),
        "the field takes the terminal's cursor"
    );
}
