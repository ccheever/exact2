//! Nesting for the rail's terminal tabs: a port of the GPUI desktop's
//! `tab_tree.rs` plus the tab operations of `workspace.rs` (close, move,
//! collapse, numbering, restore and persist) over a [`Tab`] that is the
//! model's `TerminalTab` without its terminal entity.
//!
//! Tabs stay in one flat, pre-order list; each carries a `depth`. A tab's
//! subtree is the run of following tabs whose depth is greater than its own,
//! so the invariant is simply `depth[i] <= depth[i - 1] + 1` (and `depth[0]
//! == 0`). This keeps tab numbers, ⌘digit shortcuts, and the active index
//! working unchanged while the sidebar draws the list as a tree.

use crate::settings::{SavedTab, SavedWindow};
use crate::types::State;

/// What the tree needs of a row.
pub trait TreeNode {
    /// Nesting level; 0 is top level.
    fn depth(&self) -> usize;
    /// Change the nesting level.
    fn set_depth(&mut self, depth: usize);
    /// Whether the row's children are hidden.
    fn collapsed(&self) -> bool;
}

/// Where a dragged tab lands relative to the row under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropZone {
    /// A sibling immediately above the target.
    Before,
    /// The target's last child.
    Into,
    /// A sibling right after the target's subtree; when the target's children
    /// are showing, its first child instead, which is where the line is drawn.
    After,
}

/// The fraction of a row's height at each edge that reads as "between rows"
/// rather than "onto this row".
const EDGE_FRACTION: f64 = 0.25;

/// Picks the drop zone for a pointer `y` inside a row spanning `top..top + height`.
pub fn zone_for(y: f64, top: f64, height: f64) -> DropZone {
    let edge = (height * EDGE_FRACTION).max(1.0);
    if y < top + edge {
        DropZone::Before
    } else if y > top + height - edge {
        DropZone::After
    } else {
        DropZone::Into
    }
}

/// Exclusive end of the subtree rooted at `index`.
pub fn subtree_end<T: TreeNode>(nodes: &[T], index: usize) -> usize {
    let depth = nodes[index].depth();
    let mut end = index + 1;
    while end < nodes.len() && nodes[end].depth() > depth {
        end += 1;
    }
    end
}

/// Whether the row after `index` is nested under it.
pub fn has_children<T: TreeNode>(nodes: &[T], index: usize) -> bool {
    nodes
        .get(index + 1)
        .is_some_and(|next| next.depth() > nodes[index].depth())
}

/// Indices of `index`'s ancestors, nearest first.
pub fn ancestors<T: TreeNode>(nodes: &[T], index: usize) -> Vec<usize> {
    let mut found = Vec::new();
    let mut depth = nodes[index].depth();
    for i in (0..index).rev() {
        if depth == 0 {
            break;
        }
        if nodes[i].depth() < depth {
            found.push(i);
            depth = nodes[i].depth();
        }
    }
    found
}

/// Whether no ancestor of `index` is collapsed.
pub fn is_visible<T: TreeNode>(nodes: &[T], index: usize) -> bool {
    ancestors(nodes, index)
        .into_iter()
        .all(|i| !nodes[i].collapsed())
}

/// Whether `index` lies inside the subtree rooted at `root` (the root included).
pub fn within<T: TreeNode>(nodes: &[T], root: usize, index: usize) -> bool {
    root <= index && index < subtree_end(nodes, root)
}

/// The insertion slot (an index into the list before anything moves) and the
/// depth a dropped subtree takes for `zone` on `target`.
pub fn placement<T: TreeNode>(nodes: &[T], target: usize, zone: DropZone) -> (usize, usize) {
    let depth = nodes[target].depth();
    match zone {
        DropZone::Before => (target, depth),
        DropZone::Into => (subtree_end(nodes, target), depth + 1),
        DropZone::After if has_children(nodes, target) && !nodes[target].collapsed() => {
            (target + 1, depth + 1)
        }
        DropZone::After => (subtree_end(nodes, target), depth),
    }
}

