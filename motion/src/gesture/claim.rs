//! Arena claims, compound claims, and the per-root arbitration profile.
//!
//! @ref LLP 0099#gesture-composition
//!
//! A profile is the frozen arbitration topology for one root: who may claim,
//! along which axis, and which claims defer to, exclude, or require the
//! failure of which others.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// Claims one arbitration profile may declare.
pub const MAX_ARENA_CLAIMS: usize = 256;
/// Pairwise relationships one arbitration profile may declare.
pub const MAX_ARENA_RELATIONSHIPS: usize = 1_024;

/// Identifies one claim inside a profile. Non-zero, and drawn from the same
/// number space as compound ids so the two can never be confused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArenaClaimId(pub u64);

/// Identifies one compound claim inside a profile. Non-zero, and distinct from
/// every claim id in the same profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompoundClaimId(pub u64);

/// The platform arbitration policy a profile follows. The kind decides which
/// claim kinds get precedence before depth, priority, and declaration order
/// are consulted at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArenaProfileKind {
    /// iOS touch, where a claim-capable router-history claim outranks app
    /// content.
    IosTouch = 1,
    /// macOS trackpad, where a pager is preferred over the scrollers around
    /// it.
    MacosTrackpad = 2,
    /// Mobile web. No reserved-edge precedence of its own; the browser's edge
    /// handling reaches the arena as external ownership instead.
    MobileWeb = 3,
    /// Desktop web. Only a `System` claim outranks ordinary depth and priority
    /// order.
    DesktopWeb = 4,
}

impl TryFrom<u8> for ArenaProfileKind {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::IosTouch),
            2 => Ok(Self::MacosTrackpad),
            3 => Ok(Self::MobileWeb),
            4 => Ok(Self::DesktopWeb),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.profile",
                value,
            }),
        }
    }
}

/// What kind of thing is competing for a stream. Kind is what the platform
/// policies rank on; claims of equal standing are then separated by depth,
/// priority, and declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArenaClaimKind {
    /// An authored recognizer, including a composition that presents itself to
    /// the arena as a single claim.
    Gesture = 1,
    /// Back or forward navigation, such as an edge swipe. Eligible only while
    /// the snapshot's back mode is claim-capable.
    RouterHistory = 2,
    /// A paging container that turns discretely between pages.
    Pager = 3,
    /// A scroll container. The only legal target of a pre-lock standdown.
    Scroll = 4,
    /// A platform-owned claim. It outranks every other kind in every profile.
    System = 5,
}

impl TryFrom<u8> for ArenaClaimKind {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Gesture),
            2 => Ok(Self::RouterHistory),
            3 => Ok(Self::Pager),
            4 => Ok(Self::Scroll),
            5 => Ok(Self::System),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.claim.kind",
                value,
            }),
        }
    }
}

/// The axis a claim competes on. A standdown is only legal between claims that
/// share one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArenaAxis {
    /// Left and right movement. Which direction counts as positive follows the
    /// root's layout direction.
    Horizontal = 1,
    /// Up and down movement.
    Vertical = 2,
}

impl TryFrom<u8> for ArenaAxis {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Horizontal),
            2 => Ok(Self::Vertical),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.claim.axis",
                value,
            }),
        }
    }
}

/// The direction along a claim's axis that it consumes. A scroll claim that
/// has run out of travel in its direction stops being eligible, which is how a
/// nested scroller hands the gesture back to a pager at its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArenaDirection {
    /// Toward decreasing coordinates: leading on the horizontal axis, upward
    /// on the vertical.
    Negative = 1,
    /// Toward increasing coordinates: trailing on the horizontal axis,
    /// downward on the vertical.
    Positive = 2,
}

impl TryFrom<u8> for ArenaDirection {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Negative),
            2 => Ok(Self::Positive),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.claim.direction",
                value,
            }),
        }
    }
}

/// The class of input a stream carries. Frozen at stream entry and recorded on
/// every receipt; the arena never re-derives it from later samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArenaInputKind {
    /// Direct contacts on a touchscreen.
    Touch = 1,
    /// Mouse, pen, or a unified web pointer stream.
    Pointer = 2,
    /// Discrete or momentum wheel input.
    Wheel = 3,
    /// Samples relayed from a platform recognizer rather than raw contacts.
    PlatformRecognizer = 4,
}

