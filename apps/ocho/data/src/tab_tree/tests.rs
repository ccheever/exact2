//! The tab tree's tests: the GPUI desktop's `tab_tree.rs` tests and the
//! tab-operation tests of `workspace.rs`, plus the crate's own.

use super::*;
use crate::types::{Machine, Session, Snapshot};

#[derive(Debug, PartialEq, Clone)]
struct N(&'static str, usize, bool);

impl TreeNode for N {
    fn depth(&self) -> usize {
        self.1
    }
    fn set_depth(&mut self, depth: usize) {
        self.1 = depth;
    }
    fn collapsed(&self) -> bool {
        self.2
    }
}

fn tree(spec: &[(&'static str, usize)]) -> Vec<N> {
    spec.iter()
        .map(|(name, depth)| N(name, *depth, false))
        .collect()
}

fn shape(nodes: &[N]) -> Vec<(&'static str, usize)> {
    nodes.iter().map(|n| (n.0, n.1)).collect()
}

#[test]
fn subtree_spans_deeper_followers_only() {
    let nodes = tree(&[("a", 0), ("b", 1), ("c", 2), ("d", 1), ("e", 0)]);
    assert_eq!(subtree_end(&nodes, 0), 4);
    assert_eq!(subtree_end(&nodes, 1), 3);
    assert_eq!(subtree_end(&nodes, 3), 4);
    assert_eq!(subtree_end(&nodes, 4), 5);
    assert!(has_children(&nodes, 0));
    assert!(!has_children(&nodes, 2));
    assert_eq!(ancestors(&nodes, 2), vec![1, 0]);
    assert!(ancestors(&nodes, 4).is_empty());
}

#[test]
fn collapsed_ancestor_hides_descendants() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 2), ("d", 0)]);
    nodes[0].2 = true;
    assert!(is_visible(&nodes, 0));
    assert!(!is_visible(&nodes, 1));
    assert!(!is_visible(&nodes, 2));
    assert!(is_visible(&nodes, 3));
}

#[test]
fn dropping_into_a_tab_nests_it_as_the_last_child() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 0), ("d", 0)]);
    let (slot, depth) = placement(&nodes, 0, DropZone::Into);
    assert!(move_subtree(&mut nodes, 3, slot, depth));
    assert_eq!(shape(&nodes), vec![("a", 0), ("b", 1), ("d", 1), ("c", 0)]);
}

#[test]
fn dropping_before_and_after_keeps_the_target_depth() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 2), ("d", 0)]);
    let (slot, depth) = placement(&nodes, 1, DropZone::Before);
    assert!(move_subtree(&mut nodes, 3, slot, depth));
    assert_eq!(shape(&nodes), vec![("a", 0), ("d", 1), ("b", 1), ("c", 2)]);

    // After an expanded parent means "first child", where the line sits.
    let (slot, depth) = placement(&nodes, 2, DropZone::After);
    assert_eq!((slot, depth), (3, 2));

    // After a leaf is a plain sibling.
    let (slot, depth) = placement(&nodes, 3, DropZone::After);
    assert!(move_subtree(&mut nodes, 1, slot, depth));
    assert_eq!(shape(&nodes), vec![("a", 0), ("b", 1), ("c", 2), ("d", 2)]);
}

#[test]
fn after_a_collapsed_parent_skips_its_hidden_children() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 0)]);
    nodes[0].2 = true;
    assert_eq!(placement(&nodes, 0, DropZone::After), (2, 0));
}

#[test]
fn moving_a_parent_carries_its_subtree_and_reindents_it() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 2), ("d", 0), ("e", 0)]);
    let (slot, depth) = placement(&nodes, 4, DropZone::Into);
    assert!(move_subtree(&mut nodes, 0, slot, depth));
    assert_eq!(
        shape(&nodes),
        vec![("d", 0), ("e", 0), ("a", 1), ("b", 2), ("c", 3)]
    );

    let (slot, depth) = placement(&nodes, 0, DropZone::Before);
    assert!(move_subtree(&mut nodes, 2, slot, depth));
    assert_eq!(
        shape(&nodes),
        vec![("a", 0), ("b", 1), ("c", 2), ("d", 0), ("e", 0)]
    );
}

