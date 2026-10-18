//! The binding core (LLP 1121 D12, stage 5): a design token by its name,
//! resolved in this session for a place. Code outside a plan (a native
//! module's view, a hatch's object, a host screen) holds a token's name and
//! kind, never a row's index, so a plan that reloads, updates or takes a
//! live batch keeps answering. A name the plan lacks is said once and the
//! caller keeps its fallback.

use super::{DataSource, Runner};
use exact_kernel::{ColorValue, StyleId, StyleProps, ViewId};

/// Where a binding resolves: a node, whose app tuple is the window's under
/// the nearest `-exact-mode` pin at or above it, or the session's root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    /// A host screen outside any node: the window's tuple.
    Root,
    /// A node's view (a native module's, a hatch's).
    Node(ViewId),
}

/// A generated constant (`contract tamber gen rust`, LLP 1121 D12): a
/// token's name and kind, which a session resolves by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<K> {
    /// The token's name as the design writes it.
    pub name: &'static str,
    kind: std::marker::PhantomData<K>,
}

impl<K> Token<K> {
    /// The token named `name`.
    pub const fn new(name: &'static str) -> Token<K> {
        Token {
            name,
            kind: std::marker::PhantomData,
        }
    }
}

/// A colour token's kind: [`Tamber::get`] answers its value as a row reads
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color;

/// A length token's kind (stage 6 G): its points at the place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Length;

/// A number token's kind (stage 6 G).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Number;

/// A type token's kind (stage 6 G): the [`Font`] it draws at the place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Type;

/// A type token's font at a place, as a host draws it (LLP 1121 D6).
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    /// Points: a text style's at the person's text size, a `rem` by the
    /// root font size.
    pub size: f64,
    /// The weight: the token's, else its text style's on this platform
    /// (LLP 1115.000 D12.2), stepped by Bold Text on a system family.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// The line height in points, where the token writes one (`normal`:
    /// none).
    pub line_height: Option<f64>,
    /// The family's plan stack, where the token writes one.
    pub family: Option<u16>,
}

/// A token kind, and what a token of it resolves to.
pub trait Kind {
    /// The value.
    type Value;
    /// `name`'s value at `place`, as [`Tamber::color`] reads a colour.
    fn get<D: DataSource>(
        tamber: &mut Tamber<'_, D>,
        name: &str,
        place: Place,
    ) -> Option<Self::Value>;
}

impl Kind for Length {
    type Value = f64;
    fn get<D: DataSource>(tamber: &mut Tamber<'_, D>, name: &str, place: Place) -> Option<f64> {
        tamber.length(name, place)
    }
}

impl Kind for Number {
    type Value = f64;
    fn get<D: DataSource>(tamber: &mut Tamber<'_, D>, name: &str, place: Place) -> Option<f64> {
        tamber.number(name, place)
    }
}

impl Kind for Type {
    type Value = Font;
    fn get<D: DataSource>(tamber: &mut Tamber<'_, D>, name: &str, place: Place) -> Option<Font> {
        tamber.font(name, place)
    }
}

impl Kind for Color {
    type Value = ColorValue;
    fn get<D: DataSource>(
        tamber: &mut Tamber<'_, D>,
        name: &str,
        place: Place,
    ) -> Option<ColorValue> {
        tamber.color(name, place)
    }
}

/// The session's tokens for bindings ([`Runner::tamber`]).
pub struct Tamber<'a, D: DataSource> {
    runner: &'a mut Runner<D>,
}

impl<D: DataSource> Runner<D> {
    /// This session's design tokens, by name, for code outside the plan
    /// (LLP 1121 D12): `session.tamber()`.
    pub fn tamber(&mut self) -> Tamber<'_, D> {
        Tamber { runner: self }
    }
}

