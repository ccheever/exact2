/// The compiler identity a plan carries: the crate version folded with the
/// configuration digest (there is no configuration yet).
pub fn compiler_identity() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in concat!("contract-lower ", env!("CARGO_PKG_VERSION")).bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}
