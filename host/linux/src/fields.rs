//! A text field the platform hosts (LLP 1115.001 D8). On Android an `input`
//! or a `textarea` is the framework's own `EditText`, laid over the box this
//! host paints for it: its text, its caret, its selection and its keyboard
//! are the platform's, and inside the field the text engine is the
//! `EditText`'s, not this host's. So the field's text is measured by the
//! class that draws it: during layout the kernel asks its measurer
//! ([`exact_kernel::TextMeasurer::field_text`]), which asks here, and the
//! shell answers at once ([`FieldMetrics`]). Nothing of it goes through the
//! shell's queue of requests, which is taken after the frame.
//!
//! Desktop Linux hosts nothing: no shell attaches, [`hosting`] is `None`,
//! and a field is measured, painted and typed into as it always was
//! (`paint/field.rs`, `presenter/field.rs`). Android's reader attaches when
//! it boots the host ([`host`], from `canvas/field.rs`), which is the one
//! thing that tells the two apart.
//!
//! A Markdown field (`markup="markdown"`) is never hosted: an `EditText`
//! shows plain text, and that editor keeps the painted field it has.
use crate::text::{Run, RunPaint, Spec, TextEngine};
use exact_kernel::{AxisOffer, FieldKind, NodeRef, PropId, TextMeasureRequest, TextMetrics};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// The type a hosted field is set in, in the shell's own units.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldFace {
    /// The font's size, pixels.
    pub size: f32,
    /// CSS weight, 100 to 900.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// A `textarea`: its text wraps and takes lines.
    pub multiline: bool,
    /// Tracking, in ems.
    pub spacing: f32,
    /// The line's height in pixels, or 0 for the font's own.
    pub line: f32,
    /// The face's file and its index in a collection (`path#index`), or
    /// empty for the device's default family.
    pub family: String,
}

/// The shell's own text field, asked during layout.
pub trait FieldMetrics: Sync {
    /// `text` as the platform's field sets it in `face`: its width, its
    /// height and its first baseline, in pixels. A `textarea`'s lines wrap
    /// at `width` pixels; `None` is as long as they are, and a single line
    /// never wraps.
    fn measure(&self, face: &FieldFace, text: &str, width: Option<f32>) -> [f32; 3];
}

/// A shell that hosts text fields, and its pixels a point.
#[derive(Clone, Copy)]
pub struct Hosting {
    /// The shell's field, measured.
    pub metrics: &'static dyn FieldMetrics,
    /// Pixels a point.
    pub scale: f32,
}

/// The process's shell: a host may boot on one thread and run on another.
static SHELL: std::sync::Mutex<Option<Hosting>> = std::sync::Mutex::new(None);

thread_local! {
    /// A shell for this thread alone (a test's), over the process's.
    static HERE: Cell<Option<Hosting>> = const { Cell::new(None) };
    /// The file each face resolves to, by family, weight and slant.
    static FACES: RefCell<HashMap<(u16, u16, bool), String>> = RefCell::new(HashMap::new());
}

/// The process's shell hosts text fields from now on, at `scale` pixels a
/// point: before the host boots, so its first layout asks it.
pub fn host(metrics: &'static dyn FieldMetrics, scale: f32) {
    *SHELL.lock().unwrap_or_else(|e| e.into_inner()) = Some(Hosting { metrics, scale });
}

/// The shell's pixels a point changed (a new display density).
pub fn rescale(scale: f32) {
    if let Some(hosting) = SHELL.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        hosting.scale = scale;
    }
    HERE.with(|here| here.set(here.get().map(|h| Hosting { scale, ..h })));
}

/// A shell that hosts text fields on this thread until the guard drops: a
/// test's stand-in for Android's reader.
#[cfg(test)]
pub(crate) fn host_here(metrics: &'static dyn FieldMetrics, scale: f32) -> impl Drop {
    struct Until;
    impl Drop for Until {
        fn drop(&mut self) {
            HERE.with(|here| here.set(None));
        }
    }
    HERE.with(|here| here.set(Some(Hosting { metrics, scale })));
    Until
}

