//! MIDI in the compiler (LLP 1125 D6–D8): `sendMIDI` and
//! `requestMIDIAccess` checked, `midimessage` and `midiinputs`, `clockOut=`
//! compiled and `clock=` refused, and the test steps for the driver.

fn app(body: &str, view: &str) -> String {
    format!("component A\n  state out = \"synth\"\n  state ins = [\"keys\"]\n  action go\n    {body}\n  action heard(e: MIDIMessageEvent)\n    out = e.input\n  view\n    {view}\n")
}

#[test]
fn midi_commands_events_and_inputs_compile() {
    for ok in [
        "sendMIDI(out, [144, 60, 100])",
        "sendMIDI(\"synth\", [144, 60, 100, 128, 60, 0], at=performanceNow() + 100)",
        "sendMIDI(out, [248])",
        "requestMIDIAccess()",
    ] {
        contract::compile(&app(ok, "button \"go\" press=go"))
            .unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    for view in [
        "column midimessage=heard midiinputs=ins",
        "column midimessage=heard midiinputs=[\"keys\", \"pads\"]",
        "column onMIDIMessage=heard midiinputs=\"keys\"",
    ] {
        let src = app("requestMIDIAccess()", view);
        match contract::compile(&src) {
            Ok(_) => {}
            Err(e) if view.contains("onMIDIMessage") => {
                assert!(e.message.contains("midimessage"), "{e}")
            }
            Err(e) => panic!("{view}: {e}"),
        }
    }
    for (bad, id) in [
        ("sendMIDI(out, [144, 60])", "type-send-midi"),
        ("sendMIDI(out, [60, 100])", "type-send-midi"),
        ("sendMIDI(out, [240, 1, 247])", "type-send-midi"),
        ("sendMIDI(out, [144, 300, 1])", "type-send-midi"),
        ("sendMIDI(out, [144, 60, 100], when=3)", "type-send-midi"),
        ("sendMIDI(out, \"90 3c 64\")", "type-send-midi"),
        ("sendMIDI(1, [144, 60, 100])", "type-send-midi"),
        ("requestMIDIAccess(1)", "type-request-midi-access"),
    ] {
        let e = contract::compile(&app(bad, "button \"go\" press=go")).unwrap_err();
        assert_eq!(e.id, id, "{bad}: {e}");
    }
}

#[test]
fn clock_out_and_clock_in_compile_with_and_without_bpm() {
    let seq = |opts: &str| {
        format!("component A\n  state on = false\n  sequence s when on\n    play([], {opts})\n  view\n    text \"a\"\n")
    };
    contract::compile(&seq("bpm=120, clockOut=\"synth\"")).unwrap();
    contract::compile(&seq("bpm=100, clock=\"keys\"")).unwrap();
    contract::compile(&seq("clock=\"keys\"")).unwrap();
    let e = contract::compile(&seq("from=-4")).unwrap_err();
    assert!(e.message.contains("`bpm=` or `clock=`"), "{e}");
}

#[test]
fn midi_steps_parse_and_reach_the_driver() {
    let src = "test \"midi\"\n  prefer midi.state granted midi.outputs \"synth, iac\" midi.inputs \"keys\"\n  tap midi \"keys\" 90 3c 64 b0 07 7f\n  expect midi sent \"synth\" 99 4c 7f at 500\n  expect midi missing \"synth\" fc\n  expect midi sent \"synth\" 1e 0a 00\n  tap midi \"clk\" clock 125 1000\n";
    let tests = contract::tests(src).unwrap();
    let json = contract::tests_json(&tests);
    for want in [
        r#"{"op":"prefer-midi","state":"granted","inputs":["keys"],"outputs":["synth","iac"],"line":2}"#,
        r#"{"op":"tap-midi","input":"keys","data":[144,60,100,176,7,127],"line":3}"#,
        r#"{"op":"expect-midi","output":"synth","data":[153,76,127],"present":true,"at":500,"line":4}"#,
        r#"{"op":"expect-midi","output":"synth","data":[252],"present":false,"line":5}"#,
        r#"{"op":"expect-midi","output":"synth","data":[30,10,0],"present":true,"line":6}"#,
        r#"{"op":"tap-midi","input":"clk","clock":{"bpm":125,"ms":1000},"line":7}"#,
    ] {
        assert!(json.contains(want), "{want}\n{json}");
    }
    for (bad, why) in [
        ("tap midi \"keys\"", "expected a byte"),
        ("tap midi \"keys\" 9", "not a byte"),
        ("tap midi \"keys\" clock 0 100", "a tempo above 0"),
        ("tap midi \"keys\" 90 3g", "not a byte"),
        ("expect midi \"synth\" 90 3c 64", "`sent` or `missing`"),
        ("prefer midi.colour red", "midi.state"),
        ("prefer midi.state maybe", "prompt, granted"),
        ("prefer midi.state granted midi.state denied", "given twice"),
    ] {
        let e = contract::tests(&format!("test \"t\"\n  {bad}\n")).unwrap_err();
        assert!(e.message.contains(why), "{bad}: {}", e.message);
    }
}
