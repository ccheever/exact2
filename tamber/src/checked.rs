//! The last few designs a server's reads checked, with their answers
//! (theme2's condition on the counts' dial influence; `check_design_cached`):
//! a design asked about again, unchanged, is answered from here, so a client
//! asking (`describe`, `resolve`, a scrub's frame) can't make the server fold
//! the same design, its dial influence included, again and again. Keyed by the whole input (the design, the
//! vocabulary, whether the fold is traced), compared whole, not by a hash
//! alone.

use crate::{Design, Diagnostic, Folding, Vocab};
use std::sync::Mutex;

type Answer = Result<Folding, Vec<Diagnostic>>;

/// The most designs kept.
const KEPT: usize = 4;

static CHECKED: Mutex<Vec<(Design, Vocab, bool, Answer)>> = Mutex::new(Vec::new());

/// `check` of `design` and `vocab`, or the answer it gave last time.
pub(crate) fn cached(design: Design, vocab: &Vocab, check: fn(Design, &Vocab) -> Answer) -> Answer {
    // (a unit test's own switches change answers for one input: never kept)
    if cfg!(test) {
        return check(design, vocab);
    }
    let traced = crate::trace::on();
    let hit = |kept: &mut Vec<(Design, Vocab, bool, Answer)>| {
        let i = kept
            .iter()
            .position(|(d, v, t, _)| *t == traced && v == vocab && *d == design)?;
        // The most recent last.
        let entry = kept.remove(i);
        let answer = entry.3.clone();
        kept.push(entry);
        Some(answer)
    };
    if let Some(answer) = CHECKED.lock().ok().and_then(|mut kept| hit(&mut kept)) {
        return answer;
    }
    let answer = check(design.clone(), vocab);
    if let Ok(mut kept) = CHECKED.lock() {
        if kept.len() >= KEPT {
            drop(kept.remove(0));
        }
        kept.push((design, vocab.clone(), traced, answer.clone()));
    }
    answer
}
