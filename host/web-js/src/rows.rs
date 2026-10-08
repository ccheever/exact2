//! A node's dynamic style rows as the compiler writes them (rt.js `S`,
//! `Sm`), where a node's static rows must be inline too, and its bound
//! `data-*` words.

use super::Em;
use crate::style;
use exact_kernel::PropId;
use exact_kernel::{NodeType, StyleId};
use exact_plan::{BindingKind, BindingsRow};
use exact_web::host::template::Parts;
use std::fmt::Write as _;

impl Em<'_> {
    /// Whether image `i` needs symbols.js — it draws a symbol, its source is
    /// bound, or its source is an `app:/` file, which symbols.js resolves
    /// (`data-app-src`, LLP 1069.002 D7) — and whether its source is bound.
    pub(super) fn image_piece(&self, i: u32, parts: &Parts) -> Option<bool> {
        let plan = self.plan;
        let bound = plan.nodes[i as usize]
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
    ) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.nodes[i as usize];
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
        let refuse = |why: &str| Err(format!("node {i}: {why} is not in the JS target"));
        // (name, unit, map): a map is JavaScript of the value (`null` writes none).
        let one = |name: &str, map: Option<String>| vec![(name.to_string(), String::new(), map)];
        let writes: Vec<(String, String, Option<String>)> = match id {
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
                one("-webkit-text-stroke", Some(style::color_map(plan)))
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
                one("font-family", Some(format!("v=>{table}[v]??null")))
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
                    ("--exact-press".into(), String::new(), Some("v=>v==null||v===1?null:v".into())),
                    (
                        "scale".into(),
                        String::new(),
                        Some("v=>v==null||v===1?null:\"calc(var(--exact-scale,1) * var(--exact-press-factor,1))\"".into()),
                    ),
                ]
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
            _ => style::style_writes(b.id, timeline)
                .map_err(|x| format!("node {i}: {x}"))?
                .into_iter()
                .map(|w| {
                    // @ref LLP 1077 D13 — a bound colour may name a system
                    // colour, which the literal path resolved in the kernel.
                    // And a CSS-text row that may name one inside it.
                    use exact_kernel::StyleCodec as C;
                    let colors = matches!(
                        id.codec(),
                        C::ColorValue | C::KeywordColor | C::Paint | C::BackgroundImage | C::MaskImage | C::BoxShadow | C::TextShadow | C::Filter
                    );
                    // Composed with the row's own map (`accent-color` has one).
                    let map = match w.map {
                        Some(m) if colors => Some(format!("v=>({m})(({})(v))", style::color_map(plan))),
                        Some(m) => Some(m.to_string()),
                        None => colors.then(|| style::color_map(plan)),
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
                            "v=>{{if(v!=null&&/(^|[^a-z0-9-])({names})\\(/i.test(v)){{const x=globalThis.exact;x?.journal?.push(`t=${{x.now?.()??0}} invalid {} value ${{JSON.stringify(String(v))}}; unset`);return null}}return({inner})(v)}}",
                            id.name().replace('_', "-")
                        ))
                    } else {
                        map
                    };
                    (w.name, w.unit, map)
                })
                .collect(),
        };
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
        for (name, unit, map) in writes {
            match map {
                Some(m) => {
                    let _ = write!(
                        self.out,
                        "{s}({e},\"{name}\",\"{unit}\",()=>({m})(({f})()));"
                    );
                }
                None => {
                    let _ = write!(self.out, "{s}({e},\"{name}\",\"{unit}\",{f});");
                }
            }
        }
        Ok(())
    }

    /// A Markdown text field is the web host's own editor (LLP 1045 D5:
    /// markup-editor.js over its wasm), which replaces it once loaded.
    pub(super) fn editor(&mut self, i: u32, element: &str, e: &str) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.nodes[i as usize];
        let markup = row
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .find(|b| b.kind == BindingKind::Prop && b.id == PropId::Markup as u16);
        if element == "textarea" {
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
        let Some(b) = plan.nodes[i as usize]
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
            let row = &plan.nodes[n as usize];
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
        let row = &plan.nodes[i as usize];
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

    /// A node's authored `id`, named for `Sr` in a plan whose rows can
    /// reference an element.
    pub(super) fn exact_id(&self, i: u32) -> Option<String> {
        let plan = self.plan;
        if !self.refs {
            return None;
        }
        plan.nodes[i as usize]
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .find_map(|b| {
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
            "src" if matches!(element, "img" | "video" | "audio") && value.starts_with("app:/") => {
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
            // A select's value is its options' (`element_extras`).
            "value" => match element {
                "input" | "button" | "option" => attrs.push((name.clone(), value.clone())),
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
            "src" if element == "iframe" && !exact_web::host::document::navigable(value) => {
                attrs.push((name.clone(), "about:blank".into()));
            }
            _ => attrs.push((name.clone(), value.clone())),
        }
    }
    (attrs, content, css)
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
    /// the emitted binding wraps its value in `color_map(plan)`, which turns
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
            js.contains(&format!(",\"filter\",\"\",()=>({map})((")),
            "the bound filter is not mapped:\n{js}"
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
                null,
            ])
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
        let map = after
            .strip_prefix(",()=>(")
            .and_then(|mapped| mapped.find(")((").map(|end| mapped[..end].to_string()));
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
        let html = format!(
            r#"<!doctype html><meta charset="utf-8"><style>#flow{{column-count:3;width:320px}}</style><div id="flow">column</div><pre id="out"></pre><script>
const journal=[];
function css(e,prop,unit,v){{const t=v==null?null:typeof v==="number"?v+unit:String(v);if(t==null){{e.style.removeProperty(prop);return}}e.style.removeProperty(prop);e.style.setProperty(prop,t);if(!e.style.getPropertyValue(prop))journal.push(`unset ${{prop}}: ${{JSON.stringify(v)}} is not a value it takes`)}}
const map={map_src};const unit={unit:?};const flow=document.getElementById("flow");
const apply=v=>css(flow,"column-count",unit,map(v));
apply(2);const after2=getComputedStyle(flow).columnCount;const journal2=journal.slice();journal.length=0;
apply(0);const after0=getComputedStyle(flow).columnCount;
document.getElementById("out").textContent=JSON.stringify({{after2,after0,journal2,journal0:journal.slice(),inline:flow.style.getPropertyValue("column-count")}});
</script>"#
        );
        std::fs::write(&page, html).unwrap();
        let chrome = std::env::var("CHROME").unwrap_or_else(|_| {
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into()
        });
        // `--dump-dom` prints the page and then does not exit (a keychain
        // lookup keeps the process up), so read until the result and stop it.
        let mut child = std::process::Command::new(&chrome)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-first-run",
                "--no-default-browser-check",
                "--virtual-time-budget=2000",
                &format!("--user-data-dir={}", dir.join("profile").display()),
                "--dump-dom",
                &format!("file://{}", page.display()),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("chrome ({chrome}): {e}"));
        let mut stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            loop {
                match std::io::Read::read(&mut stdout, &mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.windows(6).any(|w| w == b"</pre>") {
                            break;
                        }
                    }
                }
            }
            let _ = tx.send(buf);
        });
        let dumped = rx.recv_timeout(std::time::Duration::from_secs(20));
        let _ = child.kill();
        let _ = child.wait();
        let dumped = dumped.unwrap_or_else(|_| panic!("chrome ({chrome}) dumped no DOM"));
        let dom = String::from_utf8_lossy(&dumped);
        let raw = dom
            .split_once("<pre id=\"out\">")
            .and_then(|(_, rest)| rest.split_once("</pre>"))
            .map(|(body, _)| body)
            .unwrap_or_else(|| panic!("no computed style in\n{dom}"));
        let computed: serde_json::Value =
            serde_json::from_str(&raw.replace("&quot;", "\"")).expect(raw);
        assert_eq!(computed["after2"], "2", "{computed}");
        assert_eq!(computed["after0"], "auto", "{computed}");
        assert_eq!(computed["journal2"], serde_json::json!([]), "{computed}");
        assert_eq!(computed["journal0"], serde_json::json!([]), "{computed}");
        assert_eq!(computed["inline"], "auto", "{computed}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
