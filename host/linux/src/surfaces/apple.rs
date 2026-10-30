//! CPU frames use the same layer while Metal's archive miss compiles.
//! @ref LLP 1149 D6.3; LLP 1149.001 Stage 0b.
use exact_ink::device::apple::Startup;

pub(crate) struct Boot {
    pub preparing: Startup,
    pub cpu_submitted: bool,
}