#[test]
fn a_subtree_cannot_be_dropped_inside_itself() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 2), ("d", 0)]);
    let before = nodes.clone();
    // The sidebar refuses targets inside the dragged subtree up front...
    assert!(within(&nodes, 0, 0));
    assert!(within(&nodes, 0, 2));
    assert!(!within(&nodes, 0, 3));
    // ...and the move itself rejects a slot strictly inside it.
    let (slot, depth) = placement(&nodes, 1, DropZone::Before);
    assert!(!move_subtree(&mut nodes, 0, slot, depth));
    assert_eq!(nodes, before);
    // A slot right after the subtree is where it already sits.
    let (slot, depth) = placement(&nodes, 2, DropZone::After);
    assert!(move_subtree(&mut nodes, 0, slot, depth));
    assert_eq!(nodes, before);
}

#[test]
fn moving_to_the_end_at_root_level_flattens_the_subtree_root() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 0)]);
    let end = nodes.len();
    assert!(move_subtree(&mut nodes, 1, end, 0));
    assert_eq!(shape(&nodes), vec![("a", 0), ("c", 0), ("b", 0)]);
    // Same slot, same depth: nothing changes.
    assert!(move_subtree(&mut nodes, 2, 3, 0));
    assert_eq!(shape(&nodes), vec![("a", 0), ("c", 0), ("b", 0)]);
}

#[test]
fn depth_is_clamped_to_one_below_the_previous_row() {
    let mut nodes = tree(&[("a", 0), ("b", 0)]);
    assert!(move_subtree(&mut nodes, 1, 1, 5));
    assert_eq!(shape(&nodes), vec![("a", 0), ("b", 1)]);
    assert!(move_subtree(&mut nodes, 1, 0, 3));
    assert_eq!(shape(&nodes), vec![("b", 0), ("a", 0)]);
}

#[test]
fn closing_a_parent_promotes_its_children() {
    let mut nodes = tree(&[("a", 0), ("b", 1), ("c", 2), ("d", 1), ("e", 0)]);
    let removed = nodes.remove(0);
    promote_children(&mut nodes, 0, removed.depth());
    assert_eq!(shape(&nodes), vec![("b", 0), ("c", 1), ("d", 0), ("e", 0)]);
}

#[test]
fn normalize_repairs_gaps_left_by_dropped_rows() {
    let mut nodes = tree(&[("a", 2), ("b", 3), ("c", 0), ("d", 4)]);
    normalize(&mut nodes);
    assert_eq!(shape(&nodes), vec![("a", 0), ("b", 1), ("c", 0), ("d", 1)]);
}

#[test]
fn edges_of_a_row_read_as_between_rows() {
    assert_eq!(zone_for(101.0, 100.0, 40.0), DropZone::Before);
    assert_eq!(zone_for(120.0, 100.0, 40.0), DropZone::Into);
    assert_eq!(zone_for(139.0, 100.0, 40.0), DropZone::After);
}

#[test]
fn drop_zones_split_a_row_a_quarter_each_edge() {
    // 25% / 50% / 25% of a 40px row at top 100.
    assert_eq!(zone_for(109.9, 100.0, 40.0), DropZone::Before);
    assert_eq!(zone_for(110.0, 100.0, 40.0), DropZone::Into);
    assert_eq!(zone_for(130.0, 100.0, 40.0), DropZone::Into);
    assert_eq!(zone_for(130.1, 100.0, 40.0), DropZone::After);
    // A tiny row still has a 1px edge each side.
    assert_eq!(zone_for(0.5, 0.0, 2.0), DropZone::Before);
    assert_eq!(zone_for(1.5, 0.0, 2.0), DropZone::After);
}

// ----- the workspace's tab operations -----------------------------------

fn terminal(key: &str, depth: usize) -> Tab {
    Tab {
        key: key.into(),
        title: format!("{key} · studio"),
        reconnect: vec!["attach".into(), "studio".into(), key.into()],
        session: Some(("studio".into(), key.into(), false)),
        machine: Some("studio".into()),
        depth,
        ..Default::default()
    }
}

fn folder(key: &str, depth: usize) -> Tab {
    Tab {
        depth,
        ..Tab::folder(format!("folder:{key}"), key)
    }
}

fn keys(tree: &TabTree) -> Vec<(&str, usize)> {
    tree.tabs
        .iter()
        .map(|t| (t.key.as_str(), t.depth))
        .collect()
}

