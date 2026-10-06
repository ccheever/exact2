//! What an entity draws: its presentation `DrawnMesh` swaps its simulated `Mesh`.
use exact_game::{DrawnMesh, Mesh, Transform, World};

/// What an entity draws, for `with`: its presentation `DrawnMesh`, else its `Mesh`.
pub(crate) fn shown<R>(
    w: &World,
    e: exact_game::Entity,
    with: impl FnOnce(&Mesh) -> R,
) -> Option<R> {
    if let Some(d) = w.get::<DrawnMesh>(e) {
        return Some(with(&d.mesh));
    }
    w.get::<Mesh>(e).map(|m| with(&m))
}
/// Every drawn mesh with a pose, simulated ones first (each without a
/// `DrawnMesh`), then the presentation ones, each in entity order.
pub(crate) fn each_shown<E>(
    w: &World,
    mut visit: impl FnMut(exact_game::Entity, &Mesh) -> Result<(), E>,
) -> Result<(), E> {
    for (e, (mesh, _)) in w.query::<(&Mesh, &Transform)>().iter() {
        if !w.has::<DrawnMesh>(e) {
            visit(e, mesh)?;
        }
    }
    for (e, (d, _)) in w.query::<(&DrawnMesh, &Transform)>().iter() {
        visit(e, &d.mesh)?;
    }
    Ok(())
}
