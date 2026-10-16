//! A retained region's paint holds no wide colour id (LLP 1121 stage 4): it
//! is settled at capture, so it paints the same after the table has swept
//! the colours it was captured from, and still flips scheme.
use super::*;
use crate::content_region::ContentRegionRegistration;
use exact_kernel::ColorValue;
use std::time::Instant;

const APP: &str = r#"component App
  state title = "old picture"
  state ink = "light-dark(oklch(0.55 0.15 30), oklch(0.75 0.1 250))"
  state ramp = "linear-gradient(90deg, oklch(0.6 0.12 140), oklch(0.5 0.1 300))"
  state mark = "light-dark(oklch(0.6 0.1 60), oklch(0.7 0.1 160))"
  action replace
    title = "new picture"
    ink = "light-dark(oklch(0.5 0.1 120), oklch(0.8 0.05 200))"
    ramp = "linear-gradient(90deg, oklch(0.4 0.1 20), oklch(0.7 0.1 90))"
    mark = "oklch(0.45 0.12 330)"
  view
    column width=400 height=500
      button press=replace testId="replace" height=32
        text "replace"
      view id="owner" width=400 height=300 overflow-x="hidden" overflow-y="hidden"
        scroll id="content" testId="scroll" width=400 height=300
          view height=600
            text title testId="paragraph" font-size=40 font-weight=900 color=ink
            view testId="ramp" width=200 height=40 background-image=ramp
            svg testId="mark" width=40 height=40 viewBox="0 0 40 40"
              rect width=40 height=40 fill=mark
        text "Preparing content" id="pending" position="absolute"
"#;

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    p.host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id(name)[0])
        .unwrap()
        .id
}

fn ready(p: &mut Presenter<()>) {
    let end = Instant::now() + Duration::from_secs(90);
    while !p.host.content_region().unwrap().receipt().unwrap().current {
        assert!(Instant::now() < end, "font/publication watchdog");
        assert!(p.host.content_region().unwrap().refusal().is_none());
        std::thread::sleep(Duration::from_millis(1));
        assert!(p.poll_content_region().is_none());
    }
}

fn wide(css: &str) -> u16 {
    match ColorValue::parse_light_dark(css) {
        Some(ColorValue::Wide(id)) => id,
        other => panic!("{css}: {other:?}"),
    }
}

fn has(frame: &[u8], rgba: [u8; 4]) -> bool {
    frame.chunks_exact(4).any(|px| px == rgba)
}

#[test]
fn a_retained_region_paints_after_its_wide_colours_are_swept() {
    // Its own process: it fills the process's wide table.
    const CHILD: &str = "EXACT_WIDE_REGION_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "presenter::content_region_wide_tests::a_retained_region_paints_after_its_wide_colours_are_swept",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let _service = crate::content_region::test_service();
    let (mut p, error) = Presenter::boot_with_content_region(
        &crate::test_plan(APP),
        (),
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let original = p.frame();
    let ink = "light-dark(oklch(0.55 0.15 30), oklch(0.75 0.1 250))";
    let first = [
        wide(ink),
        wide("oklch(0.6 0.12 140)"),
        wide("light-dark(oklch(0.6 0.1 60), oklch(0.7 0.1 160))"),
    ];
    let light_ink = crate::paint::rgba(ColorValue::Wide(first[0]).resolve(false));
    let [mark, dark_mark] =
        [false, true].map(|dark| crate::paint::rgba(ColorValue::Wide(first[2]).resolve(dark)));
    assert!(has(original.data(), light_ink), "the paragraph's ink");
    assert!(has(original.data(), mark), "the svg's fill");

    // B replaces every colour, and waits at its text: A stays on screen,
    // and no kernel holds A's colours any more.
    let gate = crate::content_region::test_hooks::next_text();
    assert!(p
        .host
        .dispatch_at(id(&p, "replace"), Event::Press, 1.)
        .is_none());
    assert!(p.after_commit().is_none());
    gate.entered();
    // The kernel lets go of A's styles, as it does when content settles.
    p.host.runner_mut().kernel_mut().trim();
    // Enough new colours, held by nothing, that the table sweeps past
    // every recent intern: parsed on a thread with no holder, as this
    // thread's would be a batch, kept until its kernel next commits.
    std::thread::spawn(|| {
        for i in 0..4096 {
            let _ = ColorValue::parse_light_dark(&format!(
                "color(srgb {} {} 0.5)",
                (i % 64) as f64 / 64.0,
                (i / 64) as f64 / 64.0
            ));
        }
    })
    .join()
    .unwrap();
    assert!(exact_kernel::style::wide::used() < 4096);
    // A's colours were reclaimed: interned again, each is another id.
    assert_ne!(wide(ink), first[0]);
    assert_ne!(wide("oklch(0.6 0.12 140)"), first[1]);
    assert_ne!(
        wide("light-dark(oklch(0.6 0.1 60), oklch(0.7 0.1 160))"),
        first[2]
    );
    let held = p.frame();
    assert_eq!(
        original.data(),
        held.data(),
        "A's retained paint, settled at capture, paints as it did"
    );

    // A picture not yet replaced is shown as captured through a scheme
    // flip, as before; its svg's wide pair, settled to an 8-bit pair, still
    // flips.
    p.brush.dark = true;
    assert!(p.host.content_region_appearance(true).is_none());
    let flipped = p.frame();
    assert!(has(flipped.data(), dark_mark), "the svg's dark fill");
    assert!(!has(flipped.data(), mark));
    p.brush.dark = false;
    assert!(p.host.content_region_appearance(false).is_none());

    // B lands. A scheme flip repaints its text from the pair its source
    // settled at capture (no current node is read), its gradient and svg
    // captured again for the new appearance.
    drop(gate);
    ready(&mut p);
    let b_ink = "light-dark(oklch(0.5 0.1 120), oklch(0.8 0.05 200))";
    let [b_light, b_dark] = [false, true]
        .map(|dark| crate::paint::rgba(ColorValue::parse_light_dark(b_ink).unwrap().resolve(dark)));
    let light = p.frame();
    assert!(has(light.data(), b_light), "B's ink");
    p.brush.dark = true;
    assert!(p.host.content_region_appearance(true).is_none());
    let dark = p.frame();
    assert!(p.last_frame_succeeded);
    assert!(has(dark.data(), b_dark), "B's dark ink");
    assert!(!has(dark.data(), b_light));
    drop(p);
}
