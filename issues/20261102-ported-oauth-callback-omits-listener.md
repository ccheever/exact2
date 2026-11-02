# An explicitly ported OAuth callback omits its macOS listener

**Status:** Closed
**Severity:** P2
**Systems:** Bake artifact graph, macOS authentication, Apple packaging
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

A production macOS build with store level `0`, no `net.listen` grant, and
`auth.callback http://127.0.0.1:1455/auth/callback` accepts the callback at
runtime but packages no receiver for it. `bake/src/receipt.rs:668-677`
recognizes only callback grants with a slash immediately after the IP
literal. An explicit port therefore leaves `listen` out of the artifact
graph, for IPv4 and IPv6 alike.

`runner/src/auth.rs:442-446` admits an exact callback grant, including its
port, after checking that the request is a valid loopback callback. The
portless grant is useful for admitting any port; it is not the only grant
the runtime accepts. Packaging should carry the receiver for every
admitted loopback callback or reject unsupported declarations at bake.

`host/apple/build.mjs:1001-1006` uses this graph to leave modules out of
fixed production builds; `:1066` and `:1091` then omit
`libexact_listen.dylib`. `AuthLoopback.swift:46-48` consequently answers
501, telling the app to rebuild with a grant it already has. Development
and updating builds carry all modules, so the mismatch appears only when
shipping the fixed binary.

Reproduce with the public admission and graph APIs:

```rust
fn main() {
    let plan=exact_plan::builder::PlanBuilder::new(0,0).finish().unwrap();
    for callback in ["http://127.0.0.1/auth/callback", "http://127.0.0.1:1455/auth/callback", "http://[::1]:1455/auth/callback"] {
        let grants=format!("auth.session https://auth.example\nauth.callback {callback}");
        let session=exact_runner::auth::Session {
            url:"https://auth.example/authorize".into(), callback:callback.replace("127.0.0.1/","127.0.0.1:1455/"), state:"s".into(), ephemeral:true
        };
        let admitted=exact_runner::auth::check_grants(&session,&grants,None,None).is_ok();
        let graph=exact_bake::plan_graph(&plan.encode(),&serde_json::json!({"grantCeiling":grants}),"aarch64-apple-darwin").unwrap();
        println!("{callback}: admitted={admitted}, loads={}",graph["loads"]);
    }
}
```

The probe is saved as `/tmp/exact-review-loopback-load.rs`. Using the
reviewed checkout's existing build artifacts:

```sh
rustc --edition=2021 /tmp/exact-review-loopback-load.rs --crate-name exact_review_loopback_load -L dependency=target/debug/deps --extern exact_bake=target/debug/deps/libexact_bake-905d712826898709.rlib --extern exact_plan=target/debug/deps/libexact_plan-57b7cd1485dec34b.rlib --extern exact_runner=target/debug/deps/libexact_runner-ea99ef2cb22d8243.rlib --extern serde_json=target/debug/deps/libserde_json-c60d5ef330a27bfe.rlib -o /tmp/exact-review-loopback-load
/tmp/exact-review-loopback-load
```

Observed:

```text
http://127.0.0.1/auth/callback: admitted=true, loads=["listen"]
http://127.0.0.1:1455/auth/callback: admitted=true, loads=[]
http://[::1]:1455/auth/callback: admitted=true, loads=[]
```

This directly proves the admission/packaging-graph mismatch. No app was
built or launched; the missing-library 501 follows from the packaging
and loader paths above. No authorization server was contacted.

Duplicate check: searched this checkout's current issues and both related
issue corpora, including closed tickets. The existing multiple-loopback
sign-in ticket concerns simultaneous requests and callback validation;
it does not cover listener reachability in the bake graph.

**Resolution.** Use the runner’s loopback callback parser when deciding whether an explicit callback URL needs the native listener, including IPv4 and IPv6 callbacks with ports.

**Verification after the fix.** The bake library tests passed, including portless IPv4, IPv4 port 1455, IPv6 port 1455 and a remote callback that does not require the listener.