/// Moves the subtree rooted at `from` so it starts at `insert_at` (an index
/// into the list as it is now) with its root at `depth`. Returns false and
/// leaves the list alone when the slot is inside the moving subtree. Depth is
/// clamped so the tree invariant always holds.
pub fn move_subtree<T: TreeNode>(
    nodes: &mut Vec<T>,
    from: usize,
    insert_at: usize,
    depth: usize,
) -> bool {
    if from >= nodes.len() || insert_at > nodes.len() {
        return false;
    }
    let end = subtree_end(nodes, from);
    if insert_at > from && insert_at < end {
        return false;
    }
    let moving: Vec<T> = nodes.drain(from..end).collect();
    let slot = if insert_at >= end {
        insert_at - moving.len()
    } else {
        insert_at
    };
    let max_depth = slot.checked_sub(1).map_or(0, |i| nodes[i].depth() + 1);
    let depth = depth.min(max_depth);
    let old_depth = moving[0].depth();
    let tail = nodes.split_off(slot);
    for mut node in moving {
        let d = node.depth() - old_depth + depth;
        node.set_depth(d);
        nodes.push(node);
    }
    nodes.extend(tail);
    normalize(nodes);
    true
}

/// After the node at `removed` was taken out of the list, lifts the children
/// it left behind one level so they become siblings of their former parent.
pub fn promote_children<T: TreeNode>(nodes: &mut [T], removed: usize, removed_depth: usize) {
    let mut i = removed;
    while i < nodes.len() && nodes[i].depth() > removed_depth {
        let d = nodes[i].depth() - 1;
        nodes[i].set_depth(d);
        i += 1;
    }
}

/// Clamps every depth so no tab sits more than one level below the tab above it.
pub fn normalize<T: TreeNode>(nodes: &mut [T]) {
    let mut previous = 0;
    for (i, node) in nodes.iter_mut().enumerate() {
        let max = if i == 0 { 0 } else { previous + 1 };
        if node.depth() > max {
            node.set_depth(max);
        }
        previous = node.depth();
    }
}

/// A row in the sidebar's TERMINALS tree: a terminal, or a folder that only
/// groups the rows nested under it (workspace.rs `TerminalTab` without the
/// terminal entity, transcript, panel and recovery state).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tab {
    /// The row's identity; see [`SavedTab::key`].
    pub key: String,
    /// The row's title; `"{label} · {machine}"` for session tabs.
    pub title: String,
    /// The `fleet` argv that (re)opens the terminal; empty for a folder.
    pub reconnect: Vec<String>,
    /// (machine id, session id, read-only) for tabs that follow a session's label.
    pub session: Option<(String, String, bool)>,
    /// Machine the terminal runs on, when known; pasted images are sent there.
    pub machine: Option<String>,
    /// Nesting level in the sidebar tree.
    pub depth: usize,
    /// Whether this tab's children are hidden in the sidebar.
    pub collapsed: bool,
    /// A sidebar folder: no terminal, only a title and nested rows.
    pub folder: bool,
}

impl Tab {
    /// A folder at the top level.
    pub fn folder(key: impl Into<String>, title: impl Into<String>) -> Tab {
        Tab {
            key: key.into(),
            title: title.into(),
            folder: true,
            ..Default::default()
        }
    }

    /// Whether the row is a folder (a terminal tab it is not).
    pub fn is_folder(&self) -> bool {
        self.folder
    }

    /// The row as `desktop.json` keeps it.
    pub fn to_saved(&self) -> SavedTab {
        SavedTab {
            key: self.key.clone(),
            title: self.title.clone(),
            reconnect: self.reconnect.clone(),
            session: self.session.clone(),
            machine: self.machine.clone(),
            depth: self.depth,
            collapsed: self.collapsed,
            folder: self.folder,
        }
    }

