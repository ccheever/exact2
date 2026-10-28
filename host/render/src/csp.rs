//! The Content-Security-Policy of the pages the render server serves (LLP
//! 1048.000 D11), the boundary there, since the web runtime's admission is
//! parity (LLP 1133.036 D5).
use std::fmt::Write as _;
use std::path::Path;

/// The pages' Content-Security-Policy (D11): scripts only from the app's
/// origin (the only inline scripts are inert data), the wasm, and fetches
/// and sockets to the origins the app's grants name.
pub(crate) fn csp(grants: &str, dist: &Path) -> String {
    let mut connect = String::from("'self'");
    for line in grants.lines() {
        let mut words = line.split_whitespace();
        let Some(capability @ ("net.fetch" | "net.websocket")) = words.next() else {
            continue;
        };
        if let Some(target) = words.next() {
            // An origin is scheme://host[:port]: the target up to its path.
            // A scheme-wide line (`ws:`) is already a CSP scheme source.
            // `local-network` (LLP 1133.036 D5): CSP has no source for an
            // address range, and a scheme source would admit the internet,
            // so a served page reaches `.local` names alone by it.
            let origin = match target.strip_suffix(':').map(str::to_ascii_lowercase) {
                _ if target == "local-network" && capability == "net.fetch" => {
                    Some("http://*.local:* https://*.local:*".to_owned())
                }
                _ if target == "local-network" => Some("ws://*.local:* wss://*.local:*".to_owned()),
                Some(s) if matches!(&*s, "http" | "https" | "ws" | "wss") => Some(s + ":"),
                _ => target.split_once("://").map(|(scheme, rest)| {
                    format!("{scheme}://{}", rest.split('/').next().unwrap_or(""))
                }),
            };
            if let Some(origin) = origin {
                connect.push(' ');
                connect.push_str(&origin);
            }
        }
    }
    // The admitted TypeScript module and its host prelude run in a private
    // same-origin realm. Admit their exact baked bytes, never arbitrary inline JS.
    // A document's one inline script, the host's capture script, likewise.
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| exact_data::envelope::base64(&Sha256::digest(bytes));
    // The JavaScript runtime's capture script too (LLP 1071): its pages carry
    // that one; and the script a boot document's page removes it with (LLP
    // 1048.005).
    let mut scripts = format!(
        "'self' 'wasm-unsafe-eval' 'sha256-{}' 'sha256-{}' 'sha256-{}' 'sha256-{}'",
        hash(crate::page::capture().as_bytes()),
        hash(crate::page::capture_js().as_bytes()),
        hash(crate::page::scroll_document_js().as_bytes()),
        hash(crate::direct::boot_swap_js().as_bytes())
    );
    for file in ["module-binary.js", "module-prelude.js", "app.js"] {
        if let Ok(bytes) = std::fs::read(dist.join(file)) {
            let _ = write!(scripts, " 'sha256-{}'", hash(&bytes));
        }
    }
    format!(
        "default-src 'self'; script-src {scripts}; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; media-src 'self' blob: https:; font-src 'self' data:; connect-src {connect}; worker-src 'self' blob:; frame-src 'self' https:; object-src 'none'; base-uri 'self'; frame-ancestors 'self'"
    )
}
