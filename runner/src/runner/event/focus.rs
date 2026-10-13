use super::{DataSource, Event, EventKind, Runner, ViewId};
use exact_kernel::PropId;

impl<D: DataSource> Runner<D> {
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
