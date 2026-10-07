//! A followed link (LLP 1038 §7; LLP 1045 D4): a press on a Markdown
//! reader's link run, an inline run's `href` or a `link href` follows it,
//! as the web's same-document link and Apple's `ExactSession.follow` do. A
//! path naming one of the app's routes is a location for the navigation
//! root's `navigate`; an `http`, `https`, `mailto` or `tel` target leaves
//! the app, and with no browser here it is logged instead.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// The target a press at `(x, y)` on `hit` follows: the link run under
    /// the point in a painted paragraph, else the nearest `href` at or above
    /// the hit node.
    pub(crate) fn link_at(&self, hit: ViewId, x: f32, y: f32) -> Option<String> {
        let kernel = self.host.kernel();
        let node = kernel.node(hit)?;
        if node.node_type == NodeType::Text {
            if let Some(href) = self.run_link(hit, x, y) {
                return Some(href);
            }
        }
        let mut at = Some(hit);
        while let Some(id) = at {
            let node = kernel.node(id)?;
            if let Some(href) = node.props.str(PropId::Href).filter(|h| !h.is_empty()) {
                return Some(href.to_string());
            }
            at = node.parent;
        }
        None
    }

    /// The glyph under a point in `id`'s painted paragraph, and its run's
    /// link: a Markdown run's own target, or an inline run's `href` (its
    /// leaf or an inline ancestor below the paragraph).
    fn run_link(&self, id: ViewId, x: f32, y: f32) -> Option<String> {
        let kernel = self.host.kernel();
        let node = kernel.node(id)?;
        let paragraph = self.paragraph(id)?;
        let b = self.boxes.iter().find(|b| b.id == id)?;
        let content = exact_kernel::svg::scene::content_box(&node);
        let (px, py) = (x - b.rect.0 - content.0, y - b.rect.1 - content.1);
        // The line whose box the point is in: from a little over the glyphs'
        // ascent above its baseline to their descent below it.
        let run = paragraph
            .layout_runs()
            .zip(paragraph.baselines())
            .flat_map(|(line, baseline)| line.glyphs.iter().map(move |g| (g, *baseline)))
            .find(|(g, baseline)| {
                (g.x..g.x + g.w).contains(&px)
                    && (baseline - g.font_size * 1.05..baseline + g.font_size * 0.35).contains(&py)
            })?
            .0
            .run();
        let shown = paragraph.runs().get(run)?;
        if node.props.str(PropId::Markup) == Some("markdown") {
            return Some(shown.href.clone()).filter(|h| !h.is_empty());
        }
        let mut palette = Vec::new();
        crate::paint::inline::text_palette(kernel, &node, false, None, &mut palette);
        let mut at = palette.get(run).map(|p| p.source);
        while let Some(leaf) = at.filter(|leaf| *leaf != id) {
            let leaf = kernel.node(leaf)?;
            if let Some(href) = leaf.props.str(PropId::Href).filter(|h| !h.is_empty()) {
                return Some(href.to_string());
            }
            at = leaf.parent;
        }
        None
    }

    /// Follow `href` from `from`: a path naming a declared route navigates
    /// the navigation root above `from`; anything the reader keeps leaves the
    /// app, which this host has no browser to open.
    pub(crate) fn follow(&mut self, from: ViewId, href: &str, now_ms: f64) {
        let Some(target) = crate::text::markup::target(href) else {
            self.host.log(format!(
                "link {href:?}: only an app path, http, https, mailto and tel are followed"
            ));
            return;
        };
        if target.starts_with('/') && self.host.runner().route_matches(target) {
            let kernel = self.host.kernel();
            let mut at = Some(from);
            while let Some(id) = at {
                let Some(node) = kernel.node(id) else { break };
                if node.props.str(PropId::NavigationBack).is_some()
                    && self
                        .host
                        .runner()
                        .handlers_of(id)
                        .contains(&EventKind::Navigate)
                {
                    let location = target.to_string();
                    if let Some(error) =
                        self.host.dispatch_at(id, Event::Navigate(location), now_ms)
                    {
                        self.host.log(error);
                    }
                    if let Some(error) = self.after_commit() {
                        self.host.log(error);
                    }
                    return;
                }
                at = node.parent;
            }
            self.host.log(format!(
                "link {target}: no navigation root takes `navigate` here"
            ));
            return;
        }
        // No browser, mail or phone app on the headless/DRM host.
        self.host.log(format!(
            "openURL {target}: the Linux host has no browser to open it in"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::PainterChoice;
    use exact_runner::{DataError, Value};

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }

    const SRC: &str = r##"routes nav
  home "/"
  note "/note/:note"
  notfound

component App
  derive current = top(nav)
  action follow(url: string)
    nav = go(nav, url)
  view
    main navigationKey=`${current.id}` navigationBack="back" navigate=follow width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` testId=`route-${e.name}` position="absolute" inset=0 padding=10 background-color="#fff"
          text "- [Note](/note/3) and [out](https://example.com) and [inert](ftp://x)" markup="markdown" testId=`reader-${e.name}` font-size=16
"##;

    /// Where the run linking to `href` paints in `reader`, in window points.
    fn link(p: &mut Presenter<NoData>, reader: &str, href: &str) -> (f32, f32) {
        p.frame();
        let kernel = p.host.kernel();
        let id = kernel
            .node_by_key(kernel.find_by_test_id(reader)[0])
            .unwrap()
            .id;
        let content = exact_kernel::svg::scene::content_box(&kernel.node(id).unwrap());
        let rect = p.rect_of(id).unwrap();
        let paragraph = p.paragraph(id).unwrap();
        let (line, baseline) = paragraph
            .layout_runs()
            .zip(paragraph.baselines())
            .next()
            .unwrap();
        let g = line
            .glyphs
            .iter()
            .find(|g| paragraph.runs()[g.run()].href == href)
            .unwrap();
        (
            rect.0 + content.0 + g.x + g.w / 2.0,
            rect.1 + content.1 + baseline - 4.0,
        )
    }

    #[test]
    fn a_markdown_link_to_a_route_navigates_and_one_out_of_the_app_is_logged() {
        let (mut p, error) = Presenter::boot_with(
            &contract::compile(SRC).unwrap().encode(),
            NoData,
            (400., 300.),
            1.,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        // An inert target paints as words and is no link (`markup::target`).
        assert!(p
            .paragraph_runs_with_href("reader-home")
            .iter()
            .all(|h| h != "ftp://x"));
        let (x, y) = link(&mut p, "reader-home", "/note/3");
        p.press_at(x, y, 10.);
        assert!(
            !p.host.kernel().find_by_test_id("route-note").is_empty(),
            "the route was pushed"
        );
        let (x, y) = link(&mut p, "reader-note", "https://example.com");
        p.press_at(x, y, 20.);
        assert!(
            p.host
                .runner()
                .journal()
                .any(|l| l.contains("openURL https://example.com")),
            "{:?}",
            p.host.runner().journal().collect::<Vec<_>>()
        );
    }

    impl Presenter<NoData> {
        fn paragraph_runs_with_href(&mut self, reader: &str) -> Vec<String> {
            self.frame();
            let kernel = self.host.kernel();
            let id = kernel
                .node_by_key(kernel.find_by_test_id(reader)[0])
                .unwrap()
                .id;
            let runs = self.paragraph(id).unwrap().runs();
            runs.iter()
                .map(|r| r.href.clone())
                .filter(|h| !h.is_empty())
                .collect()
        }
    }
}