/// The shell that hosts this host's text fields, if one does.
pub(crate) fn hosting() -> Option<Hosting> {
    HERE.with(Cell::get)
        .or_else(|| *SHELL.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Whether the shell hosts `node`, a text field: every one but a Markdown
/// editor, on a host whose shell hosts any.
pub(crate) fn hosts(node: &NodeRef<'_>) -> bool {
    node.node_type == exact_kernel::NodeType::TextInput
        && node.props.str(PropId::Markup) != Some("markdown")
        && hosting().is_some()
}

/// The face `style` sets a field in, as the shell is told it: sizes in its
/// pixels, and the file this host's catalog resolves the family to (the
/// device's default family is left to the shell).
pub(crate) fn face(
    engine: &mut TextEngine,
    style: &exact_kernel::TextStyle,
    multiline: bool,
    scale: f32,
) -> FieldFace {
    let italic = style.font_style != exact_kernel::FontStyle::Normal;
    let size = style.font_size.max(0.0);
    FieldFace {
        size: size * scale,
        weight: style.font_weight,
        italic,
        multiline,
        spacing: if size > 0.0 {
            style.letter_spacing / size
        } else {
            0.0
        },
        line: style.line_height.map_or(0.0, |h| (h * scale).max(0.0)),
        family: family(engine, style, italic),
    }
}

/// The file `style`'s family resolves to here, with its collection index
/// (`path#index`); empty for the system's default family (stack 0), which
/// is the device's own on the shell, and for a face that has no file.
fn family(engine: &mut TextEngine, style: &exact_kernel::TextStyle, italic: bool) -> String {
    if style.font_family == 0 {
        return String::new();
    }
    let key = (style.font_family, style.font_weight, italic);
    if let Some(known) = FACES.with(|f| f.borrow().get(&key).cloned()) {
        return known;
    }
    // One glyph shaped in the face: the catalog names the file it came from.
    let probe = exact_kernel::TextStyle {
        font_size: 16.0,
        line_height: None,
        letter_spacing: 0.0,
        ..*style
    };
    let spec = Spec {
        strut: Run::from_style("", probe),
        runs: vec![Run::from_style("0", probe)],
        align: exact_kernel::TextAlign::Left,
        line_clamp: 0,
        overflow_wrap: Default::default(),
        white_space: Default::default(),
        direction: exact_kernel::Direction::Ltr,
        text_indent: 0.0,
        text_box_trim: Default::default(),
        text_box_edge: Default::default(),
    };
    let paragraph = engine.paragraph(&spec, None);
    let paint = [RunPaint {
        color: [0, 0, 0, 0xff],
        source: 0,
    }];
    let file = engine
        .glyph_runs(&paragraph, &paint)
        .into_iter()
        .find_map(|run| run.file)
        .map_or_else(String::new, |(path, index)| format!("{path}#{index}"));
    FACES.with(|f| f.borrow_mut().insert(key, file.clone()));
    file
}

/// A hosted field's text, measured by the shell's own field: what
/// [`exact_kernel::TextMeasurer::field_text`] answers on a host whose shell
/// hosts fields. `None` where none does, and for a Markdown editor.
pub(crate) fn measure(
    engine: &mut TextEngine,
    kind: FieldKind,
    request: &TextMeasureRequest<'_>,
) -> Option<TextMetrics> {
    let hosting = hosting()?;
    if request.paragraph.markup == exact_kernel::text::Markup::Markdown {
        return None;
    }
    let style = request.runs.first()?.style;
    let multiline = kind == FieldKind::Textarea;
    // The kernel holds a textarea's last, empty line open with a zero-width
    // space; the platform's field shows that line itself.
    let text: String = request
        .runs
        .iter()
        .map(|run| &*run.text)
        .filter(|text| *text != "\u{200b}")
        .collect();
    let scale = hosting.scale;
    let face = face(engine, &style, multiline, scale);
    let width = match request.width {
        AxisOffer::Definite(w) if multiline => Some((w * scale).max(0.0)),
        _ => None,
    };
    let [w, h, baseline] = hosting.metrics.measure(&face, &text, width);
    let metrics = TextMetrics {
        width: w / scale,
        height: h / scale,
        first_baseline: Some(baseline / scale),
    };
    let sane = |v: f32| v.is_finite() && v >= 0.0;
    (sane(metrics.width) && sane(metrics.height) && sane(baseline)).then_some(metrics)
}
