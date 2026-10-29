//! UI Automation over the presenter's accessibility model; dormant until requested.
use accesskit::{
    Action, ActionData, Affine, Live, Node, NodeId, Rect, Role, Toggled, TreeId, TreeInfo,
    TreeUpdate,
};
use accesskit_winit::{Adapter, WindowEvent as AccessEvent};
use exact_kernel::PropId;
use exact_linux::{
    presenter::access::{act, flag, role, Node as Source},
    Presenter,
};
use exact_runner::{DataSource, FieldSelection, SelectionDirection};
use std::collections::BTreeMap;
use unicode_segmentation::UnicodeSegmentation;
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::Window,
};

pub(super) enum Event {
    Wake,
    ModalWake,
    Access(accesskit_winit::Event),
}
impl From<accesskit_winit::Event> for Event {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::Access(event)
    }
}

const ROOT: NodeId = NodeId(0);

pub(super) struct Access {
    adapter: Adapter,
    active: bool,
    nodes: BTreeMap<NodeId, Node>,
    runs: BTreeMap<NodeId, (i32, Vec<u32>)>,
    focus: NodeId,
}

impl Access {
    pub fn new(events: &ActiveEventLoop, window: &Window, proxy: EventLoopProxy<Event>) -> Self {
        Self {
            adapter: Adapter::with_event_loop_proxy(events, window, proxy),
            active: false,
            nodes: BTreeMap::new(),
            runs: BTreeMap::new(),
            focus: ROOT,
        }
    }

    pub fn process_event(&mut self, window: &Window, event: &WindowEvent) {
        self.adapter.process_event(window, event);
    }

