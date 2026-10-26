//! Host-assigned Neutral typography follows the device preference in measure and paint.
use exact_kernel::ViewId;
use exact_linux::image::Bitmap;
use exact_linux::paint::{Backend, GradientPaint, Rect4, Shape};
use exact_linux::presenter::{Device, PainterChoice, Presenter};
use exact_linux::text::{Paragraph, RunPaint, TextEngine};
use std::{cell::RefCell, path::PathBuf, sync::Arc};
use tiny_skia::{Pixmap, Transform};

thread_local! {
    static TEXTS: RefCell<Vec<(String, u16, f32)>> = const { RefCell::new(Vec::new()) };
}

struct Texts;
impl Backend for Texts {
    fn name(&self) -> &'static str {
        "text-observer"
    }
    fn begin(&mut self, _: f32, _: f32, _: f32) {
        TEXTS.with_borrow_mut(Vec::clear);
    }
    fn fill(&mut self, _: &Shape, _: [u8; 4], _: Transform) {}
    fn fill_gradient(&mut self, _: &Shape, _: &GradientPaint, _: Transform) {}
    fn fill_border(&mut self, _: &exact_linux::paint::border::BorderFill, _: Transform) {}
    fn image(&mut self, _: &Arc<Bitmap>, _: Rect4, _: &[Shape], _: Transform, _: Option<[u8; 4]>) {}
    fn text(
        &mut self,
        _: &mut TextEngine,
        p: &Paragraph,
        _: &[RunPaint],
        _: (f32, f32),
        _: Transform,
    ) {
        TEXTS.with_borrow_mut(|texts| {
            for run in p.runs() {
                texts.push((run.text.clone(), run.weight, p.width));
            }
        });
    }
    fn push_clip(&mut self, _: &Shape, _: Transform) {}
    fn pop_clip(&mut self) {}
    fn push_opacity(&mut self, _: f32) {}
    fn pop_opacity(&mut self) {}
    fn pointer(&mut self, _: f32, _: f32) {}
    fn finish(&mut self) -> Result<Pixmap, String> {
        Ok(Pixmap::new(400, 500).unwrap())
    }
}

fn boot(source: &str, painter: PainterChoice) -> Presenter<()> {
    static SETUP: std::sync::Once = std::sync::Once::new();
    SETUP.call_once(|| {
        exact_linux::app::show_neutral_look();
        std::env::set_var("EXACT_FONT", "Roboto Flex");
        std::env::set_var(
            "EXACT_FONTS",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fonts"),
        );
        exact_linux::presenter::set_custom_painter("text-observer", || Ok(Box::new(Texts)));
    });
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        (),
        (400., 500.),
        1.,
        PathBuf::new(),
        painter,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn prefer(p: &mut Presenter<()>, bold_text: bool) {
    let device = Device {
        night: false,
        font_scale: 1.,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text,
        high_contrast: false,
        touch: false,
        tv: false,
        touch_slop: 0.,
        dates: String::new(),
    };
    assert!(p.set_device(&device).is_none());
    p.frame();
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

#[test]
fn bold_text_steps_file_words_and_button_defaults_once() {
    for (family, steps) in [("system-ui", true), ("Georgia, serif", false)] {
        let mut p = boot(
            &format!(
                r#"component App
  view
    column align-items="flex-start" font-family="{family}"
      input type="file" testId="file"
      button appearance="auto"
        text "Save"
"#
            ),
            PainterChoice::Custom,
        );
        let mut widths = Vec::new();
        let mut chrome = Vec::new();
        for bold in [false, true, false] {
            prefer(&mut p, bold);
            let texts = TEXTS.with_borrow(Clone::clone);
            let word = |s: &str| texts.iter().find(|t| t.0 == s).unwrap();
            let expected = if bold && steps { 700 } else { 500 };
            assert_eq!(word("Choose File").1, expected, "{family}, bold={bold}");
            assert_eq!(word("Save").1, expected, "button stepped once");
            let frame = p.host().kernel().node(id(&p, "file")).unwrap().frame;
            chrome.push(frame.width - word("Choose File").2 - word("No file chosen").2);
            widths.push(word("Choose File").2);
        }
        assert_eq!(
            chrome[0], chrome[1],
            "measured width follows both painted labels"
        );
        assert_eq!(chrome[0], chrome[2]);
        assert_eq!(widths[0], widths[2]);
        assert_eq!(widths[0] != widths[1], steps, "button word width, {family}");
    }
}

#[test]
fn bold_text_alert_title_measure_matches_paint_and_restores() {
    for (family, steps) in [("system-ui", true), ("Georgia, serif", false)] {
        let source = format!(
            r#"component App
  view
    column font-family="{family}"
      button commandfor="confirm" command="show-modal" testId="open"
        text "Open"
      dialog id="confirm" testId="dialog" role="alertdialog" aria-label="MMMM MMMM MMMM MMMM MMMM" width=210
        text "Keep these changes?"
        button commandfor="confirm" command="close"
          text "Cancel"
"#
        );
        let mut p = boot(&source, PainterChoice::Cpu);
        p.tap(id(&p, "open")).unwrap();
        p.run_commands(|| ());
        let dialog = id(&p, "dialog");
        let mut sizes = Vec::new();
        for bold in [false, true, false] {
            prefer(&mut p, bold);
            let title = p.paragraph(dialog).expect("painted alert title");
            assert_eq!(
                title.runs()[0].weight,
                if bold && steps { 800 } else { 600 }
            );
            let frame = p.host().kernel().alert_prefix_frame(dialog).unwrap();
            assert_eq!(
                frame.height, title.height,
                "measured title wraps as painted"
            );
            let advances: f32 = title
                .layout_runs()
                .flat_map(|line| line.glyphs.iter())
                .map(|g| g.w)
                .sum();
            sizes.push((advances, title.height));
        }
        assert_eq!(sizes[0], sizes[2]);
        assert_eq!(
            sizes[0] != sizes[1],
            steps,
            "title width or wrapping, {family}"
        );
    }
}