    /// A saved row as it reopens: the reconnect argv upgraded to a restoring
    /// launch, the machine taken from the session when it was not saved.
    pub fn from_saved(saved: &SavedTab) -> Tab {
        if saved.folder {
            return Tab {
                depth: saved.depth,
                collapsed: saved.collapsed,
                ..Tab::folder(saved.key.clone(), saved.title.clone())
            };
        }
        Tab {
            key: saved.key.clone(),
            title: saved.title.clone(),
            reconnect: restore_terminal_command(&saved.reconnect),
            session: saved.session.clone(),
            machine: saved
                .machine
                .clone()
                .or_else(|| saved.session.as_ref().map(|s| s.0.clone())),
            depth: saved.depth,
            collapsed: saved.collapsed,
            folder: false,
        }
    }
}

impl TreeNode for Tab {
    fn depth(&self) -> usize {
        self.depth
    }
    fn set_depth(&mut self, depth: usize) {
        self.depth = depth;
    }
    fn collapsed(&self) -> bool {
        self.collapsed
    }
}

impl TreeNode for SavedTab {
    fn depth(&self) -> usize {
        self.depth
    }
    fn set_depth(&mut self, depth: usize) {
        self.depth = depth;
    }
    fn collapsed(&self) -> bool {
        self.collapsed
    }
}

/// The slot a dragged sidebar tab would land in if released now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabDrop {
    /// The row under the pointer.
    pub target: usize,
    /// Where on that row.
    pub zone: DropZone,
}

/// A closed tab on the undo stack.
#[derive(Clone, Debug, PartialEq)]
pub struct ClosedTab {
    /// The row as it was.
    pub tab: Tab,
    /// Where it sat.
    pub position: usize,
    /// Rows that were nested under a closed folder and got promoted; undo
    /// tucks that many rows back under it. None for a terminal.
    pub folder: Option<usize>,
    /// It was the active tab when closed, so undo selects it again.
    pub activate_on_restore: bool,
}

/// What reopening a closed tab did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reopened {
    /// A tab with that key is open already, at this position; it was selected.
    AlreadyOpen(usize),
    /// The folder is back at this position, its rows tucked under it.
    Folder(usize),
    /// The tab is back at this position; the app spawns its terminal.
    Tab(usize),
}

/// The rail's TERMINALS tree and its selection: `active` is 0 for the
/// manager and `i + 1` for `tabs[i]`, as in the desktop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabTree {
    /// The rows, pre-order.
    pub tabs: Vec<Tab>,
    /// 0 for the manager, `i + 1` for `tabs[i]`.
    pub active: usize,
}

impl TabTree {
    /// No tabs, the manager selected.
    pub fn new() -> TabTree {
        TabTree::default()
    }

    /// The rows a saved window reopens (workspace.rs `restore_tabs`): folders
    /// as they were, terminals with a reconnect argv that are not one-shot
    /// sign-in or enrollment tabs. `skipped` are the keys left out (the app
    /// reports a tab whose terminal fails to spawn the same way).
    pub fn restore(saved: &SavedWindow) -> TabTree {
        let mut tree = TabTree::new();
        for (index, tab) in saved.tabs.iter().enumerate() {
            if !tab.folder && (tab.reconnect.is_empty() || !restorable(&tab.key)) {
                continue;
            }
            tree.tabs.push(Tab::from_saved(tab));
            if index + 1 == saved.active {
                tree.active = tree.tabs.len();
            }
        }
        // Tabs that were left out may have left gaps in the tree.
        normalize(&mut tree.tabs);
        tree.settle_active();
        tree
    }

    /// Drop the tab at `pos` after its terminal failed to open, keeping the
    /// tree well formed and the selection on a terminal.
    pub fn drop_unrestored(&mut self, pos: usize) {
        if pos >= self.tabs.len() {
            return;
        }
        let depth = self.tabs[pos].depth;
        self.tabs.remove(pos);
        promote_children(&mut self.tabs, pos, depth);
        self.active = active_after_tab_close(self.active, pos, self.tabs.len());
        normalize(&mut self.tabs);
        self.settle_active();
    }