impl<D: DataSource> Tamber<'_, D> {
    /// The app tuple at `place`.
    pub fn tuple(&self, place: Place) -> u32 {
        let kernel = self.runner.kernel();
        let mut at = match place {
            Place::Node(id) => kernel.node(id),
            Place::Root => None,
        };
        while let Some(node) = at {
            // A pin's row holds the tuple it makes, plus one (bridge.rs).
            if node.style.mask.has(StyleId::Mode) && node.style.mode > 0 {
                return node.style.mode - 1;
            }
            at = node.parent.and_then(|p| kernel.node(p));
        }
        self.runner.ids.theme.window
    }

    /// Generated constant `token` at `place` (`SURFACE_CARD`): its value,
    /// or `None` when the plan lacks it, said once ([`Tamber::color`]).
    pub fn get<K: Kind>(&mut self, token: Token<K>, place: Place) -> Option<K::Value> {
        K::get(self, token.name, place)
    }

    /// Whether the plan has token `name`.
    pub fn has(&self, name: &str) -> bool {
        self.row_name(name).is_some()
    }

    /// What a row names token `name` by: a role override's role spelling
    /// (`-exact-link`, `CanvasText`), which reads the session's override
    /// (a tint role stays the role, as a row keeps it), whether `name` is
    /// that spelling or the name the design wrote (`link`); else `name`.
    fn row_name(&self, name: &str) -> Option<String> {
        let plan = &self.runner.plan;
        // A colour binding names a colour token (a length's binding is
        // stage 6's commit G).
        let row = plan
            .theme
            .iter()
            .filter(|t| t.kind == 0)
            .find(|t| plan.str(t.name) == name)
            .or_else(|| {
                plan.theme
                    .iter()
                    .find(|t| !plan.str(t.role).is_empty() && plan.str(t.role) == name)
            })?;
        let role = plan.str(row.role);
        Some(
            if role.is_empty() {
                plan.str(row.name)
            } else {
                role
            }
            .to_string(),
        )
    }

    /// Colour token `name` at `place`: what a row naming it there reads (a
    /// colour, a `light-dark()` pair, a role the host resolves), with the
    /// live table's edits and the session's role overrides. `None` for a
    /// name the plan lacks, or a value its row would refuse (a live edit
    /// that does not parse), each said once in the journal; the caller
    /// keeps its fallback.
    pub fn color(&mut self, name: &str, place: Place) -> Option<ColorValue> {
        let Some(row) = self.row_name(name) else {
            self.say_once(name, "no token `{name}` in this plan");
            return None;
        };
        let tuple = self.tuple(place);
        let runner = &*self.runner;
        let _roles = runner.bind_roles();
        let before = crate::bridge::set_tuple(tuple);
        let mut patch = StyleProps::default();
        // Read aside: what the runner's own parses recorded stays theirs.
        let (set, _) = exact_kernel::style::theme::recorded(|| {
            crate::bridge::set_plan_style(
                &mut patch,
                StyleId::TextColor as u16,
                &exact_plan::Value::str(&row),
                &runner.plan,
            )
        });
        crate::bridge::set_tuple(before);
        if set.is_err() {
            self.say_once(name, "`{name}`'s value is no colour a row takes");
            return None;
        }
        Some(patch.text_color)
    }

    /// Token `name`'s value of `kind` at `place`, as the session's bound
    /// table holds it (its tuple's, with the live edits; a family as its
    /// stack); `None`, said once, when the plan has no such token.
    fn entry(
        &mut self,
        name: &str,
        kind: exact_kernel::style::theme::Kind,
        place: Place,
    ) -> Option<String> {
        let tuple = self.tuple(place);
        let table = crate::bridge::theme_table_at(&self.runner.plan, tuple);
        let value = table
            .iter()
            .find(|e| e.kind == kind && *e.name == *name)
            .map(|e| e.value.to_string());
        if value.is_none() {
            self.say_once(name, "no such token `{name}` in this plan");
        }
        value
    }

    /// The root font size and the font size at `place`, in points.
    fn font_sizes(&self, place: Place) -> (f32, f32) {
        let root = self.runner.root_font_size() as f32;
        let at = match place {
            Place::Node(id) => self
                .runner
                .kernel()
                .node(id)
                .map(|n| n.text_style().font_size),
            Place::Root => None,
        };
        (root, at.unwrap_or(root))
    }

    /// Length `text` (px, `rem`, `em`, a text style, the hairline) in points
    /// at `place`.
    fn points(&self, text: &str, place: Place) -> Option<f64> {
        let (root, reference) = self.font_sizes(place);
        match exact_kernel::style::relative::parse(text) {
            Some((unit, n)) => {
                let device = self.runner.kernel().device_pixel();
                Some(f64::from(unit.basis(root, reference, device) * n))
            }
            None => exact_num::parse_f64(text.trim().trim_end_matches("px")).ok(),
        }
    }

    /// Length token `name` at `place`, in points (stage 6 G).
    pub fn length(&mut self, name: &str, place: Place) -> Option<f64> {
        let text = self.entry(name, exact_kernel::style::theme::Kind::Length, place)?;
        self.points(&text, place)
    }

    /// Number token `name` at `place` (stage 6 G).
    pub fn number(&mut self, name: &str, place: Place) -> Option<f64> {
        let text = self.entry(name, exact_kernel::style::theme::Kind::Number, place)?;
        exact_num::parse_f64(text.trim()).ok()
    }

    /// Type token `name` at `place`: the font a host draws (stage 6 G).
    pub fn font(&mut self, name: &str, place: Place) -> Option<Font> {
        use exact_kernel::style::relative::{text_style, text_style_weight};
        use exact_kernel::style::theme::typeface;
        let text = self.entry(name, exact_kernel::style::theme::Kind::Type, place)?;
        let parts = typeface::parts(&text)?;
        let size = self.points(parts.size, place)?;
        let family = parts.family.and_then(|f| f.strip_prefix('#')?.parse().ok());
        let weight = match parts.weight.and_then(|w| w.parse().ok()) {
            Some(w) => w,
            None => text_style(parts.size).map_or(400, |id| {
                text_style_weight(id, exact_kernel::style::metrics::column())
            }),
        };
        let weight = match self.runner.kernel().bold_text_weights(family.unwrap_or(0)) {
            true => exact_kernel::bolder(weight),
            false => weight,
        };
        // A ratio, or a length whose `em` is the token's own size, as a
        // `line-height` reads it beside the `font-size` it writes.
        let line_height = match parts.leading {
            Some("normal") | None => None,
            Some(l) => match exact_num::parse_f64(l) {
                Ok(n) => Some(n * size),
                Err(_) => match exact_kernel::style::relative::parse(l) {
                    Some((exact_kernel::style::relative::Unit::Em, n)) => Some(f64::from(n) * size),
                    _ => self.points(l, place),
                },
            },
        };
        Some(Font {
            size,
            weight,
            italic: parts.style == Some("italic"),
            line_height,
            family,
        })
    }

    /// What a host's binding asks besides colours (stage 6 G), as the JSON
    /// members its answer appends: `,"lengths":{name: points},"numbers":
    /// {name: n},"fonts":{name: {size, weight, italic, lineHeight,
    /// family}}`, each for the names `request` lists under that key
    /// (`null` for a name the plan lacks).
    pub fn measures_json(&mut self, request: &serde_json::Value, place: Place) -> String {
        let list = |key: &str| -> Vec<String> {
            request[key]
                .as_array()
                .map(|n| {
                    n.iter()
                        .filter_map(|n| n.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut out = serde_json::Map::new();
        for key in ["lengths", "numbers", "fonts"] {
            let mut answers = serde_json::Map::new();
            for name in list(key) {
                let value = match key {
                    "lengths" => serde_json::json!(self.length(&name, place)),
                    "numbers" => serde_json::json!(self.number(&name, place)),
                    _ => self
                        .font(&name, place)
                        .map_or(serde_json::Value::Null, |f| {
                            serde_json::json!({
                                "size": f.size,
                                "weight": f.weight,
                                "italic": f.italic,
                                "lineHeight": f.line_height,
                                "family": f.family,
                            })
                        }),
                };
                answers.insert(name, value);
            }
            out.insert(key.into(), serde_json::Value::Object(answers));
        }
        let text = serde_json::Value::Object(out).to_string();
        format!(",{}", &text[1..text.len() - 1])
    }

    /// Whether `place` draws dark: the nearest `color-scheme` at or above
    /// it, else the window's.
    pub fn dark(&self, place: Place) -> bool {
        let scheme = match place {
            Place::Node(id) => self
                .runner
                .kernel()
                .node(id)
                .and_then(|n| n.color_scheme_dark()),
            Place::Root => None,
        };
        scheme.unwrap_or(self.runner.viewport().preferences.dark)
    }

    /// What a binding at `place` reads besides the token's name, as one
    /// number that moves when any of it does: its app tuple (`setMode`, a
    /// pin above), the design's values (a token edit, a live batch, a role
    /// override's), the platform's colours a role reads (a system accent),
    /// its scheme (the window's, a `color-scheme` above), the contrast, the
    /// root font size (the person's text size) and Bold Text. A host compares
    /// it after each batch
    /// to tell its observers (`observe`); it costs no walk of the design.
    pub fn inputs(&self, place: Place) -> u64 {
        use std::hash::{Hash, Hasher};
        let runner = &*self.runner;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.tuple(place).hash(&mut h);
        runner.design_moves.hash(&mut h);
        // The session's role overrides and the platform's reported colours
        // (a system accent moved), as a host compares them to re-present.
        {
            let _roles = runner.bind_roles();
            exact_kernel::style::roles::generation().hash(&mut h);
        }
        self.dark(place).hash(&mut h);
        format!("{:?}", runner.viewport().preferences.contrast).hash(&mut h);
        runner.root_font_size().to_bits().hash(&mut h);
        // Bold Text, which a font's weight reads, the place's own font size
        // (an `em`) and a device pixel (the hairline), stage 6 G.
        runner.viewport().preferences.bold_text.hash(&mut h);
        self.font_sizes(place).1.to_bits().hash(&mut h);
        runner.kernel().device_pixel().to_bits().hash(&mut h);
        h.finish()
    }

    /// Say `why` (its `{name}` the token's) once for this runner.
    fn say_once(&mut self, name: &str, why: &str) {
        let line = format!(
            "tamber: {}: its binding keeps its fallback (LLP 1121 D12)",
            why.replace("{name}", name)
        );
        if !self.runner.tamber_said.contains(&line) {
            self.runner.tamber_said.push(line.clone());
            self.runner.log(line);
        }
    }
}
