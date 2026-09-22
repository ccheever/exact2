/// Portable app-relative asset path. Shared as source with the GPU-free game codec.
pub fn asset_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii() && !b.is_ascii_control() && b != b'\\')
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