    /// The window as `desktop.json` keeps it (workspace.rs `saved_window`).
    pub fn saved(&self, rail_width: f64) -> SavedWindow {
        let mut tabs: Vec<SavedTab> = self
            .tabs
            .iter()
            .filter(|t| restorable(&t.key))
            .map(Tab::to_saved)
            .collect();
        // Omitted tabs may have been parents; keep the saved tree well formed.
        normalize(&mut tabs);
        // Ephemeral login/setup tabs are not restored. Keep the selection
        // attached to the same saved tab even when earlier tabs are omitted.
        let active = self
            .active
            .checked_sub(1)
            .and_then(|index| self.tabs.get(index))
            .and_then(|tab| tabs.iter().position(|saved| saved.key == tab.key))
            .map_or(0, |index| index + 1);
        SavedWindow {
            tabs,
            active,
            rail_width: Some(rail_width),
        }
    }

    /// The index of the active tab, None on the manager.
    pub fn active_index(&self) -> Option<usize> {
        self.active.checked_sub(1).filter(|i| *i < self.tabs.len())
    }

    /// The active tab.
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_index().map(|i| &self.tabs[i])
    }

    /// The active tab, mutable.
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let i = self.active_index()?;
        Some(&mut self.tabs[i])
    }

    /// Where the tab with `key` sits.
    pub fn position(&self, key: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.key == key)
    }

    /// Select the tab at `pos` (a folder cannot be selected; the selection
    /// moves to its nearest terminal).
    pub fn select(&mut self, pos: usize) {
        if pos < self.tabs.len() {
            self.active = pos + 1;
            self.settle_active();
        }
    }

    /// Select the manager.
    pub fn select_manager(&mut self) {
        self.active = 0;
    }

    /// Whether the row at `index` is on screen (no collapsed ancestor).
    pub fn is_visible(&self, index: usize) -> bool {
        is_visible(&self.tabs, index)
    }

    /// The rows the rail draws, in order.
    pub fn visible(&self) -> Vec<usize> {
        (0..self.tabs.len())
            .filter(|i| self.is_visible(*i))
            .collect()
    }

    /// Whether the row after `index` is nested under it.
    pub fn has_children(&self, index: usize) -> bool {
        has_children(&self.tabs, index)
    }

    /// Whether `index` is an ancestor of the active tab.
    pub fn is_ancestor_of_active(&self, index: usize) -> bool {
        self.active_index()
            .is_some_and(|active| ancestors(&self.tabs, active).contains(&index))
    }

    /// The 1-based number shown beside the tab at `pos`: terminals are
    /// numbered in sidebar order, folders are not.
    pub fn tab_number(&self, pos: usize) -> usize {
        self.tabs[..=pos].iter().filter(|t| !t.is_folder()).count()
    }

    fn folder_flags(&self) -> Vec<bool> {
        self.tabs.iter().map(Tab::is_folder).collect()
    }

    /// The `active` value for tab number `n` (⌘digit; 1-based, counting
    /// terminals only), or 0 for the manager; None when there is no such tab.
    pub fn numbered_slot(&self, n: usize) -> Option<usize> {
        numbered_slot(&self.folder_flags(), n)
    }

    /// Steps `active` forward or backward through the manager (0) and the
    /// terminals, skipping folders (⌘⇧] / ⌘⇧[).
    pub fn step(&mut self, forward: bool) {
        self.active = stepped_slot(&self.folder_flags(), self.active, forward);
        self.reveal_active();
    }

    /// Close the tab at `pos`; its children move up a level. What the undo
    /// stack keeps comes back, None when there is no such tab.
    pub fn close(&mut self, pos: usize) -> Option<ClosedTab> {
        if pos >= self.tabs.len() {
            return None;
        }
        let activate_on_restore = self.active == pos + 1;
        let descendants = subtree_end(&self.tabs, pos) - pos - 1;
        let tab = self.tabs.remove(pos);
        let folder = tab.is_folder().then_some(descendants);
        promote_children(&mut self.tabs, pos, tab.depth);
        self.active = active_after_tab_close(self.active, pos, self.tabs.len());
        self.settle_active();
        Some(ClosedTab {
            tab,
            position: pos,
            folder,
            activate_on_restore,
        })
    }

    /// The toast for a close (workspace.rs `close_tab`).
    pub fn closed_message(closed: &ClosedTab) -> String {
        if closed.folder.is_some() {
            format!("Removed folder {} · ⌘Z to undo", closed.tab.title)
        } else {
            format!("Closed {} · ⌘Z to undo", closed.tab.title)
        }
    }

    /// Put a closed tab back (⌘Z; workspace.rs `restore_closed_tab`). A
    /// reopened terminal starts with its children shown.
    pub fn reopen(&mut self, closed: ClosedTab) -> Reopened {
        if let Some(pos) = self.position(&closed.tab.key) {
            self.active = pos + 1;
            return Reopened::AlreadyOpen(pos);
        }
        let position = closed.position.min(self.tabs.len());
        if let Some(descendants) = closed.folder {
            self.tabs.insert(
                position,
                Tab {
                    depth: closed.tab.depth,
                    ..closed.tab
                },
            );
            // Tuck the rows it used to hold back under it, if they are still there.
            for tab in self.tabs.iter_mut().skip(position + 1).take(descendants) {
                tab.depth += 1;
            }
            normalize(&mut self.tabs);
            if let Some(index) = self.active.checked_sub(1) {
                if index >= position {
                    self.active += 1;
                }
            }
            return Reopened::Folder(position);
        }
        let active = active_after_tab_restore(self.active, position, closed.activate_on_restore);
        self.tabs.insert(
            position,
            Tab {
                collapsed: false,
                ..closed.tab
            },
        );
        normalize(&mut self.tabs);
        self.active = active;
        self.reveal_active();
        Reopened::Tab(position)
    }

    /// Drops the tab (with its children) at `from` onto the sidebar row
    /// `drop.target`: above it, below it, or nested inside it. A tab cannot
    /// be dropped into its own subtree.
    pub fn move_tab(&mut self, from: usize, drop: TabDrop) -> bool {
        let TabDrop { target, zone } = drop;
        if from >= self.tabs.len() || target >= self.tabs.len() {
            return false;
        }
        if within(&self.tabs, from, target) {
            return false;
        }
        let (slot, depth) = placement(&self.tabs, target, zone);
        let expand = match zone {
            DropZone::Into => Some(self.tabs[target].key.clone()),
            _ => None,
        };
        let moved = self.place(from, slot, depth);
        if let Some(key) = expand {
            if let Some(parent) = self.tabs.iter_mut().find(|t| t.key == key) {
                parent.collapsed = false;
            }
        }
        moved
    }

    /// Drops the tab (with its children) at `from` after every other tab, at
    /// the top level (the rail's drop tail).
    pub fn move_to_end(&mut self, from: usize) -> bool {
        let end = self.tabs.len();
        self.place(from, end, 0)
    }

    fn place(&mut self, from: usize, slot: usize, depth: usize) -> bool {
        let active_key = self.active_tab().map(|tab| tab.key.clone());
        if !move_subtree(&mut self.tabs, from, slot, depth) {
            return false;
        }
        if let Some(key) = active_key {
            self.active = self.position(&key).map_or(0, |i| i + 1);
        }
        true
    }

    /// Whether the drop `zone` on `target` is allowed for the tab at `from`:
    /// not inside its own subtree.
    pub fn can_drop(&self, from: usize, target: usize) -> bool {
        from < self.tabs.len() && target < self.tabs.len() && !within(&self.tabs, from, target)
    }

    /// Hides or shows the children of the tab at `pos` in the sidebar.
    /// Collapsing the parent of the active tab selects the parent (or, for a
    /// folder, the nearest terminal), since the active tab must stay on
    /// screen. Returns whether anything changed; a selection that moved to
    /// the manager should also leave NAV (`nav &= active > 0`).
    pub fn toggle_collapsed(&mut self, pos: usize) -> bool {
        if pos >= self.tabs.len() {
            return false;
        }
        if !self.tabs[pos].is_folder() && !has_children(&self.tabs, pos) {
            return false;
        }
        let collapsed = !self.tabs[pos].collapsed;
        self.tabs[pos].collapsed = collapsed;
        if collapsed {
            if let Some(active) = self.active.checked_sub(1) {
                if active != pos && within(&self.tabs, pos, active) {
                    self.active = if self.tabs[pos].is_folder() {
                        self.nearest_terminal(pos)
                    } else {
                        pos + 1
                    };
                }
            }
        }
        self.reveal_active();
        true
    }

    /// Adds an empty folder to the end of the sidebar's top level; None (and
    /// nothing added) for a blank title, which the app reports as "Give the
    /// folder a name".
    pub fn new_folder(&mut self, key: impl Into<String>, title: &str) -> Option<usize> {
        let title = title.trim();
        if title.is_empty() {
            return None;
        }
        self.tabs.push(Tab::folder(key, title));
        Some(self.tabs.len() - 1)
    }

    /// Rename the folder with `key`; false when blank or not a folder.
    pub fn rename_folder(&mut self, key: &str, title: &str) -> bool {
        let title = title.trim();
        if title.is_empty() {
            return false;
        }
        match self.tabs.iter_mut().find(|t| t.key == key && t.is_folder()) {
            Some(folder) => {
                folder.title = title.to_string();
                true
            }
            None => false,
        }
    }

    /// The `active` value for the terminal nearest the row at `pos`: the first
    /// one after its subtree, else the last one before it, else the manager.
    pub fn nearest_terminal(&self, pos: usize) -> usize {
        let end = subtree_end(&self.tabs, pos);
        (end..self.tabs.len())
            .chain((0..pos).rev())
            .find(|&i| !self.tabs[i].is_folder())
            .map_or(0, |i| i + 1)
    }

    /// Moves the selection off a folder, which cannot be shown as a tab, and
    /// off the end of the list.
    pub fn settle_active(&mut self) {
        if let Some(index) = self.active.checked_sub(1) {
            if index >= self.tabs.len() {
                self.active = 0;
            } else if self.tabs[index].is_folder() {
                self.active = self.nearest_terminal(index);
            }
        }
        self.reveal_active();
    }

    /// Expands any collapsed ancestor so the active tab's row is on screen.
    pub fn reveal_active(&mut self) {
        let Some(index) = self.active_index() else {
            return;
        };
        for ancestor in ancestors(&self.tabs, index) {
            self.tabs[ancestor].collapsed = false;
        }
    }

    /// The next tab (⌘⇧]) skipping folders: what `step(true)` would select.
    pub fn next_slot(&self) -> usize {
        stepped_slot(&self.folder_flags(), self.active, true)
    }

    /// The previous tab (⌘⇧[) skipping folders.
    pub fn prev_slot(&self) -> usize {
        stepped_slot(&self.folder_flags(), self.active, false)
    }

    /// Rewrite launch tabs whose session now has a pane to a plain attach;
    /// see [`promote_established_tabs`].
    pub fn promote_established(&mut self, state: &State) -> bool {
        promote_established_tabs(&mut self.tabs, state)
    }
}

