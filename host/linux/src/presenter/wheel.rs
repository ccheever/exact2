use super::*;

impl<D: DataSource> Presenter<D> {
    pub(crate) fn desktop_scroll_focus(&self, id: ViewId) -> bool {
        cfg!(windows)
            && self.host.kernel().node(id).is_some_and(|n| {
                let (x, y) = effective_overflow(&n);
                !matches!(n.node_type, NodeType::TextInput | NodeType::Control)
                    && (matches!(x, Overflow::Auto | Overflow::Scroll)
                        || matches!(y, Overflow::Auto | Overflow::Scroll))
            })
            && self
                .scroller(id, &self.collection_scroll_limits())
                .is_some_and(|s| s.axes.0 || s.axes.1)
    }

    /// Windows ScrollView keys leave focus in the region being scrolled.
    /// https://learn.microsoft.com/windows/apps/design/input/keyboard-interactions
    pub(super) fn desktop_scroll_key(&mut self, name: &str) -> bool {
        let held = self.modifiers();
        if !cfg!(windows)
            || held.alt
            || held.meta
            || held.shift
            || held.ctrl && !matches!(name, "Home" | "End")
            || !matches!(
                name,
                "PageUp"
                    | "PageDown"
                    | "Home"
                    | "End"
                    | "ArrowUp"
                    | "ArrowDown"
                    | "ArrowLeft"
                    | "ArrowRight"
            )
        {
            return false;
        }
        if self.focus.is_some_and(|id| {
            self.input_surface(id).is_some()
                || self.host.kernel().node(id).is_none_or(|n| {
                    n.node_type == NodeType::TextInput
                        || n.node_type == NodeType::Control
                            && exact_kernel::ControlKind::of(n.node_type, n.props)
                                != Some(exact_kernel::ControlKind::Button)
                        || matches!(
                            n.props.str(PropId::AccessibilityRole),
                            Some(
                                "textbox"
                                    | "searchbox"
                                    | "combobox"
                                    | "slider"
                                    | "spinbutton"
                                    | "listbox"
                            )
                        )
                })
        }) {
            return false;
        }
        self.restore_scroll_anchors();
        let limits = self.collection_scroll_limits();
        let mut at = self.focus;
        loop {
            let id = at.unwrap_or(PAGE);
            if let Some(s) = self.scroller(id, &limits) {
                let horizontal = matches!(name, "ArrowLeft" | "ArrowRight")
                    || matches!(name, "Home" | "End") && !s.axes.1 && !held.ctrl;
                let active = if horizontal { s.axes.0 } else { s.axes.1 };
                if active {
                    let port = if id == PAGE {
                        s.port
                    } else if let Some(b) = self.box_of(id) {
                        [b.rect.0, b.rect.1, b.rect.2, b.rect.3]
                    } else {
                        return false;
                    };
                    let mut to = s.offset;
                    let (value, max, page) = if horizontal {
                        (&mut to.0, s.max.0, port[2])
                    } else {
                        (&mut to.1, s.max.1, port[3])
                    };
                    *value = match name {
                        "Home" => 0.,
                        "End" => max,
                        "PageUp" => *value - page,
                        "PageDown" => *value + page,
                        "ArrowUp" | "ArrowLeft" => *value - 16.,
                        _ => *value + 16.,
                    };
                    self.scroll_to(id, to);
                    return true;
                }
            }
            if id == PAGE || self.alert_inert() && self.alert_root() == Some(id) {
                return false;
            }
            at = self.scroll_parent(id);
        }
    }

