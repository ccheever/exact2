//! The design server's retention build (LLP 1121 stage 12 B1's follow-up,
//! theme2's 11:32 ruling): the pipeline as any build runs it, stopped once
//! the design's retention and synthetic tokens are known
//! (`contract_lower::retention_only`), and, on a build whose Contract the
//! design cannot have moved, the types and expansion of the last one kept
//! ([`Kept`]) in place of checking them again.

use contract_syntax::{Expanded, File};
use contract_types::{strings::Strings, Types};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

/// What a build's types and expansion are computed from, and them: the
/// loaded program with what the loader reads of the designs cleared, its
/// strings, whether it was checked mapped.
#[derive(Clone)]
pub struct Kept {
    file: File,
    strings: Option<Arc<Strings>>,
    mapped: bool,
    types: Types,
    expanded: Expanded,
}

/// What [`retention_build`] found.
pub struct Retained {
    /// The plan's retention (D7): none when the build stopped before its
    /// design.
    pub retention: Option<exact_tamber::Retention>,
    /// The app's synthetic tokens (D3), each its expression and whether a
    /// colour row writes it.
    pub candidates: Vec<(String, bool)>,
    /// This build's types and expansion, for the next.
    pub kept: Option<Kept>,
    /// Whether `kept` was reused, not checked again.
    pub reused: bool,
    /// Under `EXACT_TAMBER_RETENTION_VERIFY` (a test's): the reused types
    /// or expansion were not what checking again gives, and the fresh ones
    /// were used.
    pub stale: bool,
}

#[derive(Default)]
struct Program {
    /// Offered for reuse to this thread's next check.
    offered: Option<Kept>,
    /// Keep the next check's.
    keep: bool,
    kept: Option<Kept>,
    reused: bool,
    stale: bool,
}

thread_local! {
    static PROGRAM: RefCell<Program> = RefCell::new(Program::default());
}

/// `file` as types and expansion read it: the designs, which only lowering
/// reads, cleared (what the loader derives from them into the views, a
/// motion token's value, stays and is compared).
fn cleared(file: &File) -> File {
    File {
        design: None,
        package_designs: Vec::new(),
        ..file.clone()
    }
}

/// Build the app at `path` as far as its design's retention, reusing
/// `offered`'s types and expansion where the loaded program, its strings
/// and how it is checked are what they were computed from; else checking
/// them as any build does. Pure but for the files read and this thread's
/// lowering state.
pub fn retention_build(path: &Path, offered: Option<Kept>) -> Retained {
    /// Restores this thread's state however the build ends (a panic a
    /// caller catches too): no later build stops at retention or reuses.
    struct Scope(Option<Program>, bool);
    impl Drop for Scope {
        fn drop(&mut self) {
            contract_lower::retention_only(self.1);
            let outer = self.0.take().unwrap_or_default();
            PROGRAM.with(|p| *p.borrow_mut() = outer);
        }
    }
    let _ = contract_lower::synthetic_candidates();
    let _ = contract_lower::retention();
    let outer = PROGRAM.with(|p| {
        std::mem::replace(
            &mut *p.borrow_mut(),
            Program {
                offered,
                keep: true,
                ..Program::default()
            },
        )
    });
    let scope = Scope(Some(outer), contract_lower::retention_only(true));
    let _ = crate::compile_path_all(path, false);
    let program = PROGRAM.with(|p| std::mem::take(&mut *p.borrow_mut()));
    drop(scope);
    Retained {
        retention: contract_lower::retention(),
        candidates: contract_lower::synthetic_candidates(),
        kept: program.kept,
        reused: program.reused,
        stale: program.stale,
    }
}

/// The offered types and expansion, when `file`, `strings` and `mapped`
/// are what they were computed from; none otherwise, or when nothing is
/// offered (any build but the design server's).
pub(crate) fn reuse(
    file: &File,
    strings: &Option<Arc<Strings>>,
    mapped: bool,
) -> Option<(Types, Expanded)> {
    let offered = PROGRAM.with(|p| p.borrow_mut().offered.take())?;
    // A design that doesn't parse is a doubt, whatever the loader made of
    // it: checked again (condition (c)).
    let parses = file
        .design
        .iter()
        .chain(&file.package_designs)
        .all(|d| exact_tamber::parse(&d.text).is_ok());
    let same = parses
        && offered.mapped == mapped
        && offered.strings == *strings
        && offered.file == cleared(file);
    same.then(|| {
        PROGRAM.with(|p| p.borrow_mut().reused = true);
        (offered.types, offered.expanded)
    })
}

/// Keep what `file` checked to, when the design server asked.
pub(crate) fn keep(
    file: &File,
    strings: &Option<Arc<Strings>>,
    mapped: bool,
    types: &Types,
    expanded: &Expanded,
) {
    PROGRAM.with(|p| {
        let mut p = p.borrow_mut();
        if p.keep {
            p.kept = Some(Kept {
                file: cleared(file),
                strings: strings.clone(),
                mapped,
                types: types.clone(),
                expanded: expanded.clone(),
            });
        }
    })
}

/// Whether reused types and expansion are to be checked against fresh
/// ones (`EXACT_TAMBER_RETENTION_VERIFY`, theme2's condition (b)): a
/// test's, so that a pass that comes to read the design fails a test
/// instead of going stale.
pub(crate) fn verifying() -> bool {
    std::env::var_os("EXACT_TAMBER_RETENTION_VERIFY").is_some()
}

/// Whether the `reused` types and expansion are the `fresh` ones; a
/// mismatch is said ([`Retained::stale`]).
pub(crate) fn verify(fresh: (&Types, &Expanded), reused: (&Types, &Expanded)) -> bool {
    let same = format!("{:?}", fresh.0) == format!("{:?}", reused.0) && fresh.1 == reused.1;
    if !same {
        PROGRAM.with(|p| p.borrow_mut().stale = true);
    }
    same
}

/// Checking again refused what reuse would have taken (condition (b)): said
/// as [`Retained::stale`].
pub(crate) fn refused_fresh() {
    PROGRAM.with(|p| p.borrow_mut().stale = true);
}