/// Sign-in and enrollment tabs are one-shot interactions; everything else can be reopened.
pub fn restorable(key: &str) -> bool {
    !terminal_refreshes_saved_state(key)
}

/// A `login:` or `add:` terminal: its success refreshes the saved fleet once.
pub fn terminal_refreshes_saved_state(key: &str) -> bool {
    key.starts_with("login:") || key.starts_with("add:")
}

/// Keep the exact frozen request as the fallback for a launch whose outcome
/// was never observed (`run --restore --request-id …`). This also upgrades
/// reconnect argv saved by older desktops.
pub fn restore_terminal_command(command: &[String]) -> Vec<String> {
    let mut restored = command.to_vec();
    if command.first().is_some_and(|arg| arg == "run")
        && !matches!(command.get(1), Some(arg) if arg == "--restore")
        && command.iter().any(|arg| arg == "--attach")
        && command
            .windows(2)
            .any(|args| args[0] == "--request-id" && !args[1].is_empty())
    {
        restored.insert(1, "--restore".into());
    }
    restored
}

/// `attach M S [--read-only]`.
pub fn session_attach_command(machine: &str, session: &str, readonly: bool) -> Vec<String> {
    let mut args = vec!["attach".into(), machine.into(), session.into()];
    if readonly {
        args.push("--read-only".into());
    }
    args
}

