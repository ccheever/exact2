use super::{boot, press, text_of};

fn source(binding: &str, declarations: &str, view: &str) -> String {
    format!(
        "shape Viewport\n  width: number\nfn pick(n: number): number = n + 1\ncomponent App\n  view\n    Child()\ncomponent Child\n  {binding}\n{declarations}  view\n{view}\ncomponent Frame\n  slot\n  view\n    children\ncomponent Label\n  props\n    value: string\n  view\n    text value testId=\"result\"\n"
    )
}

const STATE: &str = "state pick = 7";
const DEVICE: &str = "resource pick = exactViewport() as shape Viewport";

#[test]
fn state_values_leave_function_calls_intact() {
    let mut r = boot(include_str!(
        "../../../../../semantics/corpus/components/state-fn.contract"
    ));
    for (id, expected) in [
        ("direct", "7 6"),
        ("derived", "8"),
        ("initial", "6"),
        ("fill", "7 6"),
        ("argument", "8"),
        ("provided", "8"),
        ("shown", "0"),
    ] {
        assert_eq!(text_of(&r, id), expected, "{id}");
    }
    for (button, value, derived, shown) in [("update", 10, 11, 8), ("relay", 13, 14, 11)] {
        press(&mut r, button);
        assert_eq!(text_of(&r, "direct"), format!("{value} 6"));
        assert_eq!(text_of(&r, "fill"), format!("{value} 6"));
        for id in ["derived", "argument", "provided"] {
            assert_eq!(text_of(&r, id), derived.to_string(), "{id}");
        }
        assert_eq!(text_of(&r, "shown"), shown.to_string());
        assert_eq!(text_of(&r, "chosen"), "16");
    }
}

#[test]
fn device_values_leave_view_and_initializer_calls_intact() {
    for view in [
        "    text `${pick.width} ${pick(5)} ${initial}` testId=\"result\"",
        "    Frame()\n      text `${pick.width} ${pick(5)} ${initial}` testId=\"result\"",
        "    Label(value=`${pick.width} ${pick(5)} ${initial}`)",
    ] {
        let mut r = boot(&source(DEVICE, "  state initial = pick(3)\n", view));
        r.set_viewport(900.0, 600.0).unwrap();
        assert_eq!(text_of(&r, "result"), "900 6 4");
    }
}

#[test]
fn state_and_device_names_leave_calls_in_view_derives_intact() {
    for (binding, read, value) in [(STATE, "pick", 7), (DEVICE, "pick.width", 900)] {
        let src = source(
            binding,
            &format!("  derive computed = pick({read})\n"),
            "    text `${computed}` testId=\"result\"",
        );
        let mut r = boot(&src);
        r.set_viewport(900.0, 600.0).unwrap();
        assert_eq!(text_of(&r, "result"), (value + 1).to_string());
    }
}

#[test]
fn state_and_device_names_leave_calls_in_action_derives_intact() {
    for (binding, read, value) in [(STATE, "pick", 7), (DEVICE, "pick.width", 900)] {
        for expression in [format!("pick({read})"), "computed".into()] {
            let src = source(
                binding,
                &format!("  derive computed = pick({read})\n  state shown = 0\n  action update()\n    shown = {expression}\n  action relay()\n    update()\n"),
                "    column\n      text `${shown}` testId=\"result\"\n      button \"Update\" press=update testId=\"update\"\n      button \"Relay\" press=relay testId=\"relay\"",
            );
            for button in ["update", "relay"] {
                let mut r = boot(&src);
                r.set_viewport(900.0, 600.0).unwrap();
                assert_eq!(text_of(&r, "result"), "0");
                press(&mut r, button);
                assert_eq!(text_of(&r, "result"), (value + 1).to_string());
            }
        }
    }
}

#[test]
fn state_names_leave_stdlib_calls_intact() {
    let src = source(
        "state max = 7",
        "  derive computed = max(max, 5)\n",
        "    text `${max} ${computed} ${max(3, 5)}` testId=\"result\"",
    );
    assert_eq!(text_of(&boot(&src), "result"), "7 7 5");
}
