//! Standard round-trip formatting; no formatter dependency.
use std::fmt::{self, Write};
pub struct Float<T>(pub T);
impl<T: fmt::Display> fmt::Display for Float<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
pub(crate) fn shortest<T: fmt::Display + fmt::Debug>(
    out: &mut impl Write,
    n: T,
    debug: bool,
) -> fmt::Result {
    if debug {
        write!(out, "{n:?}")
    } else {
        write!(out, "{n}")
    }
}