/// Launch tabs (`run …`, `auto-machine …`) whose session exists on its
/// machine with a pane are rewritten to `attach M S [--read-only]`, so the
/// next launch reopens the session rather than replaying the launch. A
/// machine whose snapshot is missing (a blip) changes nothing. Returns
/// whether any tab changed, so the window gets persisted.
pub fn promote_established_tabs(tabs: &mut [Tab], state: &State) -> bool {
    let mut changed = false;
    for tab in tabs {
        if !tab
            .reconnect
            .first()
            .is_some_and(|arg| arg == "run" || arg == "auto-machine")
        {
            continue;
        }
        let Some((machine, session, readonly)) = &tab.session else {
            continue;
        };
        let established = state
            .machines
            .iter()
            .find(|m| m.id == *machine)
            .and_then(|m| m.last.as_ref())
            .is_some_and(|last| {
                last.sessions
                    .iter()
                    .any(|s| s.id == *session && !s.tmux_pane.is_empty())
            });
        if established {
            tab.reconnect = session_attach_command(machine, session, *readonly);
            changed = true;
        }
    }
    changed
}

/// The `active` value for tab number `n` (1-based, counting terminals only),
/// or 0 for the manager; None when there is no such tab.
pub fn numbered_slot(is_folder: &[bool], n: usize) -> Option<usize> {
    if n == 0 {
        return Some(0);
    }
    is_folder
        .iter()
        .enumerate()
        .filter(|(_, folder)| !**folder)
        .nth(n - 1)
        .map(|(i, _)| i + 1)
}

