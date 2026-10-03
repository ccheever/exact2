//! Symbol images (`image "symbol:<role>"`, LLP 1011) on this host: the
//! role's portable path from the schema, drawn as the web target draws it
//! (`host/web-js/symbols.js`): a 24-unit path, stroked with round caps and
//! joins at a width that follows `font-weight` (or filled even-odd for a
//! `-fill` role), its intrinsic size `font-size` square, placed in the
//! content box by `object-fit`, in the tint or else the text colour.
//! Before this, the host left every symbol box empty.
use super::*;
use exact_kernel::svg::parse_d;

impl Painter {
    /// Paint `node`'s symbol when its source names a known role; whether it did.
    pub(super) fn symbol(
        &mut self,
        node: &NodeRef<'_>,
        content: Rect4,
        tint: Option<[u8; 4]>,
        ts: Transform,
    ) -> bool {
        let Some(role) = node
            .props
            .str(PropId::ImageSource)
            .and_then(|s| s.strip_prefix("symbol:"))
        else {
            return false;
        };
        let Some((_, d, filled)) = exact_kernel::symbol(role) else {
            return false;
        };
        let s = node.style;
        let size = s.font_size.max(0.0);
        if size <= 0.0 || content.2 <= 0.0 || content.3 <= 0.0 {
            return true;
        }
        let (sx, sy) = (content.2 / size, content.3 / size);
        let scale = match s.object_fit {
            ObjectFit::Contain => Some(sx.min(sy)),
            ObjectFit::Cover => Some(sx.max(sy)),
            ObjectFit::None => Some(1.0),
            ObjectFit::ScaleDown => Some(sx.min(sy).min(1.0)),
            ObjectFit::Fill => None,
        };
        let (w, h) = match scale {
            Some(k) => (size * k, size * k),
            None => (content.2, content.3),
        };
        let x = content.0 + (content.2 - w) / 2.0;
        let y = content.1 + (content.3 - h) / 2.0;
        let color = tint.unwrap_or_else(|| rgba(node.text_color().resolve(self.dark)));
        let path = parse_d(d);
        let weight = f32::from(s.font_weight).clamp(100.0, 900.0);
        let paint = SvgPaint {
            path: &path,
            fill: filled.then_some(Ink::Solid(color)),
            even_odd: filled,
            stroke: (!filled).then_some(Ink::Solid(color)),
            order: [0, 1, 2],
            width: 1.1 + (weight - 100.0) / 400.0,
            cap: 1,
            join: 1,
            miter: 4.0,
            dash: Vec::new(),
            phase: 0.0,
        };
        let place = ts.pre_translate(x, y).pre_scale(w / 24.0, h / 24.0);
        self.backend.svg_path(&paint, place);
        true
    }
}