#[test]
fn tab_numbers_skip_folders() {
    // [folder, tab, folder, tab, tab]
    let flags = [true, false, true, false, false];
    assert_eq!(numbered_slot(&flags, 0), Some(0));
    assert_eq!(numbered_slot(&flags, 1), Some(2));
    assert_eq!(numbered_slot(&flags, 2), Some(4));
    assert_eq!(numbered_slot(&flags, 3), Some(5));
    assert_eq!(numbered_slot(&flags, 4), None);

    let tree = TabTree {
        tabs: vec![
            folder("f", 0),
            terminal("a", 1),
            folder("g", 0),
            terminal("b", 1),
            terminal("c", 0),
        ],
        active: 0,
    };
    assert_eq!(tree.tab_number(1), 1);
    assert_eq!(tree.tab_number(3), 2);
    assert_eq!(tree.tab_number(4), 3);
    // A folder's "number" is the count so far; the rail never shows it.
    assert_eq!(tree.tab_number(2), 1);
    assert_eq!(tree.numbered_slot(2), Some(4));
    assert_eq!(tree.numbered_slot(4), None);
}

#[test]
fn stepping_through_tabs_skips_folders() {
    let flags = [true, false, true, false, false];
    assert_eq!(stepped_slot(&flags, 0, true), 2);
    assert_eq!(stepped_slot(&flags, 2, true), 4);
    assert_eq!(stepped_slot(&flags, 5, true), 0);
    assert_eq!(stepped_slot(&flags, 0, false), 5);
    assert_eq!(stepped_slot(&flags, 4, false), 2);
    assert_eq!(stepped_slot(&flags, 2, false), 0);
    // Only folders: the manager is the only stop.
    assert_eq!(stepped_slot(&[true, true], 0, true), 0);

    let mut tree = TabTree {
        tabs: vec![folder("f", 0), terminal("a", 1), terminal("b", 1)],
        active: 0,
    };
    tree.tabs[0].collapsed = true;
    assert_eq!(tree.next_slot(), 2);
    tree.step(true);
    assert_eq!(tree.active, 2);
    // Stepping onto a hidden tab reveals it.
    assert!(!tree.tabs[0].collapsed);
    tree.step(false);
    assert_eq!(tree.active, 0);
    assert_eq!(tree.prev_slot(), 3);
}

#[test]
fn closing_and_restoring_tabs_preserves_the_expected_selection() {
    // Closing a background tab before the selected tab keeps the same tab active.
    assert_eq!(active_after_tab_close(2, 0, 2), 1);
    // Closing the selected tab activates its right neighbor, or its left at the end.
    assert_eq!(active_after_tab_close(2, 1, 2), 2);
    assert_eq!(active_after_tab_close(3, 2, 2), 2);
    assert_eq!(active_after_tab_close(1, 0, 0), 0);
    assert_eq!(active_after_tab_close(0, 0, 2), 0);

    // Undo selects a tab that was active when closed. A background restore
    // instead adjusts the index so the currently active tab keeps its identity.
    assert_eq!(active_after_tab_restore(1, 0, true), 1);
    assert_eq!(active_after_tab_restore(1, 0, false), 2);
    assert_eq!(active_after_tab_restore(0, 0, false), 0);
}

#[test]
fn closing_a_tab_lifts_its_children_and_undo_puts_them_back() {
    let mut tree = TabTree {
        tabs: vec![
            terminal("a", 0),
            terminal("b", 1),
            terminal("c", 2),
            terminal("d", 0),
        ],
        active: 3,
    };
    let closed = tree.close(0).unwrap();
    assert_eq!(keys(&tree), vec![("b", 0), ("c", 1), ("d", 0)]);
    assert_eq!(tree.active, 2);
    assert_eq!(closed.folder, None);
    assert!(!closed.activate_on_restore);
    assert_eq!(
        TabTree::closed_message(&closed),
        "Closed a · studio · ⌘Z to undo"
    );
    assert_eq!(tree.reopen(closed), Reopened::Tab(0));
    assert_eq!(keys(&tree), vec![("a", 0), ("b", 0), ("c", 1), ("d", 0)]);
    assert_eq!(tree.active, 3);

    // Closing the active tab selects its right neighbor.
    tree.active = 1;
    let closed = tree.close(0).unwrap();
    assert!(closed.activate_on_restore);
    assert_eq!(tree.active, 1);
    assert_eq!(tree.active_tab().unwrap().key, "b");
    assert_eq!(tree.reopen(closed), Reopened::Tab(0));
    assert_eq!(tree.active, 1);
    assert_eq!(tree.active_tab().unwrap().key, "a");

    // A key that is open already is selected, not duplicated.
    let again = ClosedTab {
        tab: terminal("d", 0),
        position: 9,
        folder: None,
        activate_on_restore: false,
    };
    assert_eq!(tree.reopen(again), Reopened::AlreadyOpen(3));
    assert_eq!(tree.active, 4);
    assert!(tree.close(9).is_none());
}