/// The next `active` value after `active` when cycling through the manager
/// (0) and every terminal, skipping folders.
pub fn stepped_slot(is_folder: &[bool], active: usize, forward: bool) -> usize {
    let slots = is_folder.len() + 1;
    let mut slot = active;
    for _ in 0..slots {
        slot = if forward {
            (slot + 1) % slots
        } else {
            (slot + slots - 1) % slots
        };
        if slot == 0 || !is_folder[slot - 1] {
            break;
        }
    }
    slot
}

/// The `active` value after the tab at `removed` closed, `remaining` tabs
/// left: a background close keeps the same tab, closing the selected tab
/// activates its right neighbor, or its left at the end.
pub fn active_after_tab_close(active: usize, removed: usize, remaining: usize) -> usize {
    let Some(index) = active.checked_sub(1) else {
        return 0;
    };
    if index > removed {
        return active - 1;
    }
    if index < removed {
        return active.min(remaining);
    }
    if remaining == 0 {
        0
    } else if removed < remaining {
        removed + 1
    } else {
        remaining
    }
}

/// The `active` value after a tab was put back at `inserted`: the restored
/// tab when it was active at close, else the same tab as before.
pub fn active_after_tab_restore(active: usize, inserted: usize, activate_restored: bool) -> usize {
    if activate_restored {
        return inserted + 1;
    }
    match active.checked_sub(1) {
        Some(index) if inserted <= index => active + 1,
        _ => active,
    }
}

#[cfg(test)]
mod tests;
