//! A followed link (LLP 1038 §7; LLP 1045 D4): a press on a Markdown
//! reader's link run, an inline run's `href` or a `link href` follows it,
//! as the web's same-document link and Apple's `ExactSession.follow` do. A
//! path naming one of the app's routes is a location for the navigation
//! root's `navigate`; an `http`, `https`, `mailto` or `tel` target leaves
//! the app: the host's shell opens it (LLP 1115.001 D5), and a host with no
//! shell, which has no browser, logs it instead.
use super::*;

pub(super) type UrlOpener = Box<dyn Fn(&str) -> Result<(), String>>;

impl<D: DataSource> Presenter<D> {
    /// Use the window's default browser, mail app or dialer for external links.
    pub fn set_url_opener(&mut self, open: impl Fn(&str) -> Result<(), String> + 'static) {
        self.url_opener = Some(Box::new(open));
    }

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
            if let Some(href) = node.props.link() {
                return Some(href.to_string());
            }
            at = node.parent;
        }
        None
    }

    /// The glyph under a point in `id`'s painted paragraph, and its run's
    /// link: a Markdown run's own target, or an inline run's `href` (its
    /// leaf or an inline ancestor below the paragraph).
    pub(super) fn run_link(&self, id: ViewId, x: f32, y: f32) -> Option<String> {
        let run = self.inline_run_at(id, x, y)?;
        let kernel = self.host.kernel();
        let node = kernel.node(id)?;
        if node.props.str(PropId::Markup) == Some("markdown") {
            let href = &self.paragraph(id)?.runs().get(run)?.href;
            return (!href.is_empty()).then(|| href.clone());
        }
        let mut at = inline_sources(&node).get(run).copied();
        while let Some(leaf) = at.filter(|leaf| *leaf != id) {
            let leaf = kernel.node(leaf)?;
            if let Some(href) = leaf.props.link() {
                return Some(href.to_string());
            }
            at = leaf.parent;
        }
        None
    }

    fn inline_run_at(&self, id: ViewId, x: f32, y: f32) -> Option<usize> {
        let _roles = self.host.runner().bind_roles();
        let kernel = self.host.kernel();
        let node = kernel.node(id)?;
        let paragraph = self.paragraph(id)?;
        let b = self.boxes.iter().find(|b| b.id == id)?;
        let content = exact_kernel::svg::scene::content_box(kernel, &node);
        let down = kernel.text_line_offset(&node);
        let (px, py) = (x - b.rect.0 - content.0, y - b.rect.1 - content.1 - down);
        // The line whose box the point is in: from a little over the glyphs'
        // ascent above its baseline to their descent below it.
        Some(
            paragraph
                .layout_runs()
                .zip(paragraph.baselines())
                .flat_map(|(line, baseline)| line.glyphs.iter().map(move |g| (g, *baseline)))
                .find(|(g, baseline)| {
                    (g.x..g.x + g.w).contains(&px)
                        && (baseline - g.font_size * 1.05..baseline + g.font_size * 0.35)
                            .contains(&py)
                })?
                .0
                .run(),
        )
    }

    pub(super) fn inline_hit(&self, id: ViewId, x: f32, y: f32) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let node = kernel.node(id)?;
        if node.node_type != NodeType::Text || node.props.str(PropId::Text).is_some() {
            return None;
        }
        let run = self.inline_run_at(id, x, y)?;
        let mut at = inline_sources(&node).get(run).copied();
        while let Some(leaf) = at.filter(|leaf| *leaf != id) {
            let leaf = kernel.node(leaf)?;
            if leaf.computed_row(exact_kernel::StyleId::PointerEvents, |s| s.pointer_events)
                != exact_kernel::PointerEvents::None
                && crate::paint::paints(kernel, leaf.id, None)
                && (leaf.props.link().is_some()
                    || self
                        .host
                        .runner()
                        .handlers_of(leaf.id)
                        .contains(&EventKind::Press))
            {
                return Some(leaf.id);
            }
            at = leaf.parent;
        }
        None
    }

    /// Inline nodes share their paragraph's paint box. Their own geometry
    /// comes from the glyphs, including wrapping and the paragraph's scroll.
    pub(super) fn inline_box(&self, id: ViewId) -> Option<PaintedBox> {
        let kernel = self.host.kernel();
        let node = kernel.node(id)?;
        if node.node_type != NodeType::Text || !crate::paint::paints(kernel, id, None) {
            return None;
        }
        let mut parent = node.parent?;
        let mut painted = loop {
            let node = kernel.node(parent)?;
            if node.node_type != NodeType::Text {
                return None;
            }
            if let Some(b) = self.boxes.iter().find(|b| b.id == parent) {
                break *b;
            }
            parent = node.parent?;
        };
        let node = kernel.node(parent)?;
        let sources = inline_sources(&node);
        let owns: Vec<bool> = sources
            .iter()
            .map(|source| {
                let mut at = Some(*source);
                while let Some(leaf) = at.filter(|leaf| *leaf != parent) {
                    if leaf == id {
                        return true;
                    }
                    at = kernel.node(leaf).and_then(|n| n.parent);
                }
                false
            })
            .collect();
        let paragraph = self.paragraph(parent)?;
        let content = exact_kernel::svg::scene::content_box(kernel, &node);
        let down = kernel.text_line_offset(&node);
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        for (line, baseline) in paragraph.layout_runs().zip(paragraph.baselines()) {
            for g in line
                .glyphs
                .iter()
                .filter(|g| owns.get(g.run()) == Some(&true))
            {
                let r = (
                    g.x,
                    baseline - g.font_size,
                    g.x + g.w,
                    baseline + g.font_size * 0.3,
                );
                bounds = Some(bounds.map_or(r, |b| {
                    (b.0.min(r.0), b.1.min(r.1), b.2.max(r.2), b.3.max(r.3))
                }));
            }
        }
        let (x, y, right, bottom) = bounds?;
        painted.id = id;
        painted.rect = (
            painted.rect.0 + content.0 + x,
            painted.rect.1 + content.1 + down + y,
            right - x,
            bottom - y,
        );
        Some(painted)
    }

    /// Follow `href` from `from`: a path naming a declared route navigates
    /// the navigation root above `from`; anything else the reader keeps
    /// leaves the app, where the host's shell can open it.
    pub(crate) fn follow(&mut self, from: ViewId, href: &str, now_ms: f64) {
        // A link to a fragment (`href="#moves"`) is the visit shown, at that
        // fragment (LLP 1153 D5).
        let shown = (href.trim().starts_with('#'))
            .then(|| {
                self.host
                    .router_op()
                    .map(|op| format!("{}{}", op.url, href.trim()))
            })
            .flatten();
        let Some(target) = shown
            .as_deref()
            .or_else(|| crate::text::markup::target(href))
        else {
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
        // The shell's browser, mail app or dialer.
        if self.leave(target) {
            return;
        }
        // No browser, mail or phone app on the headless/DRM host.
        self.host.log(format!(
            "openURL {target}: the Linux host has no browser to open it in"
        ));
    }

    /// A URL arriving from outside while the app runs (LLP 1133.023 D15
    /// item 3: Android's `onNewIntent`, a shortcut): its location, as a
    /// link's, goes to the navigation root's `navigate` when the root routes
    /// it. The refusal, else `None`.
    pub fn open_location(&mut self, href: &str, now_ms: f64) -> Option<String> {
        let location = exact_route::location_of(href);
        if !self.host.runner().route_matches(&location) {
            return Some(format!("open {location}: no route matches it"));
        }
        let root = self.navigation_root().filter(|root| {
            self.host
                .runner()
                .handlers_of(*root)
                .contains(&EventKind::Navigate)
        });
        let Some(root) = root else {
            return Some(format!(
                "open {location}: no navigation root takes `navigate`"
            ));
        };
        self.host
            .dispatch_at(root, Event::Navigate(location), now_ms)
            .or(self.after_commit())
    }
}

