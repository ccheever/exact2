use super::{DataSource, Event, EventKind, Runner, ViewId};
use exact_kernel::PropId;

impl<D: DataSource> Runner<D> {
    pub(super) fn capture_blur_focus(&self, target: ViewId) -> super::super::FocusPath {
        let mut path = Vec::new();
        let mut at = Some(target);
        while let Some(id) = at {
            if let Some((node, frames)) = self.find(id) {
                path.push((id, node, frames));
            }
            at = self.kernel.node(id).and_then(|node| node.parent);
        }
        path
    }

    pub(super) fn finish_blur_focus(&mut self, target: ViewId, mut path: super::super::FocusPath) {
        // Chrome builds focusout's path after blur. Only the detached subtree
        // survives here; connected handlers still resolve through the live tree.
        if let Some(end) = path
            .iter()
            .position(|(id, _, _)| self.kernel.node(*id).is_some())
        {
            path.truncate(end);
        }
        path.retain(|(_, node, _)| {
            self.plan
                .node(*node)
                .handlers
                .iter()
                .any(|h| self.plan.handler(h).event == EventKind::Focusout)
        });
        if !path.is_empty() {
            self.detached_focus.push((target, path));
        }
    }

    pub(super) fn detached_focus_target(&self, view: ViewId) -> Option<ViewId> {
        self.detached_focus
            .iter()
            .find_map(|(target, path)| path.iter().any(|(id, _, _)| *id == view).then_some(*target))
    }

    pub(super) fn take_detached_focus(
        &mut self,
        view: ViewId,
    ) -> Option<(super::NodesId, Vec<crate::vm::Frame>)> {
        let group = self
            .detached_focus
            .iter()
            .position(|(_, path)| path.iter().any(|(id, _, _)| *id == view))?;
        let path = &mut self.detached_focus[group].1;
        let index = path.iter().position(|(id, _, _)| *id == view)?;
        let (_, node, frames) = path.drain(..=index).next_back()?;
        if path.is_empty() {
            self.detached_focus.remove(group);
        }
        Some((node, frames))
    }

    /// Snapshot a bubbling path before any handler can change the tree.
    pub fn focus_bubble(
        &self,
        target: ViewId,
        related: Option<ViewId>,
        entering: bool,
    ) -> Vec<(ViewId, Event)> {
        let kernel = self.kernel();
        let Some(node) = kernel.node(target) else {
            return Vec::new();
        };
        let target_name = node.props.str(PropId::Id).unwrap_or("").to_owned();
        let related_name = related
            .and_then(|id| kernel.node(id))
            .map(|node| node.props.str(PropId::Id).unwrap_or("").to_owned());
        let kind = if entering {
            EventKind::Focusin
        } else {
            EventKind::Focusout
        };
        let mut path = Vec::new();
        let mut at = Some(target);
        while let Some(id) = at {
            if self.handlers_of(id).contains(&kind) {
                let event = if entering {
                    Event::Focusin {
                        target: target_name.clone(),
                        related_target: related_name.clone(),
                    }
                } else {
                    Event::Focusout {
                        target: target_name.clone(),
                        related_target: related_name.clone(),
                    }
                };
                path.push((id, event));
            }
            at = kernel.node(id).and_then(|node| node.parent);
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::Event;

    #[test]
    fn blur_retains_detached_focusout_binding_and_its_scope() {
        let source = r#"component App
  state shown = true
  state log = ""
  action remove
    shown = false
  action left(label: string, e: FocusEvent)
    log = `${log}${label}:${e.target};`
  view
    column
      if shown
        for label in ["wrapper"] key=label
          column focusout=left(label) testId="wrapper"
            input blur=remove testId="a"
      input testId="c" focus=remove
      text log testId="log"
"#;
        let mut r = super::Runner::boot(
            contract::compile(source).unwrap(),
            (),
            exact_kernel::Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let id = |r: &super::Runner<()>, name| {
            let k = r.kernel();
            k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
        };
        let a = id(&r, "a");
        let wrapper = id(&r, "wrapper");
        r.dispatch(a, Event::Blur).unwrap();
        assert!(r.kernel().node(wrapper).is_none());
        r.dispatch(id(&r, "c"), Event::Focus).unwrap();
        let event = Event::Focusout {
            target: "a".into(),
            related_target: Some("b".into()),
        };
        r.dispatch(wrapper, event.clone()).unwrap();
        assert_eq!(
            r.kernel()
                .node(id(&r, "log"))
                .unwrap()
                .props
                .str(exact_kernel::PropId::Text),
            Some("wrapper:a;")
        );
        assert!(
            r.dispatch(wrapper, event).is_err(),
            "a detached binding is consumed once"
        );
    }

    #[test]
    fn nested_blurs_keep_independent_outgoing_scopes() {
        for stop in [false, true] {
            let source = format!(
                r#"component App
  state first = true
  state second = true
  state log = ""
  action removeFirst
    first = false
  action removeSecond
    second = false
  action left(label: string)
    log = `${{log}}${{label}};`
    {}
  view
    column
      if first
        column focusout=left("outer") testId="outer"
          column focusout=left("first") testId="first"
            input blur=removeFirst testId="a"
      if second
        column focusout=left("second") testId="second"
          input blur=removeSecond testId="b"
      text log testId="log"
"#,
                if stop { "stopPropagation()" } else { "" }
            );
            let mut r = super::Runner::boot(
                contract::compile(&source).unwrap(),
                (),
                exact_kernel::Kernel::with_monospace(),
                Default::default(),
                "/",
            )
            .unwrap();
            let id = |r: &super::Runner<()>, name| {
                let k = r.kernel();
                k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
            };
            let first = id(&r, "first");
            let second = id(&r, "second");
            let outer = id(&r, "outer");
            r.dispatch(id(&r, "a"), Event::Blur).unwrap();
            r.dispatch(id(&r, "b"), Event::Blur).unwrap();
            let event = Event::Focusout {
                target: String::new(),
                related_target: None,
            };
            r.dispatch(second, event.clone()).unwrap();
            r.dispatch(first, event.clone()).unwrap();
            if !stop {
                r.dispatch(outer, event).unwrap();
            }
            assert_eq!(
                r.kernel()
                    .node(id(&r, "log"))
                    .unwrap()
                    .props
                    .str(exact_kernel::PropId::Text),
                Some(if stop {
                    "second;first;"
                } else {
                    "second;first;outer;"
                })
            );
            assert!(
                r.detached_focus.is_empty(),
                "finished or stopped events release their scopes"
            );
        }
    }

    #[test]
    fn focus_wire_preserves_utf8_ids_and_an_empty_related_id() {
        assert_eq!(
            Event::focus_payload(true, "3:é\n1"),
            Some(Event::Focusin {
                target: "é\n".into(),
                related_target: Some("".into())
            }),
        );
        assert_eq!(
            Event::focus_payload(false, "0:0"),
            Some(Event::Focusout {
                target: "".into(),
                related_target: None
            }),
        );
        assert!(Event::focus_payload(true, "4:é\n1").is_none());
    }
}
