//! A node's dynamic style rows as the compiler writes them (rt.js `S`,
//! `Sm`), where a node's static rows must be inline too, and its bound
//! `data-*` words.

use super::Em;
use crate::style;
use exact_kernel::PropId;
use exact_kernel::{NodeType, StyleId};
use exact_plan::{BindingKind, BindingsRow, NodesId};
use exact_web::host::template::Parts;
use std::fmt::Write as _;

#[path = "text_style.rs"]
mod text_style;

impl Em<'_> {
    /// Clock ticks format cached instants without evaluating app code.
    pub(super) fn live_time(
        &mut self,
        i: u32,
        element: &str,
        e: &str,
        scope: &crate::code::Scope,
    ) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.node(NodesId(i));
        if element == "time"
            || (element == "progress"
                && row
                    .bindings
                    .iter()
                    .any(|b| plan.binding(b).id == PropId::Start as u16))
        {
            let mut values = Vec::new();
            for prop in [
                PropId::Datetime,
                PropId::TimeFormat,
                PropId::Start,
                PropId::End,
            ] {
                let value = row
                    .bindings
                    .iter()
                    .map(|b| plan.binding(b))
                    .find(|b| b.kind == BindingKind::Prop && b.id == prop as u16);
                values.push(match value {
                    Some(b) => format!("({})()", self.f(b.expr, scope)?),
                    None => "null".into(),
                });
            }
            let live = self.uses.rt("liveTime");
            let inline = row
                .parent
                .is_some_and(|p| plan.node(p).node_type == NodeType::Text as u8);
            let reserve = !inline
                && !row.bindings.iter().map(|b| plan.binding(b)).any(|b| {
                    b.kind == BindingKind::Style
                        && (b.id == StyleId::Width as u16 || b.id == StyleId::MaxWidth as u16)
                });
            let _ = write!(
                self.out,
                "{live}({e},()=>[{}],{reserve});",
                values.join(",")
            );
        }
        Ok(())
    }

    /// Whether image `i` needs symbols.js — it draws a symbol, its source is
    /// bound, or its source is an `app:/` file, which symbols.js resolves
    /// (`data-app-src`, LLP 1069.002 D7) — and whether its source is bound.
    pub(super) fn image_piece(&self, i: u32, parts: &Parts) -> Option<bool> {
        let plan = self.plan;
        let bound = plan
            .node(NodesId(i))
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .any(|b| {
                b.kind == BindingKind::Prop
                    && b.id == PropId::ImageSource as u16
                    && style::literal(plan, plan.code(b.expr)).is_none()
            });
        let app = parts
            .props
            .get("src")
            .is_some_and(|s| s.starts_with("app:/"));
        (bound || app || parts.props.contains_key("data-symbol-path")).then_some(bound)
    }

    /// @ref LLP 1055.002 — a synced animation: the web host's clocks
    /// (navigation.js `animationClocks`, through rt.js), made once, set a
    /// joined animation's start after each commit; under the agent its
    /// register does (agent.js).
    pub(super) fn clocks(&mut self) {
        let (clocks, after, clock) = (
            self.uses.rt("animationClocks"),
            self.uses.rt("After"),
            self.uses.rt("clock"),
        );
        let _ = write!(
            self.out,
            "if(!globalThis.__exactClocks&&typeof requestAnimationFrame==\"function\"&&!globalThis.__exactRender){{const c=globalThis.__exactClocks={clocks}(document);{after}.push(()=>{clock}.agent||c.sync());}}"
        );
    }

    /// [`Self::clocks`] when a static declaration puts the node on a clock.
    pub(super) fn clocks_in(&mut self, declarations: &str) {
        if declarations.contains("--exact-animation-clock:") {
            self.clocks();
        }
    }

    pub(super) fn f(
        &mut self,
        code: exact_plan::Code,
        scope: &crate::code::Scope,
    ) -> Result<String, String> {
        crate::code::function(self.plan, self.plan.code(code), scope, 0, &mut self.uses)
    }

    /// A bound paint fact for the CSS sibling-order rule. The expression is
    /// pure; the same effect scope as its style owns this attribute.
    pub(super) fn paint_binding(&mut self, kind: NodeType, b: &BindingsRow, e: &str, f: &str) {
        if !self.paint || kind.is_svg_element() || kind.is_metadata() {
            return;
        }
        if let Some((name, value)) = crate::paint::binding(self.plan, kind, b) {
            let f = if b.kind == BindingKind::Style && b.id == StyleId::BackdropFilter as u16 {
                format!("()=>{}(({f})(),false)", self.uses.rt("backdropValue"))
            } else {
                f.to_string()
            };
            let p = self.uses.rt("P");
            let _ = write!(
                self.out,
                "{p}({e},\"{name}\",()=>{{const v=({f})();return {value}}});"
            );
        }
    }

    /// A development build's binding `k` of node `i`, counted (LLP 1079 D1,
    /// perf.js): declared once, as `p{i}_{k}`, which every write it feeds
    /// reads, so a mapped write does not make a counter per evaluation.
    pub(super) fn counted(&mut self, i: u32, k: usize, f: String) -> String {
        if !self.site_attrs {
            return f;
        }
        let pf = self.uses.rt("pf");
        let _ = write!(self.out, "const p{i}_{k}={pf}({i},{f});");
        format!("p{i}_{k}")
    }

    /// One dynamic style binding `b` of node `i` (element `e`, value `f`).
    pub(super) fn style_row(
        &mut self,
        i: u32,
        b: &BindingsRow,
        parts: &Parts,
        e: &str,
        f: &str,
        scope: &crate::code::Scope,
    ) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.node(NodesId(i));
        let press = parts.css.contains("--exact-press:");
        let binding = |id: StyleId| {
            row.bindings
                .iter()
                .map(|b| plan.binding(b))
                .find(|b| b.kind == BindingKind::Style && b.id == id as u16)
        };
        // A clock (LLP 1055.002) plays on the page's timeline: only a drag
        // timeline's consumer is paused for the drag to seek.
        let clock = |s: &str| s.trim_start().starts_with("-exact-clock(");
        let timeline = binding(StyleId::AnimationTimeline)
            .is_some_and(|t| style::can_be(plan, plan.code(t.expr), &|s| !clock(s)));
        let id = StyleId::from_bit(b.id as u32).ok_or("unknown style row")?;
        let kind = NodeType::from_wire(row.node_type).ok_or("unknown node type")?;
        // @ref LLP 1115.002 D6 — a metric the binding may name is its value
        // in the web's column before the row's own map sees it.
        let metric = style::metric_map(plan, plan.code(b.expr))
            .map(|m| format!("()=>{}(({f})())", self.uses.share('m', m)));
        let f = metric.as_deref().unwrap_or(f);
        // @ref LLP 1121 D7 — a length or number token a bound value names is
        // its CSS, or its property's `var()` where the page declares one.
        let attribute = exact_web::host::template::attribute_rows(kind)
            .iter()
            .any(|(r, _)| *r == id);
        // A node with children reads no varying type or family token: they
        // would inherit it (the runner refuses it as an invalid value).
        // (Asked only where one varies: a map's name stays as it was.)
        let varying = plan.theme.iter().any(|r| {
            r.slices.len > 0
                && matches!(
                    crate::measures::kind(r),
                    Some(
                        exact_kernel::style::theme::Kind::Type
                            | exact_kernel::style::theme::Kind::Family
                    )
                )
        });
        let container = varying
            && (plan
                .nodes
                .iter()
                .any(|n| n.parent.is_some_and(|p| p.0 == i))
                || plan
                    .regions
                    .iter()
                    .any(|r| r.parent.is_some_and(|p| p.0 == i)));
        let measure = crate::measures::row(id)
            .then(|| {
                let props = crate::modes::properties(plan);
                crate::measures::map(plan, &props, id, attribute, container)
            })
            .flatten()
            .map(|m| {
                let bases = !crate::base_scale::named(plan).is_empty();
                let name = crate::measures::map_name(id, attribute, container, bases);
                let map = self.uses.constant(&name, || m);
                format!("()=>{map}(({f})())")
            });
        let f = measure.as_deref().unwrap_or(f);
        // A list on a corner is the elliptical radius no row holds, which
        // the browser would draw: refused whole, as the kernel refuses it.
        let corner = (exact_kernel::style::sides::FAMILIES[8].contains(&id)
            && style::literal(plan, plan.code(b.expr)).is_none())
        .then(|| {
            self.uses.rt("say");
            format!(
                "()=>{}(({f})())",
                self.uses.share('m', style::corner_list(id))
            )
        });
        let f = corner.as_deref().unwrap_or(f);
        if let Some((_, name)) = exact_web::host::template::attribute_rows(kind)
            .iter()
            .find(|(attribute, _)| *attribute == id)
        {
            let p = self.uses.rt("svgGeometry");
            let css = self
                .uses
                .rt(if self.is_motion_node(i) { "Sm" } else { "S" });
            let _ = write!(self.out, "{p}({e},\"{name}\",{f},{css});");
            return Ok(());
        }
        if matches!(
            id,
            StyleId::GridTemplateColumns
                | StyleId::GridTemplateRows
                | StyleId::GridColumn
                | StyleId::GridRow
                | StyleId::GridAutoFlow
                | StyleId::JustifyItems
        ) {
            self.uses.rt("gridValue");
        }
        if id == StyleId::AnimationTimeline && style::can_be(plan, plan.code(b.expr), &clock) {
            self.clocks();
        }
        // A bound pin: modes.js gives the boundary its tuple (LLP 1121 D7).
        if id == StyleId::Mode {
            let p = self.uses.rt("P");
            let _ = write!(self.out, "{p}({e},\"data-x-pin\",{f});");
            return Ok(());
        }
        // @ref LLP 1084 D7 — what the page's sheet reads to give a grouped
        // row's separator way (index.html): a bound edge or background image
        // its author wrote, while it holds a value, and a row's bound
        // `display`, while it hides the row. A literal one is said by the
        // element's own marks (`data-exact-said`, `data-exact-hidden`).
        let mark = match id {
            StyleId::BorderWidthTop if b.written => Some("data-exact-top-width"),
            StyleId::BorderStyleTop if b.written => Some("data-exact-top-style"),
            StyleId::BorderWidthBottom if b.written => Some("data-exact-bottom-width"),
            StyleId::BorderStyleBottom if b.written => Some("data-exact-bottom-style"),
            StyleId::BackgroundImage if b.written => Some("data-exact-background-image"),
            StyleId::Display
                if parts.props.contains_key("data-groupedrowseparator")
                    || parts.props.contains_key("data-native") =>
            {
                Some("data-exact-hidden")
            }
            _ => None,
        };
        if let Some(name) = mark {
            let p = self.uses.rt("P");
            let when = if id == StyleId::Display {
                "v===\"none\"?\"\":null"
            } else {
                "v==null||v===\"\"?null:\"\""
            };
            let _ = write!(
                self.out,
                "{p}({e},\"{name}\",()=>{{const v=({f})();return {when}}});"
            );
        }
        let refuse = |why: &str| Err(format!("node {i}: {why} is not in the JS target"));
        // @ref LLP 1077 D13 — a bound colour may name a system colour, which
        // the literal path resolved in the kernel. And a CSS-text row that
        // may name one inside it. The plan's colour map: one module constant
        // every such site shares.
        use exact_kernel::StyleCodec as C;
        let cm = matches!(
            id.codec(),
            C::ColorValue
                | C::KeywordColor
                | C::Paint
                | C::BackgroundImage
                | C::MaskImage
                | C::BoxShadow
                | C::TextShadow
                | C::Filter
        )
        .then(|| self.uses.constant("$cm", || style::color_map(plan)));
        // (name, unit, map): a map is JavaScript of the value (`null` writes none).
        let one = |name: &str, map: Option<String>| vec![(name.to_string(), String::new(), map)];
        let mut writes: Vec<(String, String, Option<String>)> = match id {
            // Safari selects by WebKit's prefixed property alone; `auto` and a
            // clear write nothing, as css.rs writes them (css/selection.rs,
            // css/clears.rs).
            StyleId::UserSelect => {
                let unsaid = || Some("v=>/^\\s*(auto|initial|unset)\\s*$/i.test(v)?null:v".to_string());
                vec![
                    ("-webkit-user-select".into(), String::new(), unsaid()),
                    ("user-select".into(), String::new(), unsaid()),
                ]
            }
            // @ref LLP 1077 D8 — the `rotate` and `translate` attributes bind
            // these with the same value: the angle's and xy's declaration
            // writes the author's whole text.
            StyleId::RotateAxis | StyleId::TranslateZ | StyleId::TranslatePercent => {
                let pair = if id == StyleId::RotateAxis { StyleId::Rotate } else { StyleId::Translate };
                if binding(pair).is_some_and(|o| plan.code(o.expr) == plan.code(b.expr)) {
                    return Ok(());
                }
                return refuse("a dynamic 3D part without its `rotate` or `translate`");
            }
            // @ref LLP 1077 D7 — the shorthand binds both rows with the
            // author's whole text: one declaration of the shorthand.
            StyleId::TextStrokeWidth | StyleId::TextStrokeColor
                if binding(if id == StyleId::TextStrokeWidth { StyleId::TextStrokeColor } else { StyleId::TextStrokeWidth })
                    .is_some_and(|o| plan.code(o.expr) == plan.code(b.expr)) =>
            {
                if id == StyleId::TextStrokeColor {
                    return Ok(());
                }
                one("-webkit-text-stroke", Some(self.uses.constant("$cm", || style::color_map(plan))))
            }
            // @ref LLP 1077 D8 — 0 is `none`, as css.rs writes it.
            StyleId::Perspective => one(
                "perspective",
                Some("v=>v==null?v:/^\\s*[+-]?(0+\\.?0*|\\.0+)(px)?\\s*$/i.test(v)?\"none\":typeof v===\"number\"?`${v}px`:v".into()),
            ),
            StyleId::BackdropFilter => {
                let check = self.uses.rt("backdropValue");
                one("backdrop-filter", Some(format!("v=>{check}(v)")))
            }
            // @ref LLP 1093 §1 — the row's 0 is CSS `auto`, as css.rs writes
            // it. `setProperty("column-count", "0")` does not stick, so the
            // unit sample would journal the refusal and leave a class rule.
            StyleId::ColumnCount => one(
                "column-count",
                Some("v=>v==null?v:typeof v===\"number\"?(v===0?\"auto\":v):v".into()),
            ),
            // A stack index: css.rs's declaration for each, by index.
            StyleId::FontFamily => {
                let table = serde_json::to_string(&style::font_family_table(plan)).unwrap();
                let runtime = self.uses.rt("fontFamily");
                one("font-family", Some(format!("v=>typeof v===\"string\"?{runtime}(v):{table}[v]??null")))
            }
            // css.rs's legacy clamp, on a non-scrolling block only.
            StyleId::Display if parts.props.contains_key("data-button-style") =>
                one("display", Some("v=>v===\"none\"?\"none\":\"grid\"".into())),
            StyleId::LineClamp if parts.props.contains_key("data-button-style") => vec![
                ("--exact-button-clamp".into(), String::new(), Some("v=>v>0?v:null".into())),
                ("--exact-button-title-display".into(), String::new(), Some("v=>v>0?\"-webkit-box\":null".into())),
            ],
            StyleId::LineClamp => {
                let display = parts.css.split(';').find_map(|d| d.strip_prefix("display:"));
                if display.is_some_and(|d| d != "block")
                    || parts.css.contains("overflow-x:scroll")
                    || parts.css.contains("overflow-y:scroll")
                    || parts.css.contains("overflow-x:auto")
                    || parts.css.contains("overflow-y:auto")
                {
                    self.warnings.push(format!(
                        "node {i}: style row line_clamp skipped: legacy line-clamp requires a non-scrolling block"
                    ));
                    return Ok(());
                }
                let when = |v: &str| Some(format!("v=>v>0?{v}:null"));
                vec![
                    ("-webkit-line-clamp".into(), String::new(), when("v")),
                    ("display".into(), String::new(), when("\"-webkit-box\"")),
                    ("-webkit-box-orient".into(), String::new(), when("\"vertical\"")),
                    ("overflow".into(), String::new(), when("\"hidden\"")),
                    ("--exact-button-clamp".into(), String::new(), Some("v=>v==null?null:v>0?v:0".into())),
                    ("--exact-button-title-display".into(), String::new(), Some("v=>v==null?null:v>0?\"-webkit-box\":\"block\"".into())),
                ]
            }
            // @ref LLP 1077 D14 — host-owned, as `-exact-press-scale`: the custom
            // property input-glue.js plays at the press (css.rs).
            StyleId::PressHaptic => {
                let press = self.uses.rt("pressFeedback");
                let _ = write!(self.out, "{press}();");
                one("--exact-press-haptic", Some("v=>v==null||v===\"none\"?null:v".into()))
            }
            // @ref LLP 1061 D7 — the pressed paint: the custom properties
            // input-glue.js reads from the element's own style (css.rs).
            StyleId::PressOpacity
            | StyleId::PressOpacityTo
            | StyleId::PressBackgroundColor
            | StyleId::PressColor => {
                let press = self.uses.rt("pressFeedback");
                let _ = write!(self.out, "{press}();");
                let name = match id {
                    StyleId::PressOpacity => "--exact-press-opacity",
                    StyleId::PressOpacityTo => "--exact-press-opacity-to",
                    StyleId::PressBackgroundColor => "--exact-press-background-color",
                    _ => "--exact-press-color",
                };
                let none = "v==null||/^\\s*unset\\s*$/i.test(v)?null:";
                one(name, Some(match &cm {
                    Some(cm) => format!("v=>{none}{cm}(v)"),
                    None => format!("v=>{none}v"),
                }))
            }
            // @ref LLP 1116.001 D1.4 — what navigation.js `fitText` reads, inline.
            StyleId::Fit => {
                let fit = self.uses.rt("fitTexts");
                let _ = write!(self.out, "{fit}();");
                one("--exact-fit", Some("v=>v===\"shrink\"?v:null".into()))
            }
            // The feedback's factor, and `scale` as its product (css.rs), on
            // a node whose own `scale` does not also compose through it.
            StyleId::PressScale => {
                if [StyleId::Scale, StyleId::Transition, StyleId::Animation]
                    .into_iter()
                    .any(|r| binding(r).is_some())
                {
                    return refuse("a dynamic `-exact-press-scale` beside `scale`, `transition` or `animation`");
                }
                let press = self.uses.rt("pressFeedback");
                let _ = write!(self.out, "{press}();");
                vec![
                    ("--exact-press".into(), String::new(), Some("v=>v==null||v===1||/^\\s*unset\\s*$/i.test(v)?null:v".into())),
                    (
                        "scale".into(),
                        String::new(),
                        Some("v=>v==null||v===1||/^\\s*unset\\s*$/i.test(v)?null:\"calc(var(--exact-scale,1) * var(--exact-press-factor,1))\"".into()),
                    ),
                ]
            }
            StyleId::FontSize if parts.tag != "img" => {
                text_style::font_size(plan, row, parts, &mut |map| self.uses.share('m', map))
            }
            StyleId::FontSize | StyleId::FontWeight if parts.tag == "img" => {
                let size = id == StyleId::FontSize;
                let mut writes = vec![(if size { "font-size" } else { "font-weight" }.into(), if size { "px" } else { "" }.into(), None)];
                writes.push((if size { "--exact-symbol-size-authored" } else { "--exact-symbol-weight-authored" }.into(), String::new(), Some("v=>v==null?null:1".into())));
                writes
            }
            StyleId::FontVariantNumeric => one("font-variant-numeric", None),
            // SVG's transform grammar, restated as CSS's (kernel TransformList).
            StyleId::Transform => {
                let t = self.uses.rt("svgTransform");
                one("transform", Some(format!("v=>{t}(v)")))
            }
            StyleId::MarkerStart | StyleId::MarkerMid | StyleId::MarkerEnd => {
                let (name, _) = style::style_marker(id);
                one(&name, Some("v=>v==null||/^\\s*none\\s*$/i.test(v)?null:v".into()))
            }
            // On a pressed node an animation of `scale` plays its rule's
            // `-exact-press` copy, and `scale` transitions as `--exact-scale`
            // (css.rs `keyframes_name`, `css_text`).
            StyleId::Animation if press => {
                let names: Vec<String> = self.press_keyframes.clone();
                let names = serde_json::to_string(&names).unwrap();
                let mut w = vec![(
                    "animation".to_string(),
                    String::new(),
                    Some(format!("v=>v==null||/^\\s*none\\s*$/i.test(v)?null:v.split(/,(?![^(]*\\))/).map(p=>p.replace(/[\\w-]+/g,n=>{names}.includes(n)?n+\"-exact-press\":n)).join(\",\")")),
                )];
                if timeline {
                    w.push(("animation-play-state".into(), String::new(), Some("v=>v==null||/^\\s*none\\s*$/i.test(v)?null:\"paused\"".into())));
                }
                w
            }
            StyleId::Transition if press => one(
                "transition",
                Some(pressed_transition_map()),
            ),
            // GitHub #809: what the kernel writes for each value the code
            // can push (style.rs `layout_transitions`); presence follows it
            // (rt.js `lt`, below). Another value is unset.
            StyleId::LayoutTransition => {
                let values = style::layout_transitions(plan, plan.code(b.expr));
                let (say, view) = (self.uses.rt("say"), self.uses.rt("viewId"));
                one("--exact-layout-transition", Some(style::layout_transition_map(&values, &format!("{view}({e})"), &say)))
            }
            _ => style::style_writes(b.id, timeline)
                .map_err(|x| format!("node {i}: {x}"))?
                .into_iter()
                .map(|w| {
                    // Composed with the row's own map (`accent-color` has one).
                    let map = match (w.map, &cm) {
                        (Some(m), Some(cm)) => Some(format!("v=>({m})({cm}(v))")),
                        (Some(m), None) => Some(m.to_string()),
                        (None, cm) => cm.clone(),
                    };
                    // A computed image the native hosts refuse is dropped
                    // here too, and journaled as they journal it, so the web
                    // never paints what a Mac drops (studio diary R15).
                    let map = if matches!(id, StyleId::BackgroundImage | StyleId::MaskImage) {
                        let names = exact_kernel::gradient::REFUSED
                            .iter()
                            .map(|(p, _)| p.trim_end_matches('('))
                            .collect::<Vec<_>>()
                            .join("|");
                        let inner = map.unwrap_or_else(|| "v=>v".into());
                        Some(format!(
                            "v=>{{if(v!=null&&/(^|[^a-z0-9-])({names})\\(/i.test(v)){{const x=globalThis.exact;x?.journal?.push(`t=${{x.now?.()??0}} invalid {} value ${{JSON.stringify(String(v))}}; unset`);return \"unset\"}}return({inner})(v)}}",
                            id.name().replace('_', "-")
                        ))
                    } else {
                        map
                    };
                    (w.name, w.unit, map)
                })
                .collect(),
        };
        if id == StyleId::Display && parts.css.contains("--exact-progress-justify:") {
            let normal = self.uses.rt("normal");
            writes.push(("--exact-progress-justify".into(), String::new(), Some(format!(
                "v=>[\"flex\",\"grid\",\"none\"].includes({normal}(\"display\",String(v??\"\")))?\"auto\":\"start\""
            ))));
        }
        if id == StyleId::PositionType && parts.props.contains_key("data-exact-progress") {
            let normal = self.uses.rt("normal");
            writes.push((
                "justify-self".into(),
                String::new(),
                Some(format!(
                    "v=>{normal}(\"position\",String(v??\"\"))===\"absolute\"?\"auto\":{:?}",
                    style::PROGRESS_JUSTIFY
                )),
            ));
        }
        if matches!(id, StyleId::FontWeight | StyleId::LineHeight) {
            let normal = self.uses.rt("normal");
            // The marker depends on both rows: either can expose an inherited value.
            let start = if parts.props.contains_key("data-fontshorthand") {
                binding(StyleId::FontSize)
                    .filter(|b| style::literal(plan, plan.code(b.expr)).is_none())
                    .map(|b| {
                        let size = self.f(b.expr, scope)?;
                        let map = self
                            .uses
                            .share('m', text_style::style_map(plan, &style::TEXT_STYLE_ROWS[0]));
                        Ok::<_, String>(format!("({map})(({size})())"))
                    })
                    .transpose()?
            } else {
                None
            };
            text_style::longhand(plan, row, parts, id, &normal, start.as_deref(), &mut writes);
        }
        // @ref LLP 1084 D7 — a grouped row's separator stands below its
        // bottom padding (the page's sheet, as `element::host_css_of` says).
        if id == StyleId::PaddingBottom && parts.props.contains_key("data-groupedrowseparator") {
            if let Some((_, unit, map)) = writes.first().cloned() {
                // A length; a percentage, which a background cannot reach,
                // is none (`element::host_css_of`).
                let inner = map.unwrap_or_else(|| "v=>v".into());
                let px = format!(
                    "v=>{{const t=({inner})(v);return t!=null&&/%/.test(String(t))?\"0px\":t}}"
                );
                writes.push(("--exact-padding-bottom".into(), unit, Some(px)));
            }
        }
        // A reference (`url(#…)`) names an element by its authored id, which
        // the kernel scopes to the instance (LLP 1055.000 D3): resolved at
        // run time from the node (`Sr`).
        let refs = style::can_be(plan, plan.code(b.expr), &|v| v.contains("url("));
        let s = self.uses.rt(if refs {
            "Sr"
        } else if self.is_motion_node(i) {
            "Sm"
        } else {
            "S"
        });
        // @ref LLP 1121 D7 — an inherited colour row says whether it read
        // a token (modes.js `Sx`), which a boundary below writes again (a
        // stroke's shorthand by its colour). A boundary declares the tokens,
        // so its `--exact-accent` naming one that names the accent would name
        // itself: that token's colour stands by tuple (`data-x-accent`), as
        // css.rs leaves a self-reference out; a clear there writes nothing.
        // A bound `color` that decides a symbol's ink (style.rs
        // `symbol_marks`) keeps whether its binding has a value now, a clear
        // included, whatever its map writes: symbols.js `inked` reads it.
        let inked = (id == StyleId::TextColor
            && parts.props.get("data-exact-ink").is_some_and(|w| w != "b"))
        .then(|| format!("()=>{{const v=({f})();{e}.$ink=v!=null;return v}}"));
        let f = inked.as_deref().unwrap_or(f);
        // A blank text row clears its start; `none` leaves that start exposed.
        let clear = matches!(id, StyleId::FontWeight | StyleId::LineHeight).then(|| {
            format!("()=>{{const v=({f})();return typeof v===\"string\"&&!v.trim()?\"unset\":v}}")
        });
        let f = clear.as_deref().unwrap_or(f);
        for (name, unit, map) in writes {
            let modes = crate::modes::active(plan);
            let channel = crate::modes::CHANNELS
                .iter()
                .position(|(_, p)| {
                    *p == name || name == "-webkit-text-stroke" && *p == "-webkit-text-stroke-color"
                })
                .filter(|_| modes);
            let map = match map {
                Some(m) if modes && name == "--exact-accent" => Some(format!(
                    "v=>{{v=({m})(v);if(!{e}.hasAttribute(\"data-x-b\"))return v;{e}.removeAttribute(\"data-x-accent\");if(v==null||/^\\s*(unset|inherit)?\\s*$/i.test(v))return null;const n={}.find(p=>v.includes(`var(${{p}})`));if(n){{{e}.setAttribute(\"data-x-accent\",n);return null}}return v}}",
                    serde_json::to_string(&crate::modes::accent_properties(plan)).unwrap()
                )),
                map => map,
            };
            let value = match map {
                // An empty string declares nothing before the map reads it.
                Some(m) => format!("()=>({m})({}(({f})()))", self.uses.rt("blank")),
                None => f.to_string(),
            };
            match channel {
                Some(j) => {
                    let _ = write!(self.out, "Sx({s},{e},\"{name}\",\"{unit}\",{value},{j});");
                }
                None => {
                    let _ = write!(self.out, "{s}({e},\"{name}\",\"{unit}\",{value});");
                }
            }
        }
        // The values that declare it, which presence asks of the binding
        // before each commit's tree update (rt.js `lt`).
        if id == StyleId::LayoutTransition {
            let values = style::layout_transitions(plan, plan.code(b.expr));
            let declaring: Vec<&String> = values
                .held
                .iter()
                .filter(|(_, css)| css.is_some())
                .map(|(v, _)| v)
                .collect();
            let lt = self.uses.rt("lt");
            let _ = write!(
                self.out,
                "{lt}({e},{},{f});",
                serde_json::to_string(&declaring).unwrap()
            );
        }
        Ok(())
    }

    /// A Markdown text field is the web host's own editor (LLP 1045 D5:
    /// markup-editor.js over its wasm), which replaces it once loaded; a
    /// plain one with `decorations` keeps its textarea and is painted.
    pub(super) fn editor(&mut self, i: u32, element: &str, e: &str) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.node(NodesId(i));
        let markup = row
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .find(|b| b.kind == BindingKind::Prop && b.id == PropId::Markup as u16);
        if element == "textarea" {
            if row
                .bindings
                .iter()
                .map(|b| plan.binding(b))
                .any(|b| b.kind == BindingKind::Prop && b.id == PropId::Decorations as u16)
            {
                // Paint only (LLP 1114.007): decorations.js, no editor wasm.
                let decorate = self.uses.rt("decorate");
                let _ = write!(self.out, "{decorate}({e});");
            }
            match markup.map(|b| style::literal(plan, plan.code(b.expr))) {
                Some(Some(v)) if v.as_str() == Some("markdown") => {
                    self.editor = true;
                    let mde = self.uses.rt("mde");
                    let _ = write!(self.out, "{mde}({e});");
                }
                Some(None) => {
                    return Err(format!(
                        "node {i}: a dynamic `markup` on a text field is not in the JS target"
                    ))
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// A node's `wrap-flow` (LLP 1043.000), for the text flow piece
    /// (rt.js `wf`): an absolutely positioned `both` is an exclusion.
    pub(super) fn wrap_flow(
        &mut self,
        i: u32,
        e: &str,
        scope: &crate::code::Scope,
    ) -> Result<(), String> {
        let plan = self.plan;
        let Some(b) = plan
            .node(NodesId(i))
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .find(|b| b.kind == BindingKind::Style && b.id == StyleId::WrapFlow as u16)
        else {
            return Ok(());
        };
        let value = match style::literal(plan, plan.code(b.expr)) {
            Some(v) => format!("()=>{}", super::value_js(&v)),
            None => self.f(b.expr, scope)?,
        };
        self.flow = true;
        let wf = self.uses.rt("wf");
        let _ = write!(self.out, "{wf}({e},{value});");
        Ok(())
    }

    /// Whether node `i` is a `symbol` or inside one (through regions' arms).
    pub(super) fn in_symbol(&self, i: u32) -> bool {
        let plan = self.plan;
        let mut at = Some(i);
        while let Some(n) = at {
            let row = &plan.node(NodesId(n));
            if NodeType::from_wire(row.node_type) == Some(NodeType::SvgSymbol) {
                return true;
            }
            at = self.parent_of(n);
        }
        false
    }

    /// Node `i`'s parent node, through regions' arms.
    pub(super) fn parent_of(&self, i: u32) -> Option<u32> {
        let plan = self.plan;
        let row = &plan.node(NodesId(i));
        match (row.parent, row.arm) {
            (Some(p), _) => Some(p.0),
            (None, Some(a)) => {
                let mut region = plan.arms[a.0 as usize].region;
                loop {
                    let r = &plan.regions[region.0 as usize];
                    match (r.parent, r.arm) {
                        (Some(p), _) => break Some(p.0),
                        (None, Some(a)) => region = plan.arms[a.0 as usize].region,
                        (None, None) => break None,
                    }
                }
            }
            (None, None) => None,
        }
    }

    /// Preserve literal authored IDs for URL references and for drag bindings
    /// when the template rewrites an SVG element's DOM ID.
    pub(super) fn exact_id(&self, i: u32) -> Option<String> {
        let plan = self.plan;
        let node = &plan.node(NodesId(i));
        if !self.refs && !NodeType::from_wire(node.node_type).is_some_and(|t| t.is_svg_element()) {
            return None;
        }
        node.bindings.iter().map(|b| plan.binding(b)).find_map(|b| {
            (b.kind == BindingKind::Prop && b.id == PropId::Id as u16)
                .then(|| style::literal(plan, plan.code(b.expr)))
                .flatten()
                .and_then(|v| v.as_str().map(str::to_string))
        })
    }
}

/// The `@keyframes` that animate `scale`: a pressed node plays their
/// `-exact-press` copies.
pub(super) fn press_keyframes(plan: &exact_plan::Plan) -> Vec<String> {
    plan.keyframes
        .iter()
        .filter(|k| {
            exact_motion::Keyframes::parse(plan.str(k.css))
                .is_ok_and(|f| f.css().contains("scale:"))
        })
        .map(|k| plan.str(k.name).to_string())
        .collect()
}

/// Whether a dynamic style row can reference an element (`url(#…)`).
pub(super) fn can_refer(plan: &exact_plan::Plan) -> bool {
    plan.bindings.iter().any(|b| {
        b.kind == BindingKind::Style
            && style::literal(plan, plan.code(b.expr)).is_none()
            && style::can_be(plan, plan.code(b.expr), &|v| v.contains("url("))
    })
}

/// Take the presence rows' declarations (LLP 1063) out of a node's static
/// CSS, returned in the order they were written.
pub(super) fn presence_decls(css: &mut String) -> String {
    let mut kept = String::new();
    let mut taken = String::new();
    for decl in css.split_inclusive(';') {
        // The drag timelines' too (LLP 1057.003 D2, D4): motion-glue.js
        // reads each from the element's own declaration.
        if [
            "--exact-layout-transition:",
            "--exact-exit-animation:",
            "--exact-drag-timeline:",
            "--exact-animation-timeline:",
            // A synced animation's clock (LLP 1055.002), read the same way.
            "--exact-animation-clock:",
            "--exact-animation-range:",
            "--exact-timeline-scope:",
            // The press feedback's factor (LLP 1061) and haptic (LLP 1077
            // D14), which input-glue.js reads from the element's own style.
            "--exact-press:",
            "--exact-press-haptic:",
            // The pressed paint (LLP 1061 D7), read the same way.
            "--exact-press-opacity:",
            "--exact-press-opacity-to:",
            "--exact-press-background-color:",
            "--exact-press-color:",
            // A fitted text's (LLP 1116.001 D1.4), which navigation.js finds by it.
            "--exact-fit:",
        ]
        .iter()
        .any(|p| decl.starts_with(p))
        {
            taken.push_str(decl);
        } else {
            kept.push_str(decl);
        }
    }
    *css = kept;
    taken
}

impl Em<'_> {
    /// A `markup="markdown"` text's source, built into spans (rt.js `md`).
    pub(crate) fn markdown(&mut self, e: &str, f: &str) {
        let md = self.uses.rt("md");
        let _ = write!(self.out, "{md}({e},{f});");
    }

    /// What an element needs once made: a canvas's surface, a native
    /// module's mount (LLP 1024 D3), a hatched node's page-module hatch (LLP
    /// 1075.003.000, `data-hatch` among its static attributes), and the
    /// constant values settled once its tree is in place, as bound ones are
    /// (rt.js `drain`): a select's, which its options carry (calendar diary
    /// F6), and a scroller's offsets (F8).
    pub(crate) fn element_extras(
        &mut self,
        tag: &str,
        e: &str,
        attrs: &[(String, String)],
        props: &exact_kernel::SortedMap<String, String>,
    ) {
        for name in ["value", "scrollTop", "scrollLeft"] {
            if let Some(v) = props
                .get(name)
                .filter(|_| name != "value" || tag == "select")
            {
                let p = self.uses.rt("P");
                let v = serde_json::to_string(v).unwrap();
                let _ = write!(self.out, "{p}({e},\"{name}\",()=>{v});");
            }
        }
        if let Some(v) = sandboxed_source(tag, props) {
            let p = self.uses.rt("P");
            let v = serde_json::to_string(v).unwrap();
            let _ = write!(self.out, "{p}({e},\"src\",()=>{v});");
        }
        if tag == "canvas" {
            let cv = self.uses.rt("cv");
            let _ = write!(self.out, "{cv}({e});");
        }
        if tag.contains('-') {
            let _ = write!(self.out, "{}({e});", self.uses.rt("nm"));
        }
        if attrs.iter().any(|(k, _)| k == "data-hatch") {
            let _ = write!(self.out, "{}({e});", self.uses.rt("ht"));
        }
        // A context menu's popover (LLP 1021 §5.1), named by a literal.
        if attrs.iter().any(|(k, _)| k == "contextpopover") {
            let _ = write!(self.out, "{}({e});", self.uses.rt("cp"));
        }
    }

    /// A node's `data-*` words (LLP 1075.003 §3.3): one attribute per word,
    /// re-derived when the bound object changes (`dataset.js`).
    pub(crate) fn dataset(&mut self, e: &str, f: &str) {
        let (effect, ds) = (self.uses.rt("effect"), self.uses.rt("ds"));
        let _ = write!(self.out, "{effect}(()=>{ds}({e},({f})()));");
    }
}

/// The document's attribute rules (`host/web/src/document.rs`, `Walk::element`),
/// for a static prop: `(attributes, text content, extra CSS)`.
pub(super) fn attributes(
    element: &str,
    props: &exact_kernel::SortedMap<String, String>,
) -> (Vec<(String, String)>, Option<String>, String) {
    let mut attrs = Vec::new();
    let mut content = None;
    let mut css = String::new();
    for (name, value) in props {
        match name.as_str() {
            "scrollFollowEnd" | "scrollTop" | "scrollLeft" => {}
            // An authored `autofocus=false` is kept for the agent's tree
            // (the runner reports it); the attribute would mean true.
            "autofocus" => {
                if value == "true" {
                    attrs.push((name.clone(), String::new()));
                } else {
                    attrs.push(("data-autofocus".into(), "false".into()));
                }
            }
            // The app's own file (LLP 1069.002 D7): symbols.js shows it
            // through an object URL, media.js plays it (podcast F19); the
            // browser has no `app:` scheme.
            "src"
                if matches!(element, "img" | "video" | "audio" | "iframe")
                    && value.starts_with("app:/") =>
            {
                attrs.push(("data-app-src".into(), value.clone()));
            }
            "poster" if element == "video" && value.starts_with("app:/") => {
                attrs.push(("data-app-poster".into(), value.clone()));
            }
            "src" if element == "img" && value.starts_with("symbol:") => {
                attrs.push((
                    name.clone(),
                    "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16'/%3E".into(),
                ));
            }
            "text" => {
                if element != "canvas" && !value.is_empty() {
                    content = Some(value.clone());
                }
            }
            "data-action" => {
                attrs.push((name.clone(), value.clone()));
                css.push_str("touch-action:none;");
            }
            // A select's value is its options' (`element_extras`); a
            // `<progress>`'s is its attribute, without which it is
            // indeterminate (LLP 1116 D8).
            "value" => match element {
                "input" | "button" | "option" | "progress" => {
                    attrs.push((name.clone(), value.clone()))
                }
                "textarea" => content = Some(value.clone()),
                _ => {}
            },
            "checked" | "inert" | "disabled" | "readonly" | "multiple" => {
                if value == "true" {
                    attrs.push((name.clone(), String::new()));
                }
            }
            "autoplay"
            | "controls"
            | "loop"
            | "muted"
            | "playsinline"
            | "disablepictureinpicture"
            | "disableremoteplayback"
                if element == "video" || element == "audio" =>
            {
                if value == "true" {
                    attrs.push((name.clone(), String::new()));
                }
            }
            "href" if !exact_web::host::document::navigable(value) => {}
            "src" if sandboxed_source(element, props).is_some() => {}
            "src"
                if element == "iframe"
                    && !exact_web::host::document::frame_navigable(
                        value,
                        props.get("sandbox").map(String::as_str),
                    ) =>
            {
                attrs.push((name.clone(), "about:blank".into()));
            }
            _ => attrs.push((name.clone(), value.clone())),
        }
    }
    (attrs, content, css)
}

/// A frame's literal `data:` source (refused alone, shown in an opaque
/// sandbox) with no literal `sandbox`: a bound one decides it, so it is no
/// attribute (the frame shows about:blank) and rt.js `P` writes it
/// (`element_extras`), keeping it to decide again as the sandbox changes.
fn sandboxed_source<'a>(
    element: &str,
    props: &'a exact_kernel::SortedMap<String, String>,
) -> Option<&'a String> {
    use exact_web::host::document::{frame_navigable, navigable};
    props.get("src").filter(|v| {
        element == "iframe"
            && !props.contains_key("sandbox")
            && !navigable(v)
            && frame_navigable(v, Some(""))
    })
}