impl TryFrom<u8> for ArenaInputKind {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Touch),
            2 => Ok(Self::Pointer),
            3 => Ok(Self::Wheel),
            4 => Ok(Self::PlatformRecognizer),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.input_kind",
                value,
            }),
        }
    }
}

/// How much of a back navigation a root can currently offer. A `RouterHistory`
/// claim is only eligible while this is claim-capable, so a root with no
/// history cannot have its edge taken by a navigation claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BackMode {
    /// No back navigation is available on this root.
    None = 0,
    /// Back exists but is advisory only; nothing may claim a stream for it.
    PassiveIntent = 1,
    /// Back may be claimed, but it commits at the end of the gesture rather
    /// than tracking the pointer.
    DiscreteClaim = 2,
    /// Back may be claimed and driven interactively. A stream whose scene
    /// lease was not acquired is downgraded to `DiscreteClaim`.
    InteractiveClaim = 3,
}

impl BackMode {
    pub(crate) fn is_claim_capable(self) -> bool {
        matches!(self, Self::DiscreteClaim | Self::InteractiveClaim)
    }
}

impl TryFrom<u8> for BackMode {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::PassiveIntent),
            2 => Ok(Self::DiscreteClaim),
            3 => Ok(Self::InteractiveClaim),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.back_mode",
                value,
            }),
        }
    }
}

/// One claim as declared by a profile. This is authored data: the arena reads
/// it at every checkpoint and never rewrites it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaClaimDescriptor {
    /// Profile-unique, non-zero identity used wherever this claim is named.
    pub id: ArenaClaimId,
    /// The recognizer this claim speaks for, when there is one. Claims standing
    /// for a container rather than an authored gesture leave it unset.
    pub gesture_id: Option<GestureId>,
    /// What is competing, which is what the platform policy ranks on.
    pub kind: ArenaClaimKind,
    /// The axis this claim competes on.
    pub axis: ArenaAxis,
    /// The direction along that axis this claim can consume.
    pub direction: ArenaDirection,
    /// Nesting depth in the hit path. Deeper wins first, so same-axis nesting
    /// resolves to the deepest claim able to consume.
    pub depth: u16,
    /// Author-declared tiebreak among claims at equal depth; higher wins.
    pub priority: i16,
    /// The final tiebreak, so a winner never depends on the order samples
    /// happen to arrive.
    pub declaration_order: u16,
    /// The compound this claim belongs to, if any. It has to agree with that
    /// compound's member list; a one-sided membership is rejected.
    pub compound: Option<CompoundClaimId>,
}

/// How a compound's members behave behind its single lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompoundPolicy {
    /// Every eligible member runs at once, and more may join while the lease
    /// is active. The profile requires a declared `Simultaneous` relationship
    /// for every pair of members.
    Simultaneous = 1,
    /// One member runs at a time: the first eligible in declaration order.
    Exclusive = 2,
}

impl TryFrom<u8> for CompoundPolicy {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Simultaneous),
            2 => Ok(Self::Exclusive),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.compound.policy",
                value,
            }),
        }
    }
}

/// The tag for a declared pairwise relationship, used when decoding a profile.
/// [`ArenaRelationship`] is the decoded form that carries the operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArenaRelationshipKind {
    /// The claimant may not activate until the required claim has failed.
    RequireFailure = 1,
    /// Two claims may hold the lease together. Legal only within one
    /// simultaneous compound.
    Simultaneous = 2,
    /// One claim is tried before the other.
    Exclusive = 3,
}

impl TryFrom<u8> for ArenaRelationshipKind {
    type Error = ArenaProfileError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::RequireFailure),
            2 => Ok(Self::Simultaneous),
            3 => Ok(Self::Exclusive),
            _ => Err(ArenaProfileError::InvalidEnum {
                field: "arena.relationship.kind",
                value,
            }),
        }
    }
}

/// One compound claim: the identity the arena arbitrates over, plus the
/// members that actually run behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundClaimDescriptor {
    /// Profile-unique, non-zero identity, distinct from every claim id.
    pub id: CompoundClaimId,
    /// Whether the members run together or one at a time.
    pub policy: CompoundPolicy,
    /// Declaration order is semantically significant for `Exclusive`.
    pub members: Vec<ArenaClaimId>,
}