    /// A wheel at a point (the web's sign: a positive `dy` scrolls down).
    /// A phase-less tick is its own gesture and is not split (LLP 1070 G2,
    /// Chrome's measured tick): the innermost scroll container under the point
    /// that can take ANY of its components takes those it can, clamped, and
    /// the rest is dropped. One that can take none chains to the next scroller
    /// up, then the page, unless its `overscroll-behavior` is `contain` or
    /// `none` on an axis the tick moves along: it keeps (drops) the tick.
    pub fn wheel_at(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        if ![x, y, dx, dy].iter().all(|v| v.is_finite())
            || (dx == 0.0 && dy == 0.0)
            || !self.display.contains(x, y)
        {
            return;
        }
        if self.overlay_wheel(x, y, dy) || self.alert_inert() && self.hit(x, y).is_none() {
            return;
        }
        self.person_scrolls += 1;
        self.restore_scroll_anchors();
        // A canvas that wants input takes the wheel, and the nodes' own
        // `wheel` is still heard (review b5-b 2), as the web's element hears it.
        let canvas = self.surface_wheel(x, y, dx, dy, self.pointer_now());
        if self.wheel_event(x, y, dx, dy) || canvas {
            return;
        }
        // A UI wheel can take over a UI gesture, but does not release a game's
        // captured pointer. Browser wheel events do not end pointer capture.
        if self.contact_canvas().is_none() {
            if let Err(error) = self.pointer_cancel(self.pointer_now()) {
                self.host.log(error);
            }
        }
        let mut at = self.hit(x, y);
        // Only a node the display has no bounds for needs the collections'.
        let mut collection_limits = None;
        while let Some(id) = at {
            let kernel = self.host.kernel();
            let Some(node) = kernel.node(id) else { break };
            let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
                let limits =
                    collection_limits.get_or_insert_with(|| self.collection_scroll_limits());
                self.brush.scroll_bounds(
                    kernel,
                    self.host.content_region(),
                    &node,
                    limits.get(&id).copied(),
                )
            });
            let (ox, oy) = bounds.axes;
            let scrolls = matches!(ox, Overflow::Scroll | Overflow::Auto)
                || matches!(oy, Overflow::Scroll | Overflow::Auto);
            let contained =
                |b: exact_kernel::OverscrollBehavior| b != exact_kernel::OverscrollBehavior::Auto;
            let stops = scrolls
                && ((dx != 0.0 && contained(node.style.overscroll_behavior_x))
                    || (dy != 0.0 && contained(node.style.overscroll_behavior_y)));
            // An SVG element has no display box: its ancestors scroll it.
            let parent = self.scroll_parent(id).or_else(|| {
                node.node_type
                    .is_svg_element()
                    .then_some(node.parent)
                    .flatten()
            });
            if self.scroll_container_by(id, bounds, dx, dy) {
                self.last_wheel = Some(id);
                return;
            }
            // At every requested edge, containment or the modal ends the chain.
            if stops || self.alert_inert() && self.alert_root() == Some(id) {
                return;
            }
            at = parent;
        }
        if self.alert_inert() {
            return;
        }
        let doc = self.document();
        let viewport = self.display.viewport().unwrap_or(self.viewport);
        let max = ((doc.0 - viewport.0).max(0.0), (doc.1 - viewport.1).max(0.0));
        let next = (
            (self.page.0 + dx).clamp(0.0, max.0),
            (self.page.1 + dy).clamp(0.0, max.1),
        );
        let moved = self.anchor_input(PAGE, self.page, dy, next);
        if next != self.page || moved.is_some_and(|dy| dy != 0.) {
            self.page = next;
            self.dirty = true;
            if let Some(error) = self.refresh_transform_geometry() {
                self.host.log(error);
            }
        }
    }

    /// Apply one input to a container. False means neither axis can move.
    pub(super) fn scroll_container_by(
        &mut self,
        id: ViewId,
        bounds: crate::paint::ScrollBounds,
        dx: f32,
        dy: f32,
    ) -> bool {
        let (ox, oy) = bounds.axes;
        let max = bounds.max;
        let off = self.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
        let takes = |axis: Overflow, d: f32, off: f32, max: f32| {
            matches!(axis, Overflow::Scroll | Overflow::Auto)
                && d != 0.0
                && max > 0.0
                && ((d > 0.0 && off < max) || (d < 0.0 && off > 0.0))
        };
        let take_x = takes(ox, dx, off.0, max.0);
        let (input_y, input_max) = self
            .anchor_input_position(id, off.1)
            .unwrap_or((off.1, max.1));
        let take_y = takes(oy, dy, input_y, input_max);
        if !take_x && !take_y {
            return false;
        }
        let nx = if take_x {
            (off.0 + dx).clamp(0.0, max.0)
        } else {
            off.0
        };
        let ny = if take_y {
            (off.1 + dy).clamp(0.0, max.1)
        } else {
            off.1
        };
        // A wheel is a new input: the list's own pending position is retired now (`ack.rs`).
        self.retire_for_wheel(id);
        let moved = self.anchor_input(id, off, if take_y { dy } else { 0. }, (nx, ny));
        self.scroll.insert(id, (nx, ny));
        if (nx, ny) != off || moved.is_some_and(|dy| dy != 0.) {
            self.scroll_end_input(id, off, Completion::Instant);
        }
        self.dirty = true;
        self.collection_scrolled_or_deferred(id, off);
        if let Some(error) = self.refresh_transform_geometry() {
            self.host.log(error);
        }
        true
    }
}

#[cfg(all(test, windows))]
mod keyboard_tests {
    use super::*;

