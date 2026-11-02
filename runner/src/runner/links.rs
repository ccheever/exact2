//! What a host links of the answers the runner gives itself.

use super::{canvas2d, router, RunnerError};
use crate::DataError;
use exact_plan::{Plan, Value};

/// What a host links of the answers the runner gives itself:
/// each capability's, or `None` when the artifact doesn't link it, so its
/// code is gone. Native hosts and tests boot with [`RunnerLinks::ALL`]; the
/// web host passes what its entry registered.
#[derive(Clone, Copy)]
pub struct RunnerLinks {
    /// A GPU surface's published record, as its `exactSurface` resource.
    pub surface_answer: SurfaceAnswer,
    /// The plan's router, from its route table and shapes.
    pub router: RouterLink,
    /// The list engines: [`crate::instance::LISTS`].
    pub lists: Option<&'static crate::instance::ListLinks>,
    /// Canvas 2D, with surfaces: [`canvas2d::engine`].
    pub canvas: CanvasLink,
    /// `formatDate` and `formatNumber`:
    /// [`crate::formatting`].
    pub format: FormatLink,
    /// `frame` and `measure`: the kernel's answers
    /// natively ([`crate::geometry::KERNEL`]), the page's on the web.
    pub geometry: GeometryLink,
    /// The I/O grant grammar: [`crate::grants::validate`].
    pub grants: crate::grants::GrantsLink,
    /// CSS animations' `@keyframes`, resolved for a node's `animation` list:
    /// [`crate::bridge::KeyframesTable::resolve`].
    pub keyframes: crate::bridge::KeyframesLink,
    /// `font-weight`, `font-stretch` and `font-variation-settings`
    /// transitions and keyframes, which the runner
    /// plays: `None` where a browser plays them.
    pub text_axes: super::text_axes::TextAxesLink,
}

/// How the VM reaches the `format` capability's entries, when linked: the
/// entry and its arguments to its value, `None` when they don't fit.
pub type FormatLink = Option<fn(exact_plan::Stdlib, &[Value]) -> Option<Value>>;

/// How the runner answers geometry reads, when linked.
pub type GeometryLink = Option<&'static crate::geometry::GeometryLinks>;

/// How a runner makes its Canvas 2D engine, when linked.
pub type CanvasLink = Option<fn() -> Box<dyn canvas2d::CanvasEngine>>;

/// How a host builds a plan's router: [`router::routing`], when linked.
pub type RouterLink = Option<fn(&Plan) -> Result<Option<Box<dyn router::Routing>>, RunnerError>>;

/// The runner's answer for a surface resource, when a host links surfaces:
/// [`crate::surface_record::answer`].
pub type SurfaceAnswer =
    Option<fn(&Plan, &exact_kernel::SortedMap<String, String>, usize) -> Result<Value, DataError>>;

impl RunnerLinks {
    /// Every capability.
    pub const ALL: RunnerLinks = RunnerLinks {
        surface_answer: Some(crate::surface_record::answer),
        router: Some(router::routing),
        lists: Some(&crate::instance::LISTS),
        canvas: Some(canvas2d::engine),
        format: Some(crate::format::formatting),
        geometry: Some(&crate::geometry::KERNEL),
        grants: Some(crate::grants::validate),
        keyframes: Some(crate::bridge::KeyframesTable::resolve),
        text_axes: Some(exact_motion::Engine::new),
    };

    /// The core alone.
    pub const CORE: RunnerLinks = RunnerLinks {
        surface_answer: None,
        router: None,
        lists: None,
        canvas: None,
        format: None,
        geometry: None,
        grants: None,
        keyframes: None,
        text_axes: None,
    };
}
