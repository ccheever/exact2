//! The agent's verbs, run as the agent runs them: the binary, headless, on
//! an entry compiled with the terminal profile (LLP 1101.002 §0 P1, P7).

use std::path::PathBuf;
use std::process::Command;

const APP: &str = r#"component App
  state picked = "none"
  state draft = ""
  action pick(v: string)
    picked = v
  action write(value: string)
    draft = value
  view
    column
      text `picked ${picked}`
      input value=draft input=write placeholder="new task" testId="field"
      input value="locked" disabled=true testId="locked"
      button commandfor="d" command="show-modal" testId="open"
        text "open"
      dialog id="d" closedby="any"
        scroll max-height="3lh" testId="list"
          each v in ["alpha", "beta", "gamma", "delta", "epsilon"] key=v
            button press=pick(v) commandfor="d" command="close" text-align="start" testId=v
              text v
"#;

fn entry(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("exact-terminal-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, APP).unwrap();
    path
}

#[test]
fn color_modes_reach_authored_branches_and_every_writer() {
    use exact_terminal::{
        color::ColorDepth,
        host::{Host, Mode},
        vt::Vt,
    };
    let source = r##"shape Terminal
  colors: number
component App
  resource terminal = exactSurface("terminal") as shape Terminal
  state changed = false
  action change
    changed = true
  view
    column
      column role="log"
        column id="entry"
          text "history" color="#ff0000" background-color="#0000ff"
      when terminal.colors == 0
        text "MONO" font-weight=700 text-decoration-line="underline"
      else
        text `COLOR ${terminal.colors}` color="#00ff00"
      button appearance="none" press=change testId="change"
        text (changed ? "changed" : "ready") color=(changed ? "#0000ff" : "#ff0000")
"##;
    for mode in [Mode::Inline, Mode::Fullscreen] {
        for colors in [
            ColorDepth::Monochrome,
            ColorDepth::Ansi16,
            ColorDepth::Ansi256,
            ColorDepth::TrueColor,
        ] {
            let plan = contract::compile_terminal(source).unwrap();
            let mut host = Host::boot(plan, (), mode, 32, 10).unwrap();
            host.set_colors(colors);
            let mut vt = Vt::new(&mut host);
            let first = vt.render(&mut host);
            assert!(vt.text(true).contains("history"));
            assert!(vt
                .text(false)
                .contains(if colors == ColorDepth::Monochrome {
                    "MONO"
                } else {
                    "COLOR"
                }));
            host.act("change");
            let second = vt.render(&mut host);
            assert!(vt.text(false).contains("changed"));
            for out in [
                first,
                second,
                vt.ansi(),
                host.render(0, 10).grid.ansi(),
                host.render_settled(0, 10).grid.ansi(),
            ] {
                let mut parser = vt100::Parser::new(10, 32, 0);
                parser.process(out.as_bytes());
                for row in 0..10 {
                    for col in 0..32 {
                        let cell = parser.screen().cell(row, col).unwrap();
                        for color in [cell.fgcolor(), cell.bgcolor()] {
                            match (colors, color) {
                                (ColorDepth::Monochrome, vt100::Color::Default)
                                | (
                                    ColorDepth::Ansi16
                                    | ColorDepth::Ansi256
                                    | ColorDepth::TrueColor,
                                    vt100::Color::Default,
                                ) => {}
                                (ColorDepth::Ansi16, vt100::Color::Idx(i)) => assert!(i < 16),
                                (ColorDepth::Ansi256, vt100::Color::Idx(_))
                                | (ColorDepth::TrueColor, vt100::Color::Rgb(..)) => {}
                                other => panic!("wrong output palette: {other:?}"),
                            }
                        }
                    }
                }
            }
            if colors == ColorDepth::Monochrome {
                let grid = &host.frame().grid;
                let mut found = false;
                for y in 0..grid.rows {
                    for x in 0..grid.cols {
                        let c = grid.cell(x, y);
                        if c.text == "M" && c.style.bold && c.style.underline {
                            found = true;
                        }
                    }
                }
                assert!(found, "monochrome preserves authored emphasis");
            }
        }
    }
}

#[test]
fn cli_color_detection_and_explicit_overrides_agree_with_the_app() {
    let path = entry("colors");
    std::fs::write(&path, "shape Terminal\n  colors: number\ncomponent App\n  resource terminal = exactSurface(\"terminal\") as shape Terminal\n  view\n    text `depth ${terminal.colors}` color=\"#ff0000\"\n").unwrap();
    for (term, no_color, flag, expected) in [
        ("xterm-256color", "1", "auto", "depth 0"),
        ("xterm-256color", "0", "auto", "depth 0"),
        ("xterm-256color", "", "auto", "depth 8"),
        ("dumb", "", "auto", "depth 0"),
        ("vt100", "", "auto", "depth 0"),
        ("xterm", "", "auto", "depth 4"),
        ("xterm-256color", "", "never", "depth 0"),
        ("dumb", "1", "truecolor", "depth 24"),
        ("dumb", "1", "16", "depth 4"),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_exact-terminal"))
            .arg(&path)
            .args(["--color", flag, "print"])
            .env("TERM", term)
            .env("NO_COLOR", no_color)
            .env_remove("COLORTERM")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            String::from_utf8_lossy(&out.stdout).contains(expected),
            "{term} {no_color} {flag}: {}",
            String::from_utf8_lossy(&out.stdout)
        );
    }
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn inline_paint_carries_colours_decorations_and_links_without_crossing_siblings() {
    use exact_terminal::{
        grid::Rgb,
        host::{Host, Mode},
    };
    let plan = contract::compile(r##"component App
  state changed = false
  action change
    changed = true
  view
    column color=(changed ? "#abcdef" : "light-dark(#112233, #334455)") text-decoration-line="underline"
      text href="https://paragraph.invalid"
        text "A" testId="a"
        text ""
        text color="#445566" background-color="#778899" text-decoration-line="line-through" font-style="italic"
          text "B" font-weight=700 href="https://leaf.invalid" testId="b"
          text "C" color="inherit" background-color="transparent" text-decoration-line="none" testId="c"
          text "D" color="#aabbcc" background-color="#ddeeff" font-weight=300 href="bad\nlink" testId="d"
        text "E" testId="e"
      text "F" opacity=0.5
"##).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 20, 5).unwrap();
    for (changed, dark, inherited) in [
        (false, false, Rgb(0x11, 0x22, 0x33)),
        (false, true, Rgb(0x33, 0x44, 0x55)),
        (true, true, Rgb(0xab, 0xcd, 0xef)),
    ] {
        if changed {
            host.act("change");
        }
        host.dark = dark;
        let painted = host.render(0, 5);
        let grid = &painted.grid;
        assert!(grid.text().starts_with("ABCDE"), "{}", grid.text());
        let ink = [
            inherited,
            Rgb(0x44, 0x55, 0x66),
            Rgb(0x44, 0x55, 0x66),
            Rgb(0xaa, 0xbb, 0xcc),
            inherited,
        ];
        for (x, ink) in ink.into_iter().enumerate() {
            let style = grid.cell(x, 0).style;
            assert_eq!(style.fg, Some(ink));
            assert!(style.underline);
            assert_eq!(style.strike, (1..=3).contains(&x));
            assert_eq!(style.italic, (1..=3).contains(&x));
            assert_eq!(style.bold, x == 1);
            assert_eq!(style.faint, x == 3);
        }
        assert_eq!(grid.cell(1, 0).style.bg, Some(Rgb(0x77, 0x88, 0x99)));
        assert_eq!(grid.cell(2, 0).style.bg, Some(Rgb(0x77, 0x88, 0x99)));
        assert_eq!(grid.cell(3, 0).style.bg, Some(Rgb(0xdd, 0xee, 0xff)));
        assert_eq!(grid.cell(4, 0).style.bg, None);
        assert!(grid.cell(0, 1).style.faint);
        let paragraph_link = grid.cell(0, 0).style.link;
        assert_ne!(paragraph_link, 0);
        assert_ne!(grid.cell(1, 0).style.link, paragraph_link);
        assert_eq!(grid.cell(2, 0).style.link, paragraph_link);
        assert_eq!(grid.cell(3, 0).style.link, 0);
        assert_eq!(grid.cell(4, 0).style.link, paragraph_link);
        assert!(grid.ansi().contains("https://leaf.invalid"));
        assert!(!grid.ansi().contains("bad\nlink"));
        for (x, label) in ["a", "b", "c", "d", "e"].into_iter().enumerate() {
            let id = host.by_test_id(label).unwrap();
            assert!(painted
                .runs
                .iter()
                .any(|(at, rect)| *at == id && rect.contains(x as i32, 0)));
        }
    }
}

fn run(name: &str, ops: &[&str]) -> (bool, String, String) {
    run_in("--inline", name, ops)
}

fn run_in(mode: &str, name: &str, ops: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_exact-terminal"))
        .arg(entry(name))
        .args(["--size", "40x12", mode])
        .args(ops)
        .output()
        .expect("runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn wheel_and_tap_refuse_what_is_not_on_the_screen() {
    let (ok, out, err) = run("wheel", &["tap", "open", "wheel", "list", "2", "print"]);
    assert!(ok, "{err}");
    assert!(out.contains("delta") && !out.contains("alpha"), "{out}");
    // `epsilon` is below the list's three rows: neither verb reaches it.
    let (ok, _, err) = run("wheel-off", &["tap", "open", "wheel", "epsilon", "1"]);
    assert!(!ok && err.contains("not on the screen"), "{err}");
    let (ok, _, err) = run("tap-off", &["tap", "open", "tap", "epsilon"]);
    assert!(!ok && err.contains("not on the screen"), "{err}");
}

#[test]
fn a_verb_that_cannot_do_its_work_fails() {
    for ops in [
        &["tap", "open", "wheel", "list", "lots"][..],
        &["resize", "soon"][..],
        &["screenshot", "/no/such/dir/x.txt"][..],
    ] {
        let (ok, _, err) = run(&ops.join("-").replace('/', ""), ops);
        assert!(!ok, "{ops:?} succeeded: {err}");
    }
}

/// A native field in a terminal entry is one row with a host-painted fill,
/// no border (LLP 1104 D7), and a disabled one is faint.
#[test]
fn a_terminal_field_is_a_filled_row() {
    let (ok, out, err) = run("field", &["print"]);
    assert!(ok, "{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[1].trim_end(), "new task", "{out}");
    assert_eq!(lines[2].trim_end(), "locked", "{out}");
    assert!(
        !out.contains('╭') && !out.contains('│'),
        "a field drew a border:\n{out}"
    );
    let shot = entry("field").with_file_name("field.ans");
    // Full screen, the screenshot is the grid, faint included.
    let (ok, _, err) = run_in(
        "--fullscreen",
        "field",
        &["--color", "truecolor", "screenshot", shot.to_str().unwrap()],
    );
    assert!(ok, "{err}");
    let ans = std::fs::read_to_string(&shot).unwrap();
    // The host's fill: #303030 behind the placeholder, without compiled rows.
    assert!(ans.contains("48;2;48;48;48"), "{ans:?}");
    let before = &ans[..ans.find("locked").unwrap()];
    let sgr = &before[before.rfind("\x1b[").unwrap()..];
    let params: Vec<&str> = sgr[2..sgr.find('m').unwrap()].split(';').collect();
    assert!(params[..2].contains(&"2"), "not faint: {sgr:?}");
}

#[test]
fn native_cell_look_keeps_bare_fields_and_authored_ink() {
    use exact_terminal::{
        grid::Rgb,
        host::{Host, Mode},
    };
    let plan = contract::compile_path_terminal(&entry("look")).unwrap();
    let host = Host::boot(plan, (), Mode::Fullscreen, 40, 12).unwrap();
    assert!(host.kernel().env().control_text_styles.is_some());
    let field = host
        .kernel()
        .node_by_key(host.kernel().find_by_test_id("field")[0])
        .unwrap();
    assert!(!field.style.mask.has(exact_kernel::StyleId::BackgroundColor));
    assert_eq!(host.kernel().provisional_layouts(), 0);

    let plan = contract::compile(
        r##"component App
  view
    column color="#ff0000" font-size=32 font-style="italic" font-weight=700 letter-spacing=4
      input width="10ch" value="native" testId="native"
      input width="10ch" value="authored" color="#00ff00"
      input width="10ch" value="bare" appearance="none" disabled=true
      textarea width="10ch" rows=2 value="two\nlines"
"##,
    )
    .unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 12).unwrap();
    let n = host
        .kernel()
        .node_by_key(host.kernel().find_by_test_id("native")[0])
        .unwrap();
    let s = n.computed_style(exact_kernel::StyleMask::INHERITED);
    assert_eq!(s.font_family, 5);
    assert_eq!(s.font_size, 16.0);
    assert_eq!(s.font_weight, 400);
    assert_eq!(s.font_style, exact_kernel::FontStyle::Normal);
    assert_eq!(s.letter_spacing, 0.0);
    assert_eq!(n.text_color().resolve(false).g(), 0);
    for dark in [false, true] {
        host.dark = dark;
        let painted = host.render(0, 12);
        let grid = &painted.grid;
        let fill = if dark {
            Rgb(48, 48, 48)
        } else {
            Rgb(228, 228, 228)
        };
        let ink = if dark {
            Rgb(255, 255, 255)
        } else {
            Rgb(0, 0, 0)
        };
        assert_eq!(grid.cell(0, 0).style.fg, Some(ink));
        assert_eq!(
            grid.cell(9, 0).style.bg,
            Some(fill),
            "empty cells have fill"
        );
        assert_eq!(grid.cell(0, 1).style.fg, Some(Rgb(0, 255, 0)));
        assert_eq!(grid.cell(0, 2).style.fg, Some(Rgb(255, 0, 0)));
        assert_eq!(grid.cell(0, 2).style.bg, None);
        assert!(
            !grid.cell(0, 2).style.faint,
            "bare disabled paint stays authored"
        );
        assert_eq!(
            grid.cell(9, 4).style.bg,
            Some(fill),
            "textarea's second row"
        );
    }
}