    fn boot(source: &str) -> Presenter<()> {
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            (),
            (280., 260.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        p
    }

    fn id(p: &Presenter<()>, name: &str) -> ViewId {
        let k = p.host.kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    }

    fn key(p: &mut Presenter<()>, name: &str) {
        let now = p.host.now();
        p.key_down(name, now + 1.);
        p.key_up(name, name, now + 2.);
        p.frame();
    }

    fn click(p: &mut Presenter<()>, x: f32, y: f32) {
        let now = p.host.now();
        p.pointer_down(x, y, now + 1.).unwrap();
        p.pointer_up(x, y, now + 2.).unwrap();
    }

    #[test]
    fn windows_scroller_takes_tab_click_and_navigation_keys_without_editing() {
        let mut p = boot(
            r#"component App
  view
    column width=280 height=260
      button "Before" testId="before" height=32
      column width=240 height=160 flex-shrink=0 overflow="auto" testId="list"
        box width=800 height=1000 flex-shrink=0
          text "Transcript"
      input value="draft" testId="editor" height=32
"#,
        );
        let list = id(&p, "list");
        key(&mut p, "Tab");
        assert_eq!(p.focus, Some(id(&p, "before")));
        key(&mut p, "Tab");
        assert_eq!(p.focus, Some(list));
        for (name, y) in [
            ("PageDown", 160.),
            ("PageDown", 320.),
            ("ArrowUp", 304.),
            ("Home", 0.),
            ("End", 840.),
            ("ArrowDown", 840.),
            ("PageUp", 680.),
        ] {
            key(&mut p, name);
            assert_eq!(p.scroll_of(list).1, y, "{name}");
            assert_eq!(p.focus, Some(list));
        }
        key(&mut p, "ArrowRight");
        assert_eq!(p.scroll_of(list).0, 16.);
        p.hold_modifier("ControlLeft", true);
        key(&mut p, "Home");
        p.hold_modifier("ControlLeft", false);
        assert_eq!(p.scroll_of(list), (16., 0.));
        key(&mut p, "ArrowLeft");
        key(&mut p, "Tab");
        let editor = id(&p, "editor");
        assert_eq!(p.focus, Some(editor));
        for name in ["Home", "End", "PageDown", "ArrowDown"] {
            key(&mut p, name);
            assert_eq!(p.scroll_of(list), (0., 0.));
            assert_eq!(p.field_text(editor), "draft");
        }
        click(&mut p, 50., 80.);
        assert_eq!(p.focus, Some(list));
        key(&mut p, "PageDown");
        assert_eq!(p.scroll_of(list).1, 160.);
    }

    #[test]
    fn windows_scroll_keys_stay_in_the_focused_region_at_its_edge() {
        let mut p = boot(
            r#"component App
  view
    column width=240 height=200 overflow-y="auto" testId="outer"
      column width=200 height=100 flex-shrink=0 overflow-y="auto" testId="inner"
        box height=500 flex-shrink=0
          text "Inner"
      box height=1000 flex-shrink=0
"#,
        );
        let inner = id(&p, "inner");
        let outer = id(&p, "outer");
        click(&mut p, 30., 30.);
        assert_eq!(p.focus, Some(inner));
        key(&mut p, "End");
        assert_eq!(p.scroll_of(inner).1, 400.);
        key(&mut p, "PageDown");
        assert_eq!(p.scroll_of(inner).1, 400.);
        assert_eq!(p.scroll_of(outer).1, 0.);
        p.set_focus(Some(outer), p.host.now() + 1.);
        key(&mut p, "PageDown");
        assert_eq!(p.scroll_of(outer).1, 200.);
    }

    #[test]
    fn windows_virtualized_transcript_takes_focus_and_pages_through_its_model_extent() {
        let rows = (0..100)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            r#"component App
  state rows = [{rows}]
  view
    list virtualized=true scroll-start="end" estimated-item-height=40 width=240 height=200 testId="list"
      for n in rows key=n
        text `Message ${{n}}` height=40
"#
        );
        let mut p = boot(&source);
        for _ in 0..16 {
            p.pump(p.host.now());
            if !p.dirty() && !p.collection.pending() {
                break;
            }
            p.frame();
        }
        let list = id(&p, "list");
        let bottom = p.scroll_of(list).1;
        assert!(bottom > 3000.);
        assert!(p.host.collections()[0].rows.len() < 100);
        click(&mut p, 150., 100.);
        assert_eq!(p.focus, Some(list));
        key(&mut p, "PageUp");
        assert_eq!(p.scroll_of(list).1, bottom - 200.);
        key(&mut p, "Home");
        assert_eq!(p.scroll_of(list).1, 0.);
        key(&mut p, "End");
        assert_eq!(p.scroll_of(list).1, bottom);
        assert_eq!(p.focus, Some(list));
    }

    #[test]
    fn windows_scroll_keys_respect_prevention_modifiers_and_negative_tabindex() {
        let mut p = boot(
            r#"component App
  action handled(k: string)
    if k == "PageDown"
      preventDefault()
  view
    column width=240 height=200 overflow-y="auto" tabindex=-1 key=handled testId="list"
      box height=1000 flex-shrink=0
        text "Transcript"
"#,
        );
        let list = id(&p, "list");
        key(&mut p, "Tab");
        assert_eq!(p.focus, None);
        click(&mut p, 30., 30.);
        assert_eq!(p.focus, Some(list));
        key(&mut p, "PageDown");
        assert_eq!(p.scroll_of(list).1, 0.);
        for modifier in ["ShiftLeft", "AltLeft", "MetaLeft"] {
            p.hold_modifier(modifier, true);
            key(&mut p, "End");
            p.hold_modifier(modifier, false);
            assert_eq!(p.scroll_of(list).1, 0.);
        }
        key(&mut p, "End");
        assert_eq!(p.scroll_of(list).1, 800.);
    }
}