fn inline_sources(node: &exact_kernel::NodeRef<'_>) -> Vec<ViewId> {
    fn collect(node: &exact_kernel::NodeRef<'_>, out: &mut Vec<ViewId>) {
        if node.props.str(PropId::Text).is_some() {
            out.push(node.id);
        } else {
            for child in node.child_nodes().filter(|n| n.node_type == NodeType::Text) {
                collect(&child, out);
            }
        }
    }
    let mut out = Vec::new();
    collect(node, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::PainterChoice;

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
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` testId=`route-${e.name}` position="absolute" inset=0 padding=10 background-color="#fff"
          text "- [Note](/note/3) and [out](https://example.com) and [inert](ftp://x)" markup="markdown" testId=`reader-${e.name}` font-size=16
"##;

    /// Where the run linking to `href` paints in `reader`, in window points.
    fn link(p: &mut Presenter<()>, reader: &str, href: &str) -> (f32, f32) {
        p.frame();
        let kernel = p.host.kernel();
        let id = kernel
            .node_by_key(kernel.find_by_test_id(reader)[0])
            .unwrap()
            .id;
        let content = exact_kernel::svg::scene::content_box(kernel, &kernel.node(id).unwrap());
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
            &crate::test_plan(SRC),
            (),
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

    #[test]
    fn a_url_from_outside_goes_to_the_root_when_it_routes_it() {
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(SRC),
            (),
            (400., 300.),
            1.,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        // A scheme's URL as Android's intent data carries it: its location, as a link's.
        assert_eq!(p.open_location("shelf://note/7", 10.), None);
        assert!(!p.host.kernel().find_by_test_id("route-note").is_empty());
        let notes = p.host.kernel().find_by_test_id("route-note").len();
        assert_eq!(p.open_location("/note/8?from=shortcut", 20.), None);
        assert_eq!(
            p.host.kernel().find_by_test_id("route-note").len(),
            notes + 1
        );
        // A path no route matches is refused, and nothing moves.
        assert_eq!(
            p.open_location("/elsewhere", 30.).as_deref(),
            Some("open /elsewhere: no route matches it")
        );
        assert_eq!(
            p.host.kernel().find_by_test_id("route-note").len(),
            notes + 1
        );
    }

    impl Presenter<()> {
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

    #[test]
    fn desktop_links_and_open_url_commands_use_the_registered_handler() {
        use std::{
            cell::{Cell, RefCell},
            rc::Rc,
        };
        let source = r#"component App
  action launch
    openURL("https://example.com/command?x=1&y=2")
  view
    column width=400 height=300
      text "[Open](https://example.com/link)" markup="markdown" testId="reader" font-size=16
      button press=launch testId="launch" height=40
        text "Open a URL"
"#;
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            (),
            (400., 300.),
            1.,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let opened = Rc::new(RefCell::new(Vec::new()));
        let fail = Rc::new(Cell::new(false));
        p.set_url_opener({
            let (opened, fail) = (opened.clone(), fail.clone());
            move |url| {
                opened.borrow_mut().push(url.to_owned());
                if fail.get() {
                    Err("no registered handler".into())
                } else {
                    Ok(())
                }
            }
        });
        let (x, y) = link(&mut p, "reader", "https://example.com/link");
        p.press_at(x, y, 10.);
        let launch = p.host.kernel().find_by_test_id("launch")[0];
        p.tap(p.host.kernel().node_by_key(launch).unwrap().id)
            .unwrap();
        p.run_commands(|| ());
        assert_eq!(
            &*opened.borrow(),
            &[
                "https://example.com/link",
                "https://example.com/command?x=1&y=2",
            ]
        );
        for url in [
            " HTTP://example.com/café ",
            "mailto:hello@example.com",
            "tel:+15555550123",
        ] {
            assert!(p.leave(url));
            assert_eq!(opened.borrow().last().unwrap(), url.trim());
        }
        let before = opened.borrow().len();
        for url in [
            "/route",
            "//example.com",
            "file:///test.exe",
            "javascript:alert(1)",
            "ftp://example.com",
            "https://example.com\0ignored",
        ] {
            assert!(!p.leave(url), "{url:?}");
        }
        p.open_url(&[exact_plan::Value::from("app-settings:")]);
        assert_eq!(opened.borrow().len(), before);
        assert!(p.take_requests().is_none());
        fail.set(true);
        assert!(p.leave("https://example.com/unavailable"));
        assert_eq!(opened.borrow().len(), before + 1);
        assert!(p
            .host
            .runner()
            .journal()
            .any(|line| line.contains("no registered handler")));
    }
}
