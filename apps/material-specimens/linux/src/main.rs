//! The specimen door (LLP 1115.001.001 D4): each specimen of the Material
//! oracle's manifest, drawn by the Linux presenter as Android's reader
//! would show it. Not the neutral look; the scheme's palette and a phone's
//! settings told before the first frame; the top app bar and the navigation
//! bar painted as the reader's own views and laid over the page where those
//! views would be. Beside each picture it writes what the oracle compares:
//! the specimen's layout box and its texts' boxes, in dp from the window's
//! corner.
//!
//!     material-specimens <specimens.json> <specimens dir> <reference dir> <out dir>
//!
//! The reference directory is the Compose side's: its `schemes.json` gives
//! the colours, and its pictures the window's size in pixels, so that both
//! sides round dp to pixels alike. `EXACT_FONTS` and `EXACT_FONT` name the
//! reference's fonts (the oracle sets them).

use exact_kernel::PropId;
use exact_linux::paint::Backend;
use exact_linux::presenter::top::SurfacePainter;
use exact_linux::presenter::{Device, PainterChoice, Palette, Presenter, Window, MATERIAL_ROLES};
use exact_linux::raster::Raster;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

/// The palette door's roles, in the order it takes them (`presenter::Material`).
const ROLES: [&str; MATERIAL_ROLES] = [
    "primary",
    "onPrimary",
    "secondaryContainer",
    "surface",
    "onSurface",
    "onSurfaceVariant",
    "surfaceContainerLow",
    "surfaceContainer",
    "surfaceContainerHigh",
    "surfaceContainerHighest",
    "outlineVariant",
    "error",
    "primaryContainer",
    "onPrimaryContainer",
    "inversePrimary",
    "secondary",
    "onSecondary",
    "onSecondaryContainer",
    "tertiary",
    "onTertiary",
    "tertiaryContainer",
    "onTertiaryContainer",
    "background",
    "onBackground",
    "surfaceVariant",
    "surfaceTint",
    "inverseSurface",
    "inverseOnSurface",
    "onError",
    "errorContainer",
    "onErrorContainer",
    "outline",
    "scrim",
    "surfaceBright",
    "surfaceDim",
    "surfaceContainerLowest",
];

/// A bar's texts: the node each stands for, and its box in the bar's points.
type BarTexts = Vec<(exact_kernel::ViewId, (f32, f32, f32, f32))>;

/// A shell's painter that keeps each picture it records, as the reader's
/// views keep theirs.
struct Keep(Rc<RefCell<Vec<Pixmap>>>);