#[test]
fn removing_a_folder_promotes_its_rows_and_undo_tucks_them_back() {
    let mut tree = TabTree {
        tabs: vec![
            folder("f", 0),
            terminal("a", 1),
            terminal("b", 1),
            terminal("c", 0),
        ],
        active: 3,
    };
    let closed = tree.close(0).unwrap();
    assert_eq!(closed.folder, Some(2));
    assert_eq!(
        TabTree::closed_message(&closed),
        "Removed folder f · ⌘Z to undo"
    );
    assert_eq!(keys(&tree), vec![("a", 0), ("b", 0), ("c", 0)]);
    assert_eq!(tree.active_tab().unwrap().key, "b");
    assert_eq!(tree.reopen(closed), Reopened::Folder(0));
    assert_eq!(
        keys(&tree),
        vec![("folder:f", 0), ("a", 1), ("b", 1), ("c", 0)]
    );
    assert_eq!(tree.active_tab().unwrap().key, "b");
}

#[test]
fn collapsing_an_ancestor_of_the_active_tab_moves_the_selection() {
    let mut tree = TabTree {
        tabs: vec![
            terminal("a", 0),
            terminal("b", 1),
            terminal("c", 2),
            terminal("d", 0),
        ],
        active: 3,
    };
    // A leaf has nothing to collapse.
    assert!(!tree.toggle_collapsed(3));
    assert!(tree.toggle_collapsed(0));
    assert!(tree.tabs[0].collapsed);
    // The active tab was inside: the parent is selected.
    assert_eq!(tree.active, 1);
    assert_eq!(tree.visible(), vec![0, 3]);
    assert!(tree.toggle_collapsed(0));
    assert!(!tree.tabs[0].collapsed);
    // Collapsing the active tab itself keeps it.
    tree.active = 2;
    assert!(tree.toggle_collapsed(1));
    assert_eq!(tree.active, 2);
    assert!(tree.is_ancestor_of_active(0));
    assert!(!tree.is_ancestor_of_active(3));

    // A folder cannot be selected: the nearest terminal after it is.
    let mut tree = TabTree {
        tabs: vec![
            terminal("x", 0),
            folder("f", 0),
            terminal("a", 1),
            terminal("b", 0),
        ],
        active: 3,
    };
    assert!(tree.toggle_collapsed(1));
    assert_eq!(tree.active, 4);
    // With nothing after, the last terminal before it.
    let mut tree = TabTree {
        tabs: vec![terminal("x", 0), folder("f", 0), terminal("a", 1)],
        active: 3,
    };
    assert!(tree.toggle_collapsed(1));
    assert_eq!(tree.active, 1);
    // Selecting a folder settles the same way.
    tree.select(1);
    assert_eq!(tree.active, 1);
    assert_eq!(tree.nearest_terminal(1), 1);
    let mut only_folders = TabTree {
        tabs: vec![folder("f", 0)],
        active: 1,
    };
    only_folders.settle_active();
    assert_eq!(only_folders.active, 0);
}