/// A declared pairwise relationship between two claims of one profile. A
/// profile stores these in a sorted set, so the same declarations compare
/// equal whatever order they were given in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArenaRelationship {
    /// The claimant waits for another claim to fail before it may activate.
    RequireFailure {
        /// The claim that has to wait.
        claimant: ArenaClaimId,
        /// The claim whose failure unblocks it. If it activates instead, the
        /// claimant is blocked for the rest of the stream.
        required: ArenaClaimId,
    },
    /// Two members of one simultaneous compound may hold the lease together.
    Simultaneous {
        /// The lower-numbered claim; the pair is normalized on construction, so
        /// declaring it either way round yields this same relationship.
        first: ArenaClaimId,
        /// The higher-numbered claim.
        second: ArenaClaimId,
    },
    /// `preferred` is tried before `fallback`; ties never depend on arrival
    /// order.
    Exclusive {
        /// The claim that gets the first chance to activate.
        preferred: ArenaClaimId,
        /// The claim held back while the preferred one is still `Possible` and
        /// eligible.
        fallback: ArenaClaimId,
    },
}

/// Why a profile failed validation. A profile is checked once at construction
/// so arbitration can index into it without re-validating at each checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaProfileError {
    /// A wire value did not name any variant of the field's enum.
    InvalidEnum {
        /// Dotted name of the field being decoded, for diagnostics.
        field: &'static str,
        /// The value that named no variant.
        value: u8,
    },
    /// A claim or compound id was zero; the named field has to be non-zero.
    ZeroIdentity(&'static str),
    /// The profile declares more than [`MAX_ARENA_CLAIMS`] claims.
    TooManyClaims,
    /// The profile declares more than [`MAX_ARENA_RELATIONSHIPS`] relationships.
    TooManyRelationships,
    /// Two claims share an id.
    DuplicateClaim(ArenaClaimId),
    /// Two compounds share an id.
    DuplicateCompound(CompoundClaimId),
    /// The same relationship was declared twice, counting a simultaneous pair
    /// given in either order as one relationship.
    DuplicateRelationship,
    /// A compound member or relationship operand names a claim the profile
    /// does not declare.
    UnknownClaim(ArenaClaimId),
    /// A relationship names the same claim on both sides.
    SelfRelationship(ArenaClaimId),
    /// The require-failure edges contain a cycle, so no claim on it could ever
    /// activate.
    RequireFailureCycle,
    /// A compound has fewer than two members.
    CompoundTooSmall(CompoundClaimId),
    /// A compound id collides with a claim id; the two share one number space.
    CompoundIdentityCollision(CompoundClaimId),
    /// A claim and a compound disagree about membership: one names the other
    /// and the other does not name it back.
    CompoundMembershipMismatch(ArenaClaimId),
    /// A compound lists the same claim twice.
    DuplicateCompoundMember(ArenaClaimId),
    /// A `Simultaneous` relationship was declared between claims that are not
    /// both members of one simultaneous compound.
    SimultaneousOutsideCompound,
    /// A simultaneous compound left some pair of its members undeclared; every
    /// pair needs its own `Simultaneous` relationship.
    IncompleteSimultaneousCompound(CompoundClaimId),
    /// Both directions of an `Exclusive` relationship were declared, so neither
    /// claim could be the preferred one.
    ContradictoryRelationship,
}

/// The validated arbitration topology one arena runs on. Everything is checked
/// when the profile is built, so a checkpoint can look claims, compounds, and
/// relationships up without questioning them.
#[derive(Debug, Clone)]
pub struct ArenaProfile {
    /// The platform policy applied before depth, priority, and declaration
    /// order are consulted.
    pub kind: ArenaProfileKind,
    pub(crate) claims: BTreeMap<ArenaClaimId, ArenaClaimDescriptor>,
    pub(crate) compounds: BTreeMap<CompoundClaimId, CompoundClaimDescriptor>,
    relationships: BTreeSet<ArenaRelationship>,
    pub(crate) requires: BTreeMap<ArenaClaimId, Vec<ArenaClaimId>>,
    pub(crate) exclusive_predecessors: BTreeMap<ArenaClaimId, Vec<ArenaClaimId>>,
}