impl SurfacePainter for Keep {
    fn record(
        &mut self,
        size: (f32, f32),
        scale: f32,
        paint: &mut dyn FnMut(&mut dyn Backend),
    ) -> Vec<u32> {
        let mut raster = Raster::new();
        raster.begin(size.0, size.1, scale);
        paint(&mut raster);
        let pixels = raster.finish().expect("a bar's picture");
        let words = vec![pixels.width(), pixels.height()];
        self.0.borrow_mut().push(pixels);
        words
    }
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [manifest, specimens, reference, out] = &args[..] else {
        eprintln!(
            "usage: material-specimens <specimens.json> <specimens dir> <reference dir> <out dir>"
        );
        std::process::exit(2);
    };
    // The look the reader shows: Material's column of the schema's metrics.
    exact_linux::app::show_material_look();
    let manifest = read_json(manifest);
    let schemes = read_json(&reference.join("schemes.json"));
    let density = manifest["density"].as_f64().expect("density") as f32;
    std::fs::create_dir_all(out).expect("the out directory");
    let mut failed = false;
    for specimen in manifest["specimens"].as_array().expect("specimens") {
        let id = specimen["id"].as_str().expect("an id");
        for scheme in manifest["schemes"].as_array().expect("schemes") {
            let scheme = scheme.as_str().expect("a scheme");
            let name = format!("{id}.{scheme}");
            let window = png_size(&reference.join("compose").join(format!("{name}.png")));
            let source = std::fs::read_to_string(specimens.join(format!("{id}.contract")))
                .unwrap_or_else(|e| panic!("{id}.contract: {e}"));
            match draw(id, &source, &schemes, scheme, window, density) {
                Ok((picture, facts)) => {
                    picture
                        .save_png(out.join(format!("{name}.png")))
                        .expect("write the picture");
                    let text = serde_json::to_string_pretty(&facts).unwrap();
                    std::fs::write(out.join(format!("{name}.json")), text)
                        .expect("write the facts");
                }
                Err(why) => {
                    eprintln!("{name}: {why}");
                    failed = true;
                }
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn draw(
    id: &str,
    source: &str,
    schemes: &Value,
    scheme: &str,
    window_px: (u32, u32),
    density: f32,
) -> Result<(Pixmap, Value), String> {
    let palette = Palette {
        light: roles(&schemes["light"])?,
        dark: roles(&schemes["dark"])?,
    };
    // Told before boot, so everything boot resolves is the scheme's (as the
    // reader tells its device's palette).
    palette.tell();
    let plan = contract::compile(source)
        .map_err(|e| format!("compile: {e:?}"))?
        .encode();
    let size = (window_px.0 as f32 / density, window_px.1 as f32 / density);
    let (mut p, error) =
        Presenter::boot_with(&plan, (), size, density, PathBuf::new(), PainterChoice::Cpu)
            .map_err(|e| format!("boot: {e:?}"))?;
    if let Some(error) = error {
        return Err(format!("boot: {error}"));
    }
    let bars = Rc::new(RefCell::new(Vec::new()));
    p.serve_requests();
    p.serve_windows(Box::new(Keep(Rc::new(RefCell::new(Vec::new())))));
    if let Some(e) = p.serve_bars(Box::new(Keep(bars.clone()))) {
        return Err(format!("bars: {e}"));
    }
    let window = Window {
        size,
        bars: [0.0; 4],
        cutout: [0.0; 4],
        keyboard: [0.0; 4],
        gestures: [0.0; 4],
    };
    if let Some(e) = p.set_window(window) {
        return Err(format!("window: {e}"));
    }
    let device = Device {
        night: scheme == "dark",
        font_scale: 1.0,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text: false,
        high_contrast: false,
        touch: true,
        touch_slop: 8.0,
        dates: String::new(),
    };
    if let Some(e) = p.set_device(&device) {
        return Err(format!("device: {e}"));
    }
    // A specimen of a pushed route is pushed by its `push` control, and
    // settles before what follows.
    // The clock is absolute: what follows settles from where this left it.
    let mut now = 0.0;
    if let Some(push) = node(&p, "push") {
        p.tap(push)?;
        now = 1000.0;
        p.clock(now);
    }
    let specimen = node(&p, "specimen");
    if id.ends_with("-focused") {
        p.tap(specimen.ok_or("no specimen to focus")?)?;
    }
    // As the reference: 300 ms on, a focus and a label's float settled and a
    // caret short of its first blink.
    p.clock(now + 300.0);
    let mut picture = (*p.frame()).clone();
    // The reader asks for the bars' pictures with each stream; this asks once.
    p.reader_bars(density);
    // Each bar's frame and its texts, held apart from the presenter they borrow.
    let shown: Vec<([f32; 4], BarTexts)> = {
        let (top, bottom) = p.bars();
        [top, bottom]
            .into_iter()
            .flatten()
            .map(|b| (b.frame, b.texts.clone()))
            .collect()
    };
    let bar_frames: Vec<[f32; 4]> = shown.iter().map(|(frame, _)| *frame).collect();
    let mut texts = Vec::new();
    for (frame, items) in &shown {
        for (view, (x, y, w, h)) in items {
            texts.push(
                json!({"text": said(&p, *view), "box": rect(frame[0] + x, frame[1] + y, *w, *h)}),
            );
        }
    }
    for frame in &bar_frames {
        let want = (
            (frame[2] * density).round() as u32,
            (frame[3] * density).round() as u32,
        );
        let pictures = bars.borrow();
        let picture_of = pictures
            .iter()
            .rev()
            .find(|px| (px.width(), px.height()) == want);
        let Some(bar) = picture_of else {
            let sizes: Vec<_> = pictures
                .iter()
                .map(|px| (px.width(), px.height()))
                .collect();
            return Err(format!(
                "no picture of the bar at {frame:?}, {want:?}; recorded {sizes:?}"
            ));
        };
        let at =
            Transform::from_translate((frame[0] * density).round(), (frame[1] * density).round());
        picture.draw_pixmap(0, 0, bar.as_ref(), &PixmapPaint::default(), at, None);
    }
    let layout = match id {
        "top-app-bar" | "top-app-bar-up" => bar_frames.first().copied(),
        "navigation-bar" => bar_frames.last().copied(),
        _ => specimen
            .and_then(|s| p.rect_of(s))
            .map(|(x, y, w, h)| [x, y, w, h]),
    }
    .ok_or("the specimen has no box")?;
    if let Some(label) = node(&p, "label") {
        if let Some((x, y, w, h)) = p.rect_of(label) {
            texts.push(json!({"text": said(&p, label), "box": rect(x, y, w, h)}));
        }
    }
    // A field's label and value are painted, not nodes: the presenter says
    // where it put them.
    if let Some(field) = specimen.and_then(|s| p.field_texts(s).map(|t| (s, t))) {
        let (view, painted) = field;
        let kernel = p.host().kernel();
        let props = &kernel.node(view).ok_or("no field")?.props;
        let said = [
            (painted.label, props.str(PropId::Placeholder)),
            (painted.value, props.str(PropId::Value)),
        ];
        for (text, words) in said {
            let (Some(text), Some(words)) = (text, words) else {
                continue;
            };
            let (x, y, w, h) = text.rect;
            let line = json!({"top": y, "bottom": y + h, "baseline": text.baseline});
            texts.push(json!({"text": words, "box": rect(x, y, w, h), "lines": [line]}));
        }
    }
    let facts = json!({
        "id": id, "scheme": scheme, "density": density,
        "layout": rect(layout[0], layout[1], layout[2], layout[3]),
        "texts": texts,
    });
    Ok((picture, facts))
}

/// What a node says: its text, or its first descendant's (a bar's text
/// stands for the tab or the heading that holds it).
fn said(p: &Presenter<()>, view: exact_kernel::ViewId) -> Option<String> {
    let kernel = p.host().kernel();
    let node = kernel.node(view)?;
    if let Some(text) = node.props.str(PropId::Text) {
        return Some(text.to_owned());
    }
    node.children().into_iter().find_map(|child| said(p, child))
}

/// The node a `testId` names, if one does.
fn node(p: &Presenter<()>, test_id: &str) -> Option<exact_kernel::ViewId> {
    let kernel = p.host().kernel();
    let key = *kernel.find_by_test_id(test_id).first()?;
    kernel.node_by_key(key).map(|n| n.id)
}

fn roles(scheme: &Value) -> Result<[u32; MATERIAL_ROLES], String> {
    let mut out = [0; MATERIAL_ROLES];
    for (slot, name) in out.iter_mut().zip(ROLES) {
        *slot = scheme[name]
            .as_u64()
            .ok_or(format!("schemes.json has no {name}"))? as u32;
    }
    Ok(out)
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Value {
    json!({"x": x, "y": y, "w": w, "h": h})
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A PNG's width and height, from its header.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let word = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (word(16), word(20))
}
