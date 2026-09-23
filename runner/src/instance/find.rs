//! Locating a view's instance: its plan site and the lexical frames in force
//! there. The kernel's parent chain names every view between the root and
//! the target, so the search descends only along that path, comparing root
//! views among the siblings it meets — never walking the rest of the tree.

use super::*;

/// Where the descent stands after matching one view of the chain.
enum Step<'a> {
    /// An instance node's view.
    Node(&'a NodeInst),
    /// A windowed list's content view (its rows sit under wrappers).
    Window(&'a RegionInst),
}

/// Whether `children` contribute `view` among their roots, directly or
/// through a nested region's arm or rows.
fn has_root(children: &[Child], view: ViewId) -> bool {
    children.iter().any(|c| match c {
        Child::Node(n) => n.view == view,
        Child::Region(r) => match (&r.window, &r.active) {
            (Some(w), _) => w.content == view,
            (None, Active::Arm { roots, .. }) => has_root(roots, view),
            (None, Active::Rows { rows }) => rows.iter().any(|row| has_root(&row.roots, view)),
        },
    })
}

/// The root among `children` whose view is `target`, pushing the frames
/// of the arms and rows crossed to reach it; `scanned` counts rows compared.
fn locate<'a>(
    children: &'a [Child],
    target: ViewId,
    frames: &mut Vec<Frame>,
    scanned: &mut usize,
) -> Option<Step<'a>> {
    if let Some(n) = children.iter().find_map(|c| match c {
        Child::Node(n) if n.view == target => Some(n),
        _ => None,
    }) {
        return Some(Step::Node(n));
    }
    for c in children {
        let Child::Region(r) = c else { continue };
        if let Some(w) = &r.window {
            if w.content == target {
                return Some(Step::Window(r));
            }
            continue;
        }
        match &r.active {
            Active::Arm { roots, frame, .. } => {
                if has_root(roots, target) {
                    frames.push(frame.clone());
                    return locate(roots, target, frames, scanned);
                }
            }
            Active::Rows { rows } => {
                if let Some(row) = rows.iter().find(|row| {
                    *scanned += 1;
                    has_root(&row.roots, target)
                }) {
                    frames.push(row.frame.clone());
                    return locate(&row.roots, target, frames, scanned);
                }
            }
        }
    }
    None
}

impl NodeInst {
    /// Whether `view` is this node's or a descendant's (a pin in a row).
    pub(super) fn contains(&self, view: ViewId) -> bool {
        self.view == view
            || self.collection.as_ref().is_some_and(|c| c.contains(view))
            || contains(&self.children, view)
    }
}

/// Whether `view` belongs to any instance under `children`.
pub(super) fn contains(children: &[Child], view: ViewId) -> bool {
    children.iter().any(|c| match c {
        Child::Node(n) => n.contains(view),
        Child::Region(r) => match &r.active {
            Active::Arm { roots, .. } => contains(roots, view),
            Active::Rows { rows } => rows.iter().any(|row| contains(&row.roots, view)),
        },
    })
}

impl Tree {
    /// The site owning `view` and the frames in force there, following
    /// `parent` (the kernel's) from the root down; `scanned` counts the
    /// rows compared on the way.
    pub fn find(
        &self,
        view: ViewId,
        parent: impl Fn(ViewId) -> Option<ViewId>,
        scanned: &mut usize,
    ) -> Option<(NodesId, Vec<Frame>)> {
        let mut chain = vec![view];
        while let Some(up) = parent(*chain.last().expect("nonempty")) {
            chain.push(up);
        }
        let mut targets = chain.into_iter().rev();
        let mut frames = Vec::new();
        let mut here: &[Child] = &self.children;
        while let Some(target) = targets.next() {
            match locate(here, target, &mut frames, scanned)? {
                Step::Node(n) if target == view => return Some((n.node, frames)),
                Step::Node(n) => match &n.collection {
                    Some(collection) => {
                        let row = collection.row_by_wrapper(targets.next()?)?;
                        frames.push(row.frame.clone());
                        here = &row.roots;
                    }
                    None => here = &n.children,
                },
                Step::Window(region) => {
                    let wrapper = targets.next()?;
                    let Active::Rows { rows } = &region.active else {
                        return None;
                    };
                    let row = rows.iter().find(|r| {
                        *scanned += 1;
                        r.wrapper == Some(wrapper)
                    })?;
                    frames.push(row.frame.clone());
                    here = &row.roots;
                }
            }
        }
        None
    }
}
