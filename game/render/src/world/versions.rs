//! The component revisions a feed compares to decide what to upload, and what
//! one feed found changed.
use super::offsets;
use exact_game::{DrawnMesh, Material, Mesh, Parent, Transform, ViewModel, Visible, World};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Versions {
    pub(super) assets: u64,
    pub(super) transform: u64,
    pub(super) parent: u64,
    pub(super) material: u64,
    pub(super) glow: u64,
    pub(super) mesh: u64,
    pub(super) visible: u64,
    pub(super) viewmodel: u64,
    pub(super) opacity: u64,
    pub(super) node_materials: u64,
    pub(super) material_overrides: u64,
    pub(super) lod: u64,
    // An animated rig moves its socket followers' subtrees without a Transform write.
    pub(super) pose: u64,
    // Presentation tints and meshes: a present's revisions move only for
    // rows whose content changed.
    pub(super) tint: u64,
    pub(super) drawn: u64,
    pub(super) live: u64,
    pub(super) membership: u64,
}
impl Versions {
    pub(super) fn of(w: &World, assets: u64) -> Self {
        Self {
            assets,
            transform: w.revision::<Transform>(),
            parent: w.revision::<Parent>(),
            material: w.revision::<Material>(),
            glow: w.revision::<exact_game::Glow>(),
            mesh: w.revision::<Mesh>(),
            visible: w.revision::<Visible>(),
            viewmodel: w.revision::<ViewModel>(),
            opacity: w.revision::<exact_game::Opacity>(),
            node_materials: w.revision::<exact_game::NodeMaterials>(),
            material_overrides: w.revision::<exact_game::MaterialOverrides>(),
            lod: w.revision::<exact_game::ModelLod>(),
            pose: w.revision::<exact_game::Pose>(),
            tint: w.revision::<exact_game::Tint>(),
            drawn: w.revision::<DrawnMesh>(),
            live: w.entities_revision(),
            membership: w.membership::<Transform>(),
        }
    }
}
/// What one feed found changed since the last, decided before any upload.
#[derive(Clone, Copy)]
pub(super) struct Pass {
    pub(super) initial: bool,
    pub(super) old: Versions,
    pub(super) next: Versions,
    pub(super) moved: bool,
    pub(super) parent_changed: bool,
    pub(super) offset_change: offsets::Change,
}
