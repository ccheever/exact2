//! Read-only current poses, memoized across a visibility request. Parent and socket
//! edges share an iterative traversal so a deep hierarchy never consumes the stack.
use crate::{Affine3A, FollowTarget, Mesh, Parent, SocketFollow, Transform, World};

const WORK: usize = 1_000_000;
fn spend(remaining: &mut usize, count: usize) -> Result<(), String> {
    *remaining = remaining
        .checked_sub(count)
        .ok_or("layout pose work budget exceeded (1000000 entity and model-node visits)")?;
    Ok(())
}
fn affine(t: &Transform) -> Affine3A {
    Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}
struct Node {
    local: Affine3A,
    own: bool,
    parent: Option<usize>,
    socket: Option<(usize, Affine3A)>,
    phase: u8,
    global: Affine3A,
    available: bool,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            local: Affine3A::IDENTITY,
            own: false,
            parent: None,
            socket: None,
            phase: 0,
            global: Affine3A::IDENTITY,
            available: false,
        }
    }
}
/// Global poses have the same root-to-child affine composition as propagation,
/// but see current components and never repair a runtime cycle or mutate the world.
pub(super) fn resolve(w: &World, slots: usize) -> Result<Vec<Option<Affine3A>>, String> {
    let mut remaining = WORK;
    let mut nodes: Vec<_> = (0..slots).map(|_| Node::default()).collect();
    for e in w.entities() {
        spend(&mut remaining, 1)?;
        let n = &mut nodes[e.index() as usize];
        if let Some(t) = w.get::<Transform>(e) {
            n.local = affine(&t);
            n.own = true;
        }
        n.parent = w
            .get::<Parent>(e)
            .filter(|p| w.contains(p.0))
            .map(|p| p.0.index() as usize);
        if let Some(f) = w.get::<SocketFollow>(e) {
            let target = match &f.target {
                FollowTarget::Entity(e) => w.contains(*e).then_some(*e),
                FollowTarget::Name(name) => w.named(name),
            };
            if let Some(target) = target {
                // Bound the named-node lookup, missing-pose allocation and joint
                // ancestry before the shared animation geometry helper does work.
                let count = w
                    .get::<Mesh>(target)
                    .and_then(|mesh| match &*mesh {
                        Mesh::Asset(name) => w.model(name).map(|m| m.nodes.len()),
                        _ => None,
                    })
                    .unwrap_or(0);
                spend(&mut remaining, count.saturating_mul(3))?;
                if let Ok(local) = crate::animation::socket_local(w, target, &f.joint) {
                    n.socket = Some((
                        target.index() as usize,
                        Affine3A::from_mat4(local) * affine(&f.offset),
                    ));
                }
            }
        }
    }
    let mut stack = Vec::new();
    for e in w.entities() {
        let start = e.index() as usize;
        if nodes[start].phase == 3 {
            continue;
        }
        stack.push(start);
        while let Some(&at) = stack.last() {
            if nodes[at].phase == 0 {
                nodes[at].phase = 1;
            }
            // Prefer an available socket; an invalid target preserves G's local
            // Transform/Parent fallback, including transform-less ancestors.
            if nodes[at].phase == 1 {
                if let Some((target, local)) = nodes[at].socket {
                    match nodes[target].phase {
                        0 => {
                            stack.push(target);
                            continue;
                        }
                        3 => {
                            if nodes[target].available {
                                nodes[at].global = nodes[target].global * local;
                                nodes[at].available = true;
                                nodes[at].phase = 3;
                            }
                        }
                        _ => return Err("layout pose contains a Parent/SocketFollow cycle".into()),
                    }
                }
                if nodes[at].phase != 3 {
                    nodes[at].phase = 2;
                }
            }
            if nodes[at].phase == 2 {
                let base = if let Some(parent) = nodes[at].parent {
                    match nodes[parent].phase {
                        0 => {
                            stack.push(parent);
                            continue;
                        }
                        3 => nodes[parent].global,
                        _ => return Err("layout pose contains a Parent/SocketFollow cycle".into()),
                    }
                } else {
                    Affine3A::IDENTITY
                };
                nodes[at].global = base * nodes[at].local;
                nodes[at].available = nodes[at].own;
                nodes[at].phase = 3;
            }
            spend(&mut remaining, 1)?;
            stack.pop();
        }
    }
    Ok(nodes
        .into_iter()
        .map(|n| n.available.then_some(n.global))
        .collect())
}
