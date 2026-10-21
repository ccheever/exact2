//! `explain`'s per-tuple record (LLP 1121 stage 13 D4): whether a check
//! records what each token's value names in each tuple, and the record.

thread_local! {
    /// Whether a check on this thread records [`crate::Checked::by_tuple`]
    /// ([`traced`]).
    static TRACED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `f` with every check it makes on this thread recording what each
/// token's value names in each tuple ([`crate::Checked::by_tuple`]): `explain`'s
/// (stage 13 D4). Other checks keep none, so a large design's fold costs
/// no more for it.
pub fn traced<T>(f: impl FnOnce() -> T) -> T {
    struct Put(bool);
    impl Drop for Put {
        fn drop(&mut self) {
            TRACED.with(|t| t.set(self.0));
        }
    }
    let _put = Put(TRACED.with(|t| t.replace(true)));
    f()
}

/// Whether a check starting now on this thread records ([`traced`]).
pub(crate) fn on() -> bool {
    TRACED.with(|t| t.get())
}

/// What a token's value names in each full app tuple, as it folds there:
/// a line no tuple selects in none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ByTuple {
    /// Each distinct reading: the tokens named, by their index in
    /// `Folding::tokens`, and the unset base names read bare, first time
    /// each.
    pub reads: Vec<(Vec<u32>, Vec<String>)>,
    /// Per tuple, its reading's index in `reads` (empty with no reads).
    pub at: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recording lasts the call alone, nested or unwound, on its thread
    /// alone (Astra's D4 round 3).
    #[test]
    fn tracing_is_the_calls_alone() {
        assert!(!on());
        traced(|| {
            assert!(on());
            traced(|| assert!(on()));
            assert!(on());
            // Another thread checking meanwhile records nothing.
            assert!(!std::thread::spawn(on).join().unwrap());
        });
        assert!(!on());
        let unwound = std::panic::catch_unwind(|| traced(|| panic!("in a traced check")));
        assert!(unwound.is_err());
        assert!(!on());
    }
}