#[test]
fn dropping_moves_subtrees_and_the_tail_flattens_them() {
    let mut tree = TabTree {
        tabs: vec![
            terminal("a", 0),
            terminal("b", 1),
            terminal("c", 0),
            terminal("d", 0),
        ],
        active: 2,
    };
    tree.tabs[2].collapsed = true;
    // Into: nested as the last child; the target expands.
    assert!(tree.move_tab(
        3,
        TabDrop {
            target: 2,
            zone: DropZone::Into
        }
    ));
    assert_eq!(keys(&tree), vec![("a", 0), ("b", 1), ("c", 0), ("d", 1)]);
    assert!(!tree.tabs[2].collapsed);
    // The active tab keeps its identity across a move.
    assert!(tree.move_tab(
        2,
        TabDrop {
            target: 0,
            zone: DropZone::Before
        }
    ));
    assert_eq!(keys(&tree), vec![("c", 0), ("d", 1), ("a", 0), ("b", 1)]);
    assert_eq!(tree.active_tab().unwrap().key, "b");
    assert_eq!(tree.active, 4);
    // Into its own subtree is refused.
    assert!(!tree.can_drop(0, 1));
    assert!(!tree.move_tab(
        0,
        TabDrop {
            target: 1,
            zone: DropZone::After
        }
    ));
    assert!(tree.can_drop(0, 2));
    // The tail: end of the top level, flattened.
    assert!(tree.move_to_end(1));
    assert_eq!(keys(&tree), vec![("c", 0), ("a", 0), ("b", 1), ("d", 0)]);
    assert!(!tree.move_tab(
        9,
        TabDrop {
            target: 0,
            zone: DropZone::Before
        }
    ));
}

#[test]
fn folders_need_a_name() {
    let mut tree = TabTree::new();
    assert_eq!(tree.new_folder("folder:1", "  "), None);
    assert_eq!(tree.new_folder("folder:1", " Work "), Some(0));
    assert_eq!(tree.tabs[0].title, "Work");
    assert!(tree.tabs[0].is_folder());
    assert!(!tree.rename_folder("folder:1", ""));
    assert!(tree.rename_folder("folder:1", "Play"));
    assert_eq!(tree.tabs[0].title, "Play");
    assert!(!tree.rename_folder("folder:2", "x"));
}

#[test]
fn saved_windows_round_trip_through_tabs() {
    let mut tree = TabTree {
        tabs: vec![
            folder("f", 0),
            terminal("a", 1),
            Tab {
                key: "login:claude".into(),
                reconnect: vec!["login".into()],
                depth: 1,
                ..Default::default()
            },
            terminal("b", 2),
            terminal("c", 0),
        ],
        active: 4,
    };
    tree.tabs[0].collapsed = true;
    let saved = tree.saved(312.0);
    // The login tab is left out and the tree repaired around it.
    let keys: Vec<(&str, usize)> = saved
        .tabs
        .iter()
        .map(|t| (t.key.as_str(), t.depth))
        .collect();
    assert_eq!(keys, vec![("folder:f", 0), ("a", 1), ("b", 2), ("c", 0)]);
    assert!(saved.tabs[0].folder && saved.tabs[0].collapsed);
    assert_eq!(saved.active, 3);
    assert_eq!(saved.rail_width, Some(312.0));

    let restored = TabTree::restore(&saved);
    assert_eq!(restored.tabs.len(), 4);
    assert_eq!(restored.active, 3);
    assert_eq!(restored.tabs[2], tree.tabs[3]);
    assert_eq!(restored.tabs[0].title, "f");
    // The selection is revealed.
    assert!(!restored.tabs[0].collapsed);
    for (tab, saved) in restored.tabs.iter().zip(&saved.tabs) {
        let mut again = tab.to_saved();
        again.collapsed = saved.collapsed;
        assert_eq!(&again, saved);
    }

    // A saved tab without a reconnect argv is skipped; the selection follows.
    let mut saved = saved;
    saved.tabs[1].reconnect.clear();
    saved.active = 4;
    let restored = TabTree::restore(&saved);
    assert_eq!(
        restored
            .tabs
            .iter()
            .map(|t| t.key.as_str())
            .collect::<Vec<_>>(),
        ["folder:f", "b", "c"]
    );
    assert_eq!(restored.tabs[1].depth, 1);
    assert_eq!(restored.active, 3);
    // A selected folder settles on the first terminal after its subtree.
    saved.active = 1;
    assert_eq!(TabTree::restore(&saved).active, 3);
    // One that fails to open afterwards is dropped the same way.
    let mut restored = restored;
    restored.drop_unrestored(1);
    assert_eq!(
        restored
            .tabs
            .iter()
            .map(|t| t.key.as_str())
            .collect::<Vec<_>>(),
        ["folder:f", "c"]
    );
    assert_eq!(restored.active, 2);
}

