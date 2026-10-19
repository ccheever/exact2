//! What a button invokes in the terminal's top layer, as HTML resolves it
//! (`exact_kernel::Invocation`): after its own `press`, by the rule a
//! `commandfor` governs, and by the mode a layer was opened in.
use super::*;

const APP: &str = r#"component App
  state target = "a"
  action retarget
    target = "b"
  action shut
    close("dp")
  view
    column
      button press=retarget commandfor=target command="show-modal" id="retargets"
        text "Retargets itself"
      button commandfor="missing" command="show-popover" popovertarget="menu" id="dangling"
        text "Dangling"
      button commandfor="a" id="no-command"
        text "No command"
      button popovertarget="dp" id="open-dp"
        text "As a popover"
      button commandfor="dp" command="close" id="close-dp"
        text "Close"
      button press=shut id="shut"
        text "close(id)"
      button commandfor="a" command="show-modal" id="open-a"
        text "Modally"
      dialog id="a" popover="auto"
        button commandfor="a" command="hide-popover" id="a-hide"
          text "Hide a"
        button commandfor="a" command="close" id="a-close"
          text "Close a"
      dialog id="b"
        button commandfor="b" command="close" id="b-close"
          text "Close b"
      dialog id="dp" popover="auto"
        button popovertarget="dp" popovertargetaction="hide" id="dp-hide"
          text "Hide dp"
      column id="menu" popover="auto"
        button popovertarget="menu" popovertargetaction="HIDE" id="menu-hide"
          text "Hide menu"
"#;

fn boot() -> Host<()> {
    let plan = contract::compile(APP).unwrap();
    Host::boot(plan, (), Mode::Fullscreen, 80, 40).unwrap()
}

fn node(host: &Host<()>, name: &str) -> ViewId {
    host.find(|n| n.props.str(PropId::Id) == Some(name))
        .unwrap()
}

fn press(host: &mut Host<()>, name: &str) {
    let id = node(host, name);
    host.press(id);
}

/// The `id`s of the open layers, bottom to top.
fn open(host: &Host<()>) -> Vec<String> {
    let kernel = host.runner.kernel();
    (host.layers.iter())
        .filter_map(|(id, ..)| kernel.node(*id)?.props.str(PropId::Id).map(str::to_string))
        .collect()
}

#[test]
fn a_button_invokes_what_it_names_after_its_press() {
    let mut host = boot();
    press(&mut host, "retargets");
    assert_eq!(open(&host), ["b"], "its handler retargeted it first");
}

#[test]
fn a_commandfor_that_names_nothing_falls_back_and_hide_is_any_case() {
    let mut host = boot();
    press(&mut host, "dangling");
    assert_eq!(open(&host), ["menu"]);
    press(&mut host, "menu-hide");
    assert!(open(&host).is_empty(), "HIDE hides");
}

#[test]
fn a_commandfor_with_no_command_invokes_nothing() {
    let mut host = boot();
    press(&mut host, "no-command");
    assert!(open(&host).is_empty());
}

#[test]
fn close_acts_only_on_a_dialog_open_modally_and_popover_actions_only_as_a_popover() {
    let mut host = boot();
    press(&mut host, "open-dp");
    assert_eq!(open(&host), ["dp"]);
    host.press(node(&host, "close-dp"));
    host.press(node(&host, "shut"));
    assert_eq!(
        open(&host),
        ["dp"],
        "close leaves a dialog shown as a popover open"
    );
    press(&mut host, "dp-hide");
    assert!(open(&host).is_empty());

    press(&mut host, "open-a");
    assert_eq!(open(&host), ["a"]);
    press(&mut host, "a-hide");
    assert_eq!(
        open(&host),
        ["a"],
        "hide-popover leaves a modal dialog open"
    );
    press(&mut host, "a-close");
    assert!(open(&host).is_empty());
}

#[test]
fn caltrain_builds_in_the_terminal_and_refuses_activity_sends_at_runtime() {
    let plan = caltrain::build().unwrap();
    let mut app = caltrain::boot(plan.clone(), exact_kernel::Kernel::with_monospace()).unwrap();
    let data = std::mem::take(app.data());
    let mut host =
        crate::host::Host::boot(plan, data, crate::host::Mode::Fullscreen, 80, 24).unwrap();
    assert!(host.runner.resource("activities").is_some());
    for (action, send) in [
        ("startTracking", "startActivity"),
        ("delayTrain", "updateActivity"),
        ("endTracking", "endActivity"),
    ] {
        host.act(action);
        assert!(host
            .runner
            .journal()
            .any(|line| line.contains(&format!("{send} is not linked into this artifact"))));
        assert!(!host.runner.has_pending());
    }
}