impl ArenaProfile {
    /// Validate a set of claims, compounds, and relationships into a profile.
    ///
    /// All of the structural rules are enforced here so none of them have to be
    /// re-checked during arbitration: ids are non-zero and unique across both
    /// number spaces, compound membership agrees in both directions, a
    /// simultaneous compound declares every pair of its members, `Exclusive` is
    /// not declared in both directions, and the require-failure edges are
    /// acyclic. A `Simultaneous` pair given in either order normalizes to one
    /// relationship.
    pub fn new(
        kind: ArenaProfileKind,
        claims: impl IntoIterator<Item = ArenaClaimDescriptor>,
        compounds: impl IntoIterator<Item = CompoundClaimDescriptor>,
        relationships: impl IntoIterator<Item = ArenaRelationship>,
    ) -> Result<Self, ArenaProfileError> {
        let mut claim_map = BTreeMap::new();
        for claim in claims {
            let claim_id = claim.id;
            if claim.id.0 == 0 {
                return Err(ArenaProfileError::ZeroIdentity("arena.claim.id"));
            }
            if claim_map.insert(claim.id, claim).is_some() {
                return Err(ArenaProfileError::DuplicateClaim(claim_id));
            }
            if claim_map.len() > MAX_ARENA_CLAIMS {
                return Err(ArenaProfileError::TooManyClaims);
            }
        }
        let mut compound_map = BTreeMap::new();
        let claim_raw_ids: BTreeSet<_> = claim_map.keys().map(|id| id.0).collect();
        for compound in compounds {
            let compound_id = compound.id;
            if compound.id.0 == 0 {
                return Err(ArenaProfileError::ZeroIdentity("arena.compound.id"));
            }
            if claim_raw_ids.contains(&compound.id.0) {
                return Err(ArenaProfileError::CompoundIdentityCollision(compound.id));
            }
            if compound.members.len() < 2 {
                return Err(ArenaProfileError::CompoundTooSmall(compound.id));
            }
            let mut unique = BTreeSet::new();
            for member in &compound.members {
                if !claim_map.contains_key(member) {
                    return Err(ArenaProfileError::UnknownClaim(*member));
                }
                if !unique.insert(*member) {
                    return Err(ArenaProfileError::DuplicateCompoundMember(*member));
                }
                if claim_map.get(member).and_then(|claim| claim.compound) != Some(compound.id) {
                    return Err(ArenaProfileError::CompoundMembershipMismatch(*member));
                }
            }
            if compound_map.insert(compound.id, compound).is_some() {
                return Err(ArenaProfileError::DuplicateCompound(compound_id));
            }
        }
        for claim in claim_map.values() {
            if let Some(compound) = claim.compound {
                if !compound_map
                    .get(&compound)
                    .is_some_and(|value| value.members.contains(&claim.id))
                {
                    return Err(ArenaProfileError::CompoundMembershipMismatch(claim.id));
                }
            }
        }

        let mut relation_set = BTreeSet::new();
        let mut requires: BTreeMap<ArenaClaimId, Vec<ArenaClaimId>> = BTreeMap::new();
        let mut exclusive_predecessors: BTreeMap<ArenaClaimId, Vec<ArenaClaimId>> = BTreeMap::new();
        for relationship in relationships {
            let relationship = match relationship {
                ArenaRelationship::Simultaneous { first, second } if second < first => {
                    ArenaRelationship::Simultaneous {
                        first: second,
                        second: first,
                    }
                }
                other => other,
            };
            if relation_set.len() >= MAX_ARENA_RELATIONSHIPS {
                return Err(ArenaProfileError::TooManyRelationships);
            }
            let (first, second) = relationship_members(relationship);
            if first == second {
                return Err(ArenaProfileError::SelfRelationship(first));
            }
            if !claim_map.contains_key(&first) {
                return Err(ArenaProfileError::UnknownClaim(first));
            }
            if !claim_map.contains_key(&second) {
                return Err(ArenaProfileError::UnknownClaim(second));
            }
            if !relation_set.insert(relationship) {
                return Err(ArenaProfileError::DuplicateRelationship);
            }
            match relationship {
                ArenaRelationship::RequireFailure { claimant, required } => {
                    requires.entry(claimant).or_default().push(required);
                }
                ArenaRelationship::Exclusive {
                    preferred,
                    fallback,
                } => {
                    exclusive_predecessors
                        .entry(fallback)
                        .or_default()
                        .push(preferred);
                }
                ArenaRelationship::Simultaneous { first, second } => {
                    let first_compound = claim_map.get(&first).and_then(|claim| claim.compound);
                    let second_compound = claim_map.get(&second).and_then(|claim| claim.compound);
                    if first_compound != second_compound
                        || first_compound.is_none()
                        || match compound_map.get(&first_compound.expect("checked some")) {
                            Some(compound) => compound.policy != CompoundPolicy::Simultaneous,
                            None => true,
                        }
                    {
                        return Err(ArenaProfileError::SimultaneousOutsideCompound);
                    }
                }
            }
        }
        for relationship in &relation_set {
            if let ArenaRelationship::Exclusive {
                preferred,
                fallback,
            } = relationship
            {
                if relation_set.contains(&ArenaRelationship::Exclusive {
                    preferred: *fallback,
                    fallback: *preferred,
                }) {
                    return Err(ArenaProfileError::ContradictoryRelationship);
                }
            }
        }
        for compound in compound_map.values() {
            if compound.policy != CompoundPolicy::Simultaneous {
                continue;
            }
            for (index, first) in compound.members.iter().enumerate() {
                for second in compound.members.iter().skip(index + 1) {
                    let (first, second) = if first <= second {
                        (*first, *second)
                    } else {
                        (*second, *first)
                    };
                    if !relation_set.contains(&ArenaRelationship::Simultaneous { first, second }) {
                        return Err(ArenaProfileError::IncompleteSimultaneousCompound(
                            compound.id,
                        ));
                    }
                }
            }
        }
        if has_require_failure_cycle(&claim_map, &requires) {
            return Err(ArenaProfileError::RequireFailureCycle);
        }
        for values in requires.values_mut() {
            values.sort_unstable();
        }
        for values in exclusive_predecessors.values_mut() {
            values.sort_unstable();
        }
        Ok(Self {
            kind,
            claims: claim_map,
            compounds: compound_map,
            relationships: relation_set,
            requires,
            exclusive_predecessors,
        })
    }