#[test]
fn saved_tabs_reopen_with_the_machine_from_their_session() {
    let saved = SavedTab {
        key: "studio:s:false".into(),
        title: "t".into(),
        reconnect: vec![
            "run".into(),
            "studio".into(),
            "--request-id".into(),
            "s".into(),
            "--attach".into(),
        ],
        session: Some(("studio".into(), "s".into(), false)),
        machine: None,
        depth: 1,
        collapsed: true,
        folder: false,
    };
    let tab = Tab::from_saved(&saved);
    assert_eq!(tab.machine.as_deref(), Some("studio"));
    assert_eq!(tab.reconnect[1], "--restore");
    assert!(tab.collapsed);
    assert!(!tab.is_folder());
    let folder = Tab::from_saved(&SavedTab {
        key: "folder:1".into(),
        title: "F".into(),
        folder: true,
        depth: 0,
        ..Default::default()
    });
    assert!(folder.is_folder());
    assert!(folder.to_saved().folder);
}

#[test]
fn restoring_launch_commands_resolves_the_recorded_request() {
    let command: Vec<String> = [
        "run",
        "studio",
        "--request-id",
        "original",
        "--prompt",
        "--restore",
        "--attach",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let restored = restore_terminal_command(&command);
    assert_eq!(restored[1], "--restore");
    assert_eq!(&restored[2..], &command[1..]);
    assert_eq!(restore_terminal_command(&restored), restored);
    for verb in ["attach", "connect", "migrate", "handoff", "accounts"] {
        let command = vec![verb.into(), "studio".into()];
        assert_eq!(restore_terminal_command(&command), command);
    }
    let without_identity = vec!["run".into(), "studio".into(), "--attach".into()];
    assert_eq!(
        restore_terminal_command(&without_identity),
        without_identity
    );
    let without_attach = ["run", "studio", "--request-id", "original"].map(String::from);
    assert_eq!(restore_terminal_command(&without_attach), without_attach);
}

#[test]
fn established_tabs_persist_direct_attachment_without_regressing_on_a_blip() {
    let mut tab = Tab::folder("k", "Agent · studio");
    tab.folder = false;
    tab.key = "studio:original:true".into();
    tab.session = Some(("studio".into(), "original".into(), true));
    tab.machine = Some("studio".into());
    tab.reconnect = restore_terminal_command(
        &["run", "studio", "--request-id", "original", "--attach"].map(String::from),
    );
    let pending = tab.reconnect.clone();
    let mut tabs = vec![tab];
    let mut state = State::default();
    state.machines.push(Machine {
        id: "studio".into(),
        last: Some(Snapshot {
            sessions: vec![Session {
                id: "original".into(),
                state: "starting".into(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(!promote_established_tabs(&mut tabs, &state));
    assert_eq!(tabs[0].reconnect, pending);
    // Even a launch still starting has a durable receipt once its pane exists.
    state.machines[0].last.as_mut().unwrap().sessions[0].tmux_pane = "%1".into();
    state.machines[0].id = "other-machine".into();
    assert!(!promote_established_tabs(&mut tabs, &state));
    state.machines[0].id = "studio".into();
    assert!(promote_established_tabs(&mut tabs, &state));
    let attach = session_attach_command("studio", "original", true);
    assert_eq!(tabs[0].reconnect, attach);
    assert_eq!(attach, ["attach", "studio", "original", "--read-only"]);
    state.machines[0].last = None;
    assert!(!promote_established_tabs(&mut tabs, &state));
    assert_eq!(tabs[0].reconnect, attach);
    let saved = SavedTab {
        key: tabs[0].key.clone(),
        reconnect: tabs[0].reconnect.clone(),
        session: tabs[0].session.clone(),
        ..Default::default()
    };
    let reopened: SavedTab = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert_eq!(restore_terminal_command(&reopened.reconnect), attach);
    assert_eq!(reopened.session, saved.session);
    assert_eq!(
        session_attach_command("local", "original", false),
        ["attach", "local", "original"]
    );
    let mut tree = TabTree { tabs, active: 1 };
    assert!(!tree.promote_established(&state));
}

#[test]
fn successful_setup_terminals_refresh_saved_state_once() {
    assert!(terminal_refreshes_saved_state("login:claude"));
    assert!(terminal_refreshes_saved_state("add:studio"));
    assert!(!terminal_refreshes_saved_state("shell:studio"));
    assert!(!terminal_refreshes_saved_state("studio:session:false"));
    assert!(!restorable("login:claude"));
    assert!(restorable("folder:1"));
}
