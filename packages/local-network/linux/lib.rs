//! @exact/local-network on Linux and Windows (LLP 1133.036 §13), composed
//! into the app's host crate by the desktop builder (LLP 1133.022 D10 item 5).
//!
//! Neither host browses yet: Linux's browser over Avahi's D-Bus API and
//! Windows' over `DnsServiceBrowse` are not built, so `available` is false,
//! and a browse or resolve fails `NotSupportedError`. Nothing on either
//! asks for local network access, so the state is `granted` and a typed
//! address under `net.fetch local-network` is reached as on any host.

use exact_linux::package::{json, Context, Json, Package, Reply};

/// The session's instance; it holds nothing.
pub struct LocalNetwork;

impl Package for LocalNetwork {
    fn new(_: Context) -> Self {
        LocalNetwork
    }

    fn later(&mut self, name: &str, _: &Json, _: Option<&Json>, reply: Reply) {
        match name {
            "localNetworkStatus" => {
                reply.send(json!({ "available": false, "state": "granted", "android": null }))
            }
            _ => reply.error("NotSupportedError", None),
        }
    }
}