    /// The descriptor for one claim, or `None` when this profile does not
    /// declare it.
    pub fn claim(&self, id: ArenaClaimId) -> Option<&ArenaClaimDescriptor> {
        self.claims.get(&id)
    }

    /// Every declared relationship, in sorted order. Simultaneous pairs appear
    /// normalized with the lower claim id first.
    pub fn relationships(&self) -> &BTreeSet<ArenaRelationship> {
        &self.relationships
    }
}

fn relationship_members(relationship: ArenaRelationship) -> (ArenaClaimId, ArenaClaimId) {
    match relationship {
        ArenaRelationship::RequireFailure { claimant, required } => (claimant, required),
        ArenaRelationship::Simultaneous { first, second } => (first, second),
        ArenaRelationship::Exclusive {
            preferred,
            fallback,
        } => (preferred, fallback),
    }
}

fn has_require_failure_cycle(
    claims: &BTreeMap<ArenaClaimId, ArenaClaimDescriptor>,
    requires: &BTreeMap<ArenaClaimId, Vec<ArenaClaimId>>,
) -> bool {
    fn visit(
        claim: ArenaClaimId,
        requires: &BTreeMap<ArenaClaimId, Vec<ArenaClaimId>>,
        visiting: &mut BTreeSet<ArenaClaimId>,
        visited: &mut BTreeSet<ArenaClaimId>,
    ) -> bool {
        if visited.contains(&claim) {
            return false;
        }
        if !visiting.insert(claim) {
            return true;
        }
        if requires.get(&claim).is_some_and(|dependencies| {
            dependencies
                .iter()
                .any(|dependency| visit(*dependency, requires, visiting, visited))
        }) {
            return true;
        }
        visiting.remove(&claim);
        visited.insert(claim);
        false
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    claims
        .keys()
        .any(|claim| visit(*claim, requires, &mut visiting, &mut visited))
}
