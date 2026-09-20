//! Shortest round-trip float output, including signed zero. Inspection checks finiteness.
use std::fmt::{self, Write};
pub(crate) fn shortest<T: ryu::Float + Copy>(
    out: &mut impl Write,
    n: T,
    debug: bool,
) -> fmt::Result {
    let mut buffer = ryu::Buffer::new();
    let text = buffer.format(n);
    out.write_str(if debug {
        text
    } else {
        text.strip_suffix(".0").unwrap_or(text)
    })
}