/// A pressed node's computed `transition`: an author's entries
/// ([`crate::style::TRANSITION_ENTRIES`]), with `scale` moved to the
/// `--exact-scale` the press composes with.
pub(crate) fn pressed_transition_map() -> String {
    format!(
                    "v=>{{if(v==null)return v;{}const o=p.map(t=>t.replace(/^scale(?=\\s)/,\"--exact-scale\")),m=p.filter(t=>/^(scale|all)(\\s|$)/.test(t)).pop();if(m)o.push(\"scale 0s\",m.replace(/^(scale|all)/,\"--exact-scale\"));return o.join(\",\")||\"none\"}}",
                    crate::style::TRANSITION_ENTRIES
                )
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_names_added_by_attribute_and_canvas_writers_are_reserved() {
        let plan = contract::compile(
            "component App\n  view\n    canvas bitmap-width=16 bitmap-height=16 color-space=\"display-p3\" color-type=\"float16\"\n",
        )
        .unwrap();
        let mut attrs = Vec::new();
        for node in plan.nodes.iter() {
            attrs.extend(crate::style::canvas_bitmap(&plan, &node).unwrap());
        }
        let mut props = exact_kernel::SortedMap::default();
        props.insert("src".into(), "app:/movie.mp4".into());
        props.insert("poster".into(), "app:/poster.png".into());
        props.insert("autofocus".into(), "false".into());
        for tag in ["img", "video", "audio", "iframe", "input"] {
            attrs.extend(super::attributes(tag, &props).0);
        }
        let names: std::collections::BTreeSet<_> = attrs
            .iter()
            .map(|(name, _)| name.as_str())
            .filter(|name| name.starts_with("data-"))
            .collect();
        for name in ["data-app-poster", "data-color-space", "data-color-type"] {
            assert!(names.contains(name), "writer did not emit {name}");
        }
        let mut unreserved = Vec::new();
        for name in names {
            let source = format!("component App\n  view\n    column {name}=\"x\"\n");
            match contract::compile(&source) {
                Err(error) if error.id == "lower-data-word" => {}
                _ => unreserved.push(name),
            }
        }
        assert!(unreserved.is_empty(), "not reserved: {unreserved:?}");
    }

    #[test]
    fn a_native_buttons_bound_display_preserves_the_semantic_grid() {
        let plan = contract::compile(
            r#"component App
  state visible = true
  view
    button appearance="auto" display=(visible ? "flex" : "none")
      text "Lock"
      image "symbol:lock"
"#,
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        assert!(js.contains("v=>v===\"none\"?\"none\":\"grid\""), "{js}");
    }

    #[test]
    fn a_literal_data_frame_is_shown_by_its_sandbox() {
        // A literal opaque sandbox decides here; a bound one, in rt.js `P`,
        // which is handed the source (no attribute until then); a
        // same-origin one refuses it.
        let attrs = |sandbox: Option<&str>| {
            let mut props = exact_kernel::SortedMap::default();
            props.insert("src".to_string(), "data:text/html,hi".to_string());
            if let Some(s) = sandbox {
                props.insert("sandbox".to_string(), s.to_string());
            }
            let (attrs, _, _) = super::attributes("iframe", &props);
            attrs.into_iter().find(|(k, _)| k == "src").map(|a| a.1)
        };
        assert_eq!(attrs(Some("allow-scripts")).unwrap(), "data:text/html,hi");
        assert_eq!(
            attrs(Some("allow-scripts allow-same-origin")).unwrap(),
            "about:blank"
        );
        assert_eq!(attrs(None), None);
        let plan = contract::compile(
            r#"component App
  state sandbox = "allow-scripts"
  view
    column
      iframe "data:text/html,a" sandbox=sandbox
      iframe "data:text/html,b" sandbox="allow-scripts allow-same-origin"
      iframe "https://e.dev/c"
"#,
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        assert!(js.contains(",\"src\",()=>\"data:text/html,a\")"), "{js}");
        assert!(!js.contains("()=>\"data:text/html,b\""), "{js}");
        assert!(!js.contains("()=>\"https://e.dev/c\""), "{js}");
    }

    #[test]
    fn a_pressed_nodes_computed_transition_is_checked_as_an_authors() {
        // LLP 1081 D5, as `style`'s map, then `scale` moved to the press's
        // `--exact-scale`.
        assert_eq!(
            crate::style::tests::run(
                &super::pressed_transition_map(),
                &[
                    "-exact-tint-color 1s",
                    "tint-color 1s, opacity 1s",
                    "/**/--exact-tint 1s",
                    "opacity 1s -exact-spring(1, 2, 3), scale 1s",
                    "press-scale 1s",
                    "animation-trigger 1s, --exact-π 1s",
                ]
            ),
            serde_json::json!([
                "--exact-tint 1s",
                "opacity 1s",
                "none",
                "--exact-scale 1s,scale 0s,--exact-scale 1s",
                "none",
                "none"
            ])
        );
    }

    use crate::style::{
        color_map,
        tests::{role, run},
    };

    /// A bound `filter` (LLP 1095 D1) is written through the colour map:
    /// the emitted binding wraps its value in `color_map(plan)`, a module
    /// constant every colour site shares (`$cm`), which turns
    /// a role into its CSS, an admitted `-exact-platform-color()` literal into its
    /// fallback, and refuses one the plan does not hold.
    #[test]
    fn a_bound_filter_goes_through_the_colour_map() {
        let literal =
            "drop-shadow(0px 2px 4px -exact-platform-color(ios webJsFilterColor, #010203))";
        let source = format!(
            r#"component App
  state on = false
  action flip
    on = !on
  view
    column
      button press=flip testId="flip"
        text "Flip"
      text "Shadow" filter=(on ? "{literal}" : "drop-shadow(0px 2px 4px -exact-system-orange)")
"#
        );
        let plan = contract::compile(&source).unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        let map = color_map(&plan);
        assert!(
            map.contains("webJsFilterColor"),
            "the plan admits the literal"
        );
        assert!(
            js.contains(&format!("const $cm={map};\n"))
                && js.contains(",\"filter\",\"\",()=>($cm)(blank(("),
            "the bound filter is not mapped:\n{js}"
        );
        // One constant, however many sites; none where no site maps a colour.
        assert_eq!(js.matches("const $cm=").count(), 1);
        let plain = contract::compile(
            "component App\n  state on = false\n  view\n    text \"Plain\" opacity=(on ? 0.5 : 1)\n",
        )
        .unwrap();
        let plain = crate::emit::emit(&plain, false, false).unwrap().js;
        assert!(
            plain.contains("\"opacity\"") && !plain.contains("$cm"),
            "{plain}"
        );
        assert_eq!(
            run(
                &map,
                &[
                    "drop-shadow(0px 2px 4px -exact-system-orange)",
                    literal,
                    "drop-shadow(0px 2px 4px -exact-platform-color(ios webJsOtherColor, #010203))",
                ]
            ),
            serde_json::json!([
                format!("drop-shadow(0px 2px 4px {})", role("system-orange")),
                "drop-shadow(0px 2px 4px #010203ff)",
                "unset",
            ])
        );
    }

    /// A grouped row whose flag is bound chooses between two metrics as it
    /// runs (LLP 1115.002 D6): the page, which keeps no kernel, reads each
    /// name as its value in the web's column, as the kernel reads a literal
    /// one. A number takes the row's `px`, a role goes on through the colour
    /// map, and a metric with no row on the web unsets the row.
    #[test]
    fn a_bound_metric_is_its_value_in_the_webs_column() {
        let plan = contract::compile(
            r#"component App
  state gone = false
  action flip
    gone = !gone
  view
    list appearance="auto" flex=1
      section
        button press=flip destructive=gone testId="row"
          if gone
            image "symbol:sf/trash"
          text "Remove"
"#,
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        assert!(
            !js.contains("\"-exact-grouped-") || js.contains("Object.hasOwn(M,v)?M[v]:v"),
            "a metric's name is bound and nothing reads it:\n{js}"
        );
        let named = |row: &str| {
            let binding = plan
                .bindings
                .iter()
                .find(|b| {
                    b.kind == exact_plan::BindingKind::Style
                        && exact_kernel::StyleId::from_bit(b.id as u32)
                            .is_some_and(|id| id.name() == row)
                        && crate::style::literal(&plan, plan.code(b.expr)).is_none()
                        && crate::style::metric_map(&plan, plan.code(b.expr)).is_some()
                })
                .unwrap_or_else(|| panic!("no bound {row} names a metric"));
            crate::style::metric_map(&plan, plan.code(binding.expr)).unwrap()
        };
        // The row's content starts at its text, after a symbol where one is
        // shown: its padding, the row spanning its card (LLP 1084 D7).
        assert_eq!(
            run(
                &named("padding_left"),
                &[
                    "-exact-grouped-row-inset",
                    "-exact-grouped-row-inset-symbol",
                    "-exact-grouped-button-padding-start",
                    "12px"
                ]
            ),
            serde_json::json!([16, 56, 16, "12px"])
        );
        // Red when destructive, by its role; the label's colour is no metric.
        assert_eq!(
            run(
                &named("text_color"),
                &["-exact-grouped-destructive-color", "-exact-label"]
            ),
            serde_json::json!(["-exact-system-red", "-exact-label"])
        );
        // The Mac's push button's margin is no row on the web.
        assert_eq!(
            run(
                &named("margin_left"),
                &["-exact-grouped-button-margin-inline", "unset"]
            ),
            serde_json::json!([null, "unset"])
        );
    }

    /// A bound `column-count` of 0 is CSS `auto` (LLP 1093 §1), as css.rs
    /// writes the row. The JS target must write that string: `setProperty`
    /// rejects `"0"`, the inline declaration drops, a class rule stays, and
    /// the journal records the refusal. Toggling 2 then 0 computes `auto`.
    #[test]
    fn a_bound_column_count_of_zero_computes_to_auto() {
        let plan = contract::compile(
            r#"component App
  state n = 2
  action auto
    n = 0
  view
    view column-count=n testId="flow"
      view
"#,
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        let marker = "\"column-count\",\"";
        let at = js
            .find(marker)
            .unwrap_or_else(|| panic!("no column-count write:\n{js}"));
        let rest = &js[at + marker.len()..];
        let (unit, after) = rest.split_once('"').expect("unit");
        let map = after.strip_prefix(",()=>(").and_then(|mapped| {
            mapped
                .find(")(blank((")
                .map(|end| mapped[..end].to_string())
        });
        let map_src = map.unwrap_or_else(|| "v=>v".into());
        let script = format!(
            "const map={map_src};const unit={unit:?};const write=v=>{{const m=map(v);return m==null?null:typeof m===\"number\"?m+unit:String(m)}};console.log(JSON.stringify([write(0),write(2),write(null)]))"
        );
        let bun = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
            .args(["-e", &script])
            .output()
            .expect("bun");
        assert!(
            bun.status.success(),
            "{}",
            String::from_utf8_lossy(&bun.stderr)
        );
        let written: serde_json::Value = serde_json::from_slice(&bun.stdout).unwrap();
        assert_eq!(
            written,
            serde_json::json!(["auto", "2", null]),
            "column-count write unit={unit:?} map={map_src}\n{js}"
        );
        let dir = std::env::temp_dir().join(format!("exact-column-count-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let page = dir.join("col.html");
        let cursor_writes =
            crate::style::style_writes(exact_kernel::StyleId::Cursor as u16, false).unwrap();
        let cursor_map = cursor_writes[0].map.unwrap();
        let html = format!(
            r#"<!doctype html><meta charset="utf-8"><style>#flow{{column-count:3;width:320px}}button{{cursor:default}}</style><div id="flow">column</div><div style="cursor:grab"><button id="cursor">Cursor</button></div><pre id="out"></pre><script>
const journal=[];globalThis.exact={{journal,now:()=>0}};
function css(e,prop,unit,v){{const t=v==null?null:typeof v==="number"?v+unit:String(v);if(t==null){{e.style.removeProperty(prop);return}}e.style.removeProperty(prop);e.style.setProperty(prop,t);if(!e.style.getPropertyValue(prop))journal.push(`unset ${{prop}}: ${{JSON.stringify(v)}} is not a value it takes`)}}
const map={map_src};const unit={unit:?};const flow=document.getElementById("flow");
const apply=v=>css(flow,"column-count",unit,map(v));
apply(2);const after2=getComputedStyle(flow).columnCount;const journal2=journal.slice();journal.length=0;
apply(0);const after0=getComputedStyle(flow).columnCount;
const journal0=journal.slice();journal.length=0;
const cursorMap={cursor_map},cursor=document.getElementById("cursor");
const cursors=["pointer","inherit","unset","not-a-cursor",null].map(v=>{{css(cursor,"cursor","",cursorMap(v));return getComputedStyle(cursor).cursor}});
document.getElementById("out").textContent=JSON.stringify({{after2,after0,journal2,journal0,inline:flow.style.getPropertyValue("column-count"),cursors,cursorJournal:journal}});
</script>"#
        );
        std::fs::write(&page, html).unwrap();
        // Use the same provisioned browser carrier as the app proofs. Chrome's
        // --dump-dom can stay alive indefinitely on Linux even after the script ran.
        let browser_script = format!(
            r#"
import {{ chromium }} from 'playwright-core';
const browser = await chromium.launch({{executablePath: process.env.CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage']}});
try {{
  const page = await browser.newPage();
  await page.goto({});
  process.stdout.write(await page.locator('#out').textContent());
}} finally {{ await browser.close(); }}
"#,
            serde_json::to_string(&format!("file://{}", page.display())).unwrap()
        );
        let result =
            std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
                .args(["--eval", &browser_script])
                .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .output()
                .expect("Bun runs the provisioned browser");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let computed: serde_json::Value =
            serde_json::from_slice(&result.stdout).expect("computed styles");
        assert_eq!(computed["after2"], "2", "{computed}");
        assert_eq!(computed["after0"], "auto", "{computed}");
        assert_eq!(computed["journal2"], serde_json::json!([]), "{computed}");
        assert_eq!(computed["journal0"], serde_json::json!([]), "{computed}");
        assert_eq!(computed["inline"], "auto", "{computed}");
        assert_eq!(
            computed["cursors"],
            serde_json::json!(["pointer", "grab", "grab", "grab", "default"]),
            "{computed}"
        );
        assert_eq!(
            computed["cursorJournal"].as_array().unwrap().len(),
            1,
            "{computed}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A computed corner given a list (`10px 20px`, `10px / 20px`) is
    /// refused whole with a journal line, as the kernel refuses it; a calc
    /// is one value.
    #[test]
    fn a_computed_corner_list_is_refused_as_the_kernel_refuses_it() {
        let plan = contract::compile(
            "component App\n  state r = \"10px 20px\"\n  view\n    view border-top-left-radius=r testId=\"c\"\n",
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        assert!(
            js.contains("unset border-top-left-radius: ") && js.contains(",say,"),
            "{js}"
        );
        let map = crate::style::corner_list(exact_kernel::StyleId::BorderRadiusTopLeft);
        let script = format!(
            "const journal=[];const say=l=>journal.push(l);const m={map};console.log(JSON.stringify({{out:[\"10px 20px\",\"10px/20px\",\"calc(10px + 2px)\",\"12px\",8,null].map(m),journal:journal.length}}))"
        );
        let bun = std::process::Command::new(std::env::var("BUN").unwrap_or_else(|_| "bun".into()))
            .args(["-e", &script])
            .output()
            .expect("bun");
        assert!(
            bun.status.success(),
            "{}",
            String::from_utf8_lossy(&bun.stderr)
        );
        let got: serde_json::Value = serde_json::from_slice(&bun.stdout).unwrap();
        assert_eq!(
            got,
            serde_json::json!({"out": ["unset", "unset", "calc(10px + 2px)", "12px", 8, null], "journal": 2})
        );
    }

    /// An empty string declares nothing before a row's map reads it: the
    /// mapped rows (`cursor`, `animation-timeline` and its companions) see
    /// `null` (rt.js `blank`), never a value to diagnose.
    #[test]
    fn an_empty_value_reaches_a_mapped_row_as_nothing() {
        let plan = contract::compile(
            "component App\n  state c = \"\"\n  view\n    view cursor=c animation-timeline=c testId=\"v\"\n",
        )
        .unwrap();
        let js = crate::emit::emit(&plan, false, false).unwrap().js;
        for name in [
            "\"cursor\"",
            "\"--exact-animation-timeline\"",
            "\"animation-play-state\"",
        ] {
            let at = js.find(name).unwrap_or_else(|| panic!("{name}:\n{js}"));
            let write = js[at..].split(";S").next().unwrap();
            assert!(write.contains("blank(("), "{name}: {write}");
        }
    }
}