    pub fn event<D: DataSource>(
        &mut self,
        p: &mut Presenter<D>,
        window: &Window,
        title: &str,
        event: AccessEvent,
    ) {
        match event {
            AccessEvent::InitialTreeRequested => {
                self.active = true;
                self.nodes.clear();
                p.access_reset();
            }
            AccessEvent::AccessibilityDeactivated => {
                self.active = false;
                self.nodes.clear();
                self.runs.clear();
                p.access_reset();
                return;
            }
            AccessEvent::ActionRequested(request) => {
                if !self.active || request.target_tree != TreeId::ROOT {
                    return;
                }
                self.sync(p, window, title);
                let Some(node) = self.nodes.get(&request.target_node) else {
                    return;
                };
                if node.is_disabled() || !node.supports_action(request.action) {
                    return;
                }
                let Ok(id) = i32::try_from(request.target_node.0) else {
                    return;
                };
                let now = p.host().now();
                match (request.action, request.data) {
                    (Action::Focus, _) => {
                        p.access_focus_node(id, now);
                    }
                    (Action::Click, _) => {
                        p.access_act(id, act::CLICK, 0., now);
                    }
                    (Action::ShowContextMenu, _) => {
                        p.access_act(id, act::LONG_CLICK, 0., now);
                    }
                    (Action::Increment | Action::ScrollDown | Action::ScrollRight, _) => {
                        p.access_act(id, act::FORWARD, 0., now);
                    }
                    (Action::Decrement | Action::ScrollUp | Action::ScrollLeft, _) => {
                        p.access_act(id, act::BACKWARD, 0., now);
                    }
                    (Action::SetValue, Some(ActionData::NumericValue(value)))
                        if (value as f32).is_finite() =>
                    {
                        p.access_act(id, act::SET, value as f32, now);
                    }
                    (Action::SetValue, Some(ActionData::Value(value))) => {
                        if let Some(view) = p.access_view(id) {
                            if let Err(error) = p.type_text(view, &value) {
                                p.host_mut().log(error);
                            }
                        }
                    }
                    (Action::SetTextSelection, Some(ActionData::SetTextSelection(selection))) => {
                        let position = |at: accesskit::TextPosition| {
                            let (owner, offsets) = self.runs.get(&at.node)?;
                            (*owner == id)
                                .then(|| offsets.get(at.character_index).copied())
                                .flatten()
                        };
                        if let (Some(anchor), Some(focus)) =
                            (position(selection.anchor), position(selection.focus))
                        {
                            p.access_select(
                                id,
                                FieldSelection {
                                    start: anchor.min(focus),
                                    end: anchor.max(focus),
                                    direction: if focus < anchor {
                                        SelectionDirection::Backward
                                    } else {
                                        SelectionDirection::Forward
                                    },
                                },
                                now,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        self.sync(p, window, title);
    }

    pub fn sync<D: DataSource>(&mut self, p: &mut Presenter<D>, window: &Window, title: &str) {
        if !self.active {
            return;
        }
        p.reader_access(1., self.nodes.is_empty());
        let sources = p.access_told();
        let origin = p.access_origin();
        let focus = p.access_focus().map_or(ROOT, |id| NodeId(id as u64));
        let mut next = BTreeMap::new();
        let mut children_by_parent: BTreeMap<i32, Vec<NodeId>> = BTreeMap::new();
        for source in sources {
            children_by_parent
                .entry(source.parent)
                .or_default()
                .push(NodeId(source.id as u64));
        }
        let mut root = Node::new(Role::Window);
        root.set_label(title);
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        root.set_bounds(Rect::new(0., 0., size.width, size.height));
        root.set_transform(Affine::scale(window.scale_factor()));
        root.set_children(children_by_parent.remove(&0).unwrap_or_default());
        next.insert(ROOT, root);
        let mut runs = BTreeMap::new();
        for source in sources {
            let mut source = source.clone();
            if let Some(value) = p.access_text(source.id) {
                source.value = value;
            }
            let id = NodeId(source.id as u64);
            let props = p.host().kernel().node(source.view).map(|n| n.props);
            let password = props.is_some_and(|props| props.str(PropId::Type) == Some("password"));
            let multiline =
                props.is_some_and(|props| props.str(PropId::SemanticTag) == Some("textarea"));
            let readonly = props.is_some_and(|props| props.bool(PropId::Editable) == Some(false));
            let mut node = map_node(&source, origin, password, multiline, readonly);
            node.set_children(children_by_parent.remove(&source.id).unwrap_or_default());
            if matches!(source.role, role::EDIT | role::FIELD) && !password {
                let mut offset = 0;
                let mut lines: Vec<_> = source.value.split_inclusive('\n').collect();
                if source.value.is_empty() || source.value.ends_with('\n') {
                    lines.push("");
                }
                let mut children = Vec::new();
                for (index, line) in lines.into_iter().enumerate() {
                    let run_id = NodeId((id.0 << 32) | (index as u64 + 1));
                    let mut run = Node::new(Role::TextRun);
                    run.set_value(line);
                    let (lengths, offsets) = text_offsets(line, offset);
                    offset = *offsets.last().unwrap();
                    run.set_character_lengths(lengths);
                    runs.insert(run_id, (source.id, offsets));
                    children.push(run_id);
                    next.insert(run_id, run);
                }
                if let Some(selection) = p.access_selection(source.id) {
                    let position = |offset| {
                        let id = children
                            .iter()
                            .rev()
                            .find(|id| runs[id].1[0] <= offset)
                            .unwrap();
                        let offsets = &runs[id].1;
                        accesskit::TextPosition {
                            node: *id,
                            character_index: offsets
                                .partition_point(|n| *n < offset)
                                .min(offsets.len() - 1),
                        }
                    };
                    let (anchor, focus) = if selection.direction == SelectionDirection::Backward {
                        (selection.end, selection.start)
                    } else {
                        (selection.start, selection.end)
                    };
                    node.set_text_selection(accesskit::TextSelection {
                        anchor: position(anchor),
                        focus: position(focus),
                    });
                }
                node.set_children(children);
            }
            next.insert(id, node);
        }
        let changed = next
            .iter()
            .filter(|(id, node)| self.nodes.get(id) != Some(node))
            .map(|(id, node)| (*id, node.clone()))
            .collect::<Vec<_>>();
        if !changed.is_empty() || focus != self.focus {
            self.adapter.update_if_active(|| TreeUpdate {
                nodes: changed,
                tree: Some(TreeInfo::new(ROOT)),
                tree_id: TreeId::ROOT,
                focus,
            });
        }
        self.nodes = next;
        self.runs = runs;
        self.focus = focus;
    }
}

fn text_offsets(text: &str, mut offset: u32) -> (Vec<u8>, Vec<u32>) {
    let mut lengths = Vec::new();
    let mut offsets = vec![offset];
    for grapheme in text.graphemes(true) {
        // AccessKit's byte count is u8; an unbounded combining sequence must be split.
        let pieces: Vec<&str> = if grapheme.len() <= u8::MAX as usize {
            vec![grapheme]
        } else {
            grapheme
                .char_indices()
                .map(|(at, ch)| &grapheme[at..at + ch.len_utf8()])
                .collect()
        };
        for piece in pieces {
            lengths.push(piece.len() as u8);
            offset += piece.encode_utf16().count() as u32;
            offsets.push(offset);
        }
    }
    (lengths, offsets)
}

fn map_node(
    source: &Source,
    origin: (f32, f32),
    password: bool,
    multiline: bool,
    readonly: bool,
) -> Node {
    let role = match source.role {
        role::TEXT => Role::Label,
        role::HEADING => Role::Heading,
        role::BUTTON | role::UP | role::TOGGLE => Role::Button,
        role::LINK => Role::Link,
        role::CHECKBOX => Role::CheckBox,
        role::SWITCH => Role::Switch,
        role::RADIO => Role::RadioButton,
        role::SLIDER => Role::Slider,
        role::IMAGE => Role::Image,
        role::TAB => Role::Tab,
        role::SCROLLER => Role::ScrollView,
        role::PROGRESS => Role::ProgressIndicator,
        role::EDIT | role::FIELD if password => Role::PasswordInput,
        role::EDIT | role::FIELD if multiline => Role::MultilineTextInput,
        role::EDIT | role::FIELD => Role::TextInput,
        role::TABLIST => Role::TabList,
        role::DROPDOWN => Role::ComboBox,
        _ => Role::GenericContainer,
    };
    let mut node = Node::new(role);
    let [x, y, width, height] = source.rect;
    node.set_bounds(Rect::new(
        (x + origin.0).into(),
        (y + origin.1).into(),
        (x + origin.0 + width).into(),
        (y + origin.1 + height).into(),
    ));
    node.set_label(source.name.clone());
    if source.role == role::TEXT {
        node.set_value(source.name.clone());
    } else if !password
        && (matches!(source.role, role::EDIT | role::FIELD) || !source.value.is_empty())
    {
        node.set_value(source.value.clone());
    }
    if !source.hint.is_empty() {
        node.set_description(source.hint.clone());
    }
    let flags = source.flags;
    if flags & flag::DISABLED != 0 {
        node.set_disabled();
    }
    if flags & flag::CHECKABLE != 0 {
        node.set_toggled(if flags & flag::CHECKED != 0 {
            Toggled::True
        } else {
            Toggled::False
        });
    }
    if flags & flag::SELECTED != 0 {
        node.set_selected(true);
    }
    if flags & flag::EXPANDABLE != 0 {
        node.set_expanded(flags & flag::EXPANDED != 0);
    }
    if flags & flag::ASSERTIVE != 0 {
        node.set_live(Live::Assertive);
    } else if flags & flag::POLITE != 0 {
        node.set_live(Live::Polite);
    }
    if source.set.0 > 0 {
        node.set_position_in_set(source.set.0 as usize);
    }
    if source.set.1 > 0 {
        node.set_size_of_set(source.set.1 as usize);
    }
    if flags & flag::CLICK != 0 {
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
    }
    if flags & flag::LONG_CLICK != 0 {
        node.add_action(Action::ShowContextMenu);
    }
    if matches!(source.role, role::EDIT | role::FIELD) {
        if readonly {
            node.set_read_only();
        } else {
            node.add_action(Action::SetValue);
        }
        if !password {
            node.add_action(Action::SetTextSelection);
        }
    }
    if flags & flag::RANGE != 0 {
        node.set_min_numeric_value(source.range[0].into());
        node.set_max_numeric_value(source.range[1].into());
        node.set_numeric_value(source.range[2].into());
        if source.role == role::SLIDER {
            node.add_action(Action::SetValue);
        }
    }
    if source.role == role::SCROLLER {
        node.add_action(Action::Focus);
        let sideways = flags & flag::SIDEWAYS != 0;
        if sideways {
            node.set_scroll_x(source.range[0].into());
            node.set_scroll_x_min(0.);
            node.set_scroll_x_max(source.range[1].into());
        } else {
            node.set_scroll_y(source.range[0].into());
            node.set_scroll_y_min(0.);
            node.set_scroll_y_max(source.range[1].into());
        }
        if flags & flag::FORWARD != 0 {
            node.add_action(if sideways {
                Action::ScrollRight
            } else {
                Action::ScrollDown
            });
        }
        if flags & flag::BACKWARD != 0 {
            node.add_action(if sideways {
                Action::ScrollLeft
            } else {
                Action::ScrollUp
            });
        }
    } else if source.role == role::SLIDER {
        if flags & flag::FORWARD != 0 {
            node.add_action(Action::Increment);
        }
        if flags & flag::BACKWARD != 0 {
            node.add_action(Action::Decrement);
        }
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(role: i32) -> Source {
        Source {
            id: 7,
            parent: 0,
            view: 1,
            role,
            flags: flag::CLICK,
            rect: [10., 20., 100., 40.],
            range: [0.; 3],
            set: (0, 0),
            name: "Message".into(),
            value: "a🦀e\u{301}".into(),
            hint: "Draft".into(),
        }
    }

    #[test]
    fn accessibility_editors_protect_passwords_and_readonly_values() {
        let mut source = source(role::EDIT);
        let password = map_node(&source, (0., 0.), true, false, false);
        assert_eq!(password.role(), Role::PasswordInput);
        assert_eq!(password.value(), None);
        assert!(!password.supports_action(Action::SetTextSelection));
        let readonly = map_node(&source, (0., 0.), false, true, true);
        assert_eq!(readonly.role(), Role::MultilineTextInput);
        assert_eq!(readonly.value(), Some("a🦀e\u{301}"));
        assert!(readonly.is_read_only());
        assert!(!readonly.supports_action(Action::SetValue));
        assert!(readonly.supports_action(Action::SetTextSelection));
        source.value.clear();
        let empty = map_node(&source, (0., 0.), false, true, false);
        assert_eq!(empty.value(), Some(""));
        assert!(empty.supports_action(Action::SetValue));
    }

    #[test]
    fn accessibility_menu_bounds_and_scroll_extents_match_presentation() {
        let mut source = source(role::BUTTON);
        let menu = map_node(&source, (300., 50.), false, false, false);
        assert_eq!(menu.bounds(), Some(Rect::new(310., 70., 410., 110.)));
        source.role = role::SCROLLER;
        source.flags = flag::BACKWARD | flag::SIDEWAYS;
        source.range = [240., 240., 0.];
        let scroller = map_node(&source, (0., 0.), false, false, false);
        assert_eq!(scroller.scroll_x(), Some(240.));
        assert_eq!(scroller.scroll_x_max(), Some(240.));
        assert!(scroller.supports_action(Action::ScrollLeft));
        assert!(!scroller.supports_action(Action::ScrollRight));
        source.role = role::TEXT;
        let text = map_node(&source, (0., 0.), false, false, false);
        assert_eq!(text.value(), Some("Message"));
    }

    #[test]
    fn accessibility_text_offsets_preserve_graphemes_utf16_and_crlf() {
        let (lengths, offsets) = text_offsets("a🦀e\u{301}\r\n", 5);
        assert_eq!(lengths, [1, 4, 3, 2]);
        assert_eq!(offsets, [5, 6, 8, 10, 12]);
        let long = format!("a{}", "\u{301}".repeat(200));
        let (lengths, offsets) = text_offsets(&long, 0);
        assert_eq!(
            lengths.iter().map(|n| *n as usize).sum::<usize>(),
            long.len()
        );
        assert_eq!(offsets.last(), Some(&201));
        assert_eq!(text_offsets("", 5), (vec![], vec![5]));
    }
}
