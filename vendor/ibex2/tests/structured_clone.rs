//! The engine-side structured clone binding under the same freeze app code sees.
#![cfg(feature = "hermes")]

use ibex2::engine::hermes::{DynamicCode, Hermes};

fn check(body: &str) {
    let mut runtime = Hermes::new(DynamicCode::Closed).expect("runtime");
    let context = ibex2::bindings::Context::new(ibex2::grant::GrantSet::none());
    runtime
        .install_runtime(ibex2::bindings::Groups::DEFAULT, &context)
        .expect("bindings");
    runtime.harden().expect("harden");
    runtime
        .eval(&format!(
            r#"(function () {{
                function assert(value, message) {{
                    if (!value) throw new Error(message || "assertion failed");
                }}
                function dataCloneError(fn, label) {{
                    try {{ fn(); }} catch (error) {{
                        assert(error instanceof DOMException, "not a DOMException");
                        assert(error.name === "DataCloneError", "wrong error name: " + error.name);
                        return;
                    }}
                    throw new Error("expected DataCloneError" + (label ? ": " + label : ""));
                }}
                {body}
            }})()"#
        ))
        .unwrap_or_else(|error| panic!("{body}\n{error}"));
}

#[test]
fn primitives_wrappers_and_plain_objects_round_trip() {
    check(
        r#"
        const primitives = [undefined, null, true, false, 0, -0, 1.5, NaN,
          Infinity, -Infinity, "text", 123456789012345678901234567890n];
        for (const value of primitives) {
          const clone = structuredClone(value);
          assert(Object.is(clone, value), "primitive changed");
        }
        const wrappers = [new Boolean(true), new Number(-0), new String("hi"), Object(12n)];
        for (const value of wrappers) {
          const clone = structuredClone(value);
          assert(clone !== value, "wrapper identity kept");
          assert(clone.valueOf() === value.valueOf(), "wrapper value changed");
          assert(Object.getPrototypeOf(clone) === Object.getPrototypeOf(value), "wrapper brand changed");
        }
        let gets = 0;
        const source = { first: 1, get second() { gets++; return { nested: 2 }; } };
        Object.defineProperty(source, "hidden", { value: 3 });
        source[Symbol("ignored")] = 4;
        const clone = structuredClone(source);
        assert(clone !== source && clone.first === 1 && clone.second.nested === 2);
        assert(gets === 1, "getter count");
        assert(Object.keys(clone).join(",") === "first,second", "key order or selection");
        assert(Object.getPrototypeOf(clone) === Object.prototype, "plain prototype");
        const nullProto = Object.create(null); nullProto.x = 1;
        assert(Object.getPrototypeOf(structuredClone(nullProto)) === Object.prototype);
        const deleting = { get a() { delete this.b; return 1; }, b: 2 };
        const deletedClone = structuredClone(deleting);
        assert(Object.keys(deletedClone).join(",") === "a", "deleted key was serialized");
        "#,
    );
}

#[test]
fn arrays_keep_holes_length_properties_cycles_and_sharing() {
    check(
        r#"
        const array = new Array(5);
        array[1] = { x: 1 };
        array.extra = array[1];
        array.self = array;
        const clone = structuredClone(array);
        assert(clone !== array && clone.length === 5);
        assert(!(0 in clone) && (1 in clone) && !(2 in clone) && !(4 in clone), "holes changed");
        assert(clone[1] !== array[1] && clone.extra === clone[1], "sharing changed");
        assert(clone.self === clone, "cycle changed");
        const shared = { value: 1 };
        const graph = { a: shared, b: shared }; graph.graph = graph;
        const graphClone = structuredClone(graph);
        assert(graphClone.a === graphClone.b && graphClone.a !== shared);
        assert(graphClone.graph === graphClone);
        "#,
    );
}

#[test]
fn dates_regexps_maps_and_sets_keep_their_data_and_graph() {
    check(
        r#"
        const date = new Date(-123456789);
        const invalidDate = new Date(NaN);
        const regexp = /a+b/gi; regexp.lastIndex = 7;
        const key = { key: true };
        const map = new Map();
        const set = new Set();
        map.set(key, set); map.set("self", map); set.add(key); set.add(map);
        const source = { date, invalidDate, regexp, map, set };
        const clone = structuredClone(source);
        assert(clone.date !== date && clone.date.valueOf() === date.valueOf());
        assert(Number.isNaN(clone.invalidDate.valueOf()));
        assert(clone.regexp !== regexp && clone.regexp.source === regexp.source);
        assert(clone.regexp.flags === regexp.flags && clone.regexp.lastIndex === 0);
        const clonedKey = Array.from(clone.map.keys())[0];
        assert(clonedKey !== key && clonedKey.key === true);
        assert(clone.map.get(clonedKey) === clone.set);
        assert(clone.map.get("self") === clone.map);
        assert(clone.set.has(clonedKey) && clone.set.has(clone.map));
        "#,
    );
}

#[test]
fn buffers_views_and_every_available_typed_array_keep_bytes_and_aliasing() {
    check(
        r#"
        const names = ["Int8Array", "Uint8Array", "Uint8ClampedArray", "Int16Array",
          "Uint16Array", "Int32Array", "Uint32Array", "Float32Array", "Float64Array",
          "BigInt64Array", "BigUint64Array", "Float16Array"];
        for (const name of names) {
          const Type = globalThis[name];
          if (typeof Type !== "function") continue;
          const source = new Type(4);
          const bytes = new Uint8Array(source.buffer);
          for (let i = 0; i < bytes.length; i++) bytes[i] = i + 1;
          const clone = structuredClone(source);
          assert(Object.getPrototypeOf(clone) === Type.prototype, name + " brand");
          assert(clone !== source && clone.buffer !== source.buffer, name + " identity");
          assert(new Uint8Array(clone.buffer).every((x, i) => x === i + 1), name + " bytes");
        }
        const buffer = new ArrayBuffer(24);
        const bytes = new Uint8Array(buffer); bytes.set([1, 2, 3, 4], 4);
        const left = new Uint8Array(buffer, 4, 8);
        const right = new Uint16Array(buffer, 4, 4);
        const data = new DataView(buffer, 2, 12);
        const clone = structuredClone({ buffer, left, right, data });
        assert(clone.buffer !== buffer && clone.left.buffer === clone.buffer);
        assert(clone.right.buffer === clone.buffer && clone.data.buffer === clone.buffer);
        assert(clone.left.byteOffset === 4 && clone.left.length === 8);
        assert(clone.right.byteOffset === 4 && clone.right.length === 4);
        assert(clone.data.byteOffset === 2 && clone.data.byteLength === 12);
        assert(clone.left[0] === 1 && clone.left[1] === 2);
        "#,
    );
}

#[test]
fn errors_normalize_names_and_keep_message_and_stack() {
    check(
        r#"
        for (const name of ["Error", "EvalError", "RangeError", "ReferenceError",
                            "SyntaxError", "TypeError", "URIError"]) {
          const source = new globalThis[name]("message-" + name);
          source.stack = "stack-" + name;
          const clone = structuredClone(source);
          assert(clone !== source && clone instanceof globalThis[name], name + " brand");
          assert(clone.name === name && clone.message === source.message, name + " fields");
          assert(clone.stack === source.stack, name + " stack");
        }
        const odd = new Error("odd"); odd.name = "CustomError";
        const normalized = structuredClone(odd);
        assert(normalized.name === "Error" && normalized instanceof Error);
        "#,
    );
}

#[test]
fn text_encoder_and_decoder_are_not_cloneable() {
    check(
        r#"
        dataCloneError(() => structuredClone(new TextEncoder()), "TextEncoder");
        dataCloneError(() => structuredClone(new TextDecoder()), "TextDecoder");
        assert(Object.keys(structuredClone(Object.create(TextEncoder.prototype))).length === 0,
          "forged TextEncoder prototype was rejected");
        assert(Object.keys(structuredClone(Object.create(TextDecoder.prototype))).length === 0,
          "forged TextDecoder prototype was rejected");
        "#,
    );
}

#[test]
fn platform_identity_is_private_and_unsupported_values_throw_data_clone_error() {
    check(
        r#"
        const rejected = [function () {}, Symbol("x"), Object(Symbol("x")),
          new WeakMap(), new WeakSet(), Promise.resolve(1), new Headers(),
          new URL("https://example.com/"), new URLSearchParams("x=1"),
          new AbortController(), AbortSignal.abort(), crypto,
          new Headers().entries(), new URLSearchParams("x=1").entries()];
        if (typeof Intl !== "undefined" && typeof Intl.NumberFormat === "function") rejected.push(new Intl.NumberFormat());
        if (typeof Intl !== "undefined" && typeof Intl.DateTimeFormat === "function") rejected.push(new Intl.DateTimeFormat());
        if (typeof WeakRef === "function") rejected.push(new WeakRef({}));
        if (typeof FinalizationRegistry === "function") rejected.push(new FinalizationRegistry(function () {}));
        for (let i = 0; i < rejected.length; i++) {
          dataCloneError(() => structuredClone(rejected[i]), "rejected[" + i + "]");
        }

        const changed = [new Headers(), new URL("https://example.com/"),
          new URLSearchParams("x=1"), new AbortController(), AbortSignal.abort()];
        changed.push(new Headers().entries(), new URLSearchParams("x=1").entries());
        if (typeof Intl !== "undefined" && typeof Intl.NumberFormat === "function") changed.push(new Intl.NumberFormat());
        if (typeof Intl !== "undefined" && typeof Intl.DateTimeFormat === "function") changed.push(new Intl.DateTimeFormat());
        for (const value of changed) {
          Object.setPrototypeOf(value, null);
          Object.defineProperty(value, Symbol.toStringTag, { value: "Changed" });
          dataCloneError(() => structuredClone(value));
        }

        const exception = new DOMException("message", "AbortError");
        Object.setPrototypeOf(exception, null);
        Object.defineProperty(exception, Symbol.toStringTag, { value: "Changed" });
        const exceptionClone = structuredClone(exception);
        assert(exceptionClone instanceof DOMException, "DOMException clone brand");
        assert(exceptionClone.message === "message" && exceptionClone.name === "AbortError");

        const quotaError = new QuotaExceededError("storage full", {
          quota: 12, requested: 34
        });
        const quotaClone = structuredClone(quotaError);
        assert(quotaClone !== quotaError, "QuotaExceededError identity kept");
        assert(quotaClone instanceof QuotaExceededError, "QuotaExceededError clone brand");
        assert(quotaClone instanceof DOMException, "QuotaExceededError DOMException brand");
        assert(quotaClone.message === "storage full" &&
          quotaClone.name === "QuotaExceededError", "QuotaExceededError base fields");
        assert(quotaClone.quota === 12 && quotaClone.requested === 34,
          "QuotaExceededError numeric fields");
        const nullQuotaClone = structuredClone(new QuotaExceededError("unknown"));
        assert(nullQuotaClone instanceof QuotaExceededError, "null quota clone brand");
        assert(nullQuotaClone.quota === null && nullQuotaClone.requested === null,
          "QuotaExceededError null fields");
        let messageConversions = 0;
        const coercedQuota = new QuotaExceededError({
          toString() { messageConversions++; return "converted"; }
        });
        const coercedQuotaClone = structuredClone(coercedQuota);
        assert(coercedQuotaClone.message === "converted", "converted quota message");
        assert(messageConversions === 1, "quota message was converted again while cloning");

        class UserClass { constructor() { this.x = 1; } }
        const userClone = structuredClone(new UserClass());
        assert(userClone.x === 1 && Object.getPrototypeOf(userClone) === Object.prototype);
        const spoof = { x: 1 };
        Object.defineProperty(spoof, Symbol.toStringTag, { value: "Response" });
        const spoofClone = structuredClone(spoof);
        assert(spoofClone.x === 1 && Object.getPrototypeOf(spoofClone) === Object.prototype);

        let handleGets = 0;
        const hostileHandle = { x: 1 };
        Object.defineProperty(hostileHandle, "_handle", {
          get() { handleGets++; throw new Error("must not cross the host boundary"); }
        });
        assert(structuredClone(hostileHandle).x === 1 && handleGets === 0);

        const forgedMap = Object.create(Map.prototype); forgedMap.x = 1;
        assert(structuredClone(forgedMap).x === 1, "forged Map prototype is ordinary");
        // Hermes represents Promise state in ordinary JavaScript fields and
        // exposes no non-mutating internal-slot predicate. Pin both documented
        // prototype-check limits: a forged prototype is a false positive, and
        // severing a real Promise's prototype is a false negative.
        dataCloneError(() => structuredClone(Object.create(Promise.prototype)),
          "forged Promise prototype");
        const severedPromise = Promise.resolve(1);
        Object.setPrototypeOf(severedPromise, null);
        assert(typeof structuredClone(severedPromise) === "object");
        // Live proxies that do not expose their proxy identity are the stated
        // JavaScript-level limitation; a revoked proxy is unambiguous.
        assert(Object.keys(structuredClone(new Proxy(new Map(), {}))).length === 0);
        const revoked = Proxy.revocable(new Map(), {}); revoked.revoke();
        dataCloneError(() => structuredClone(revoked.proxy));
        "#,
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_intl_replacements_are_registered_platform_objects() {
    check(
        r#"
        for (const value of [new Intl.NumberFormat(), new Intl.DateTimeFormat()]) {
          dataCloneError(() => structuredClone(value));
          Object.setPrototypeOf(value, null);
          Object.defineProperty(value, Symbol.toStringTag, { value: "Changed" });
          dataCloneError(() => structuredClone(value));
        }
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn webcrypto_platform_objects_and_keys_are_not_cloneable_by_identity() {
    let mut runtime = Hermes::new(DynamicCode::Closed).expect("runtime");
    let context = ibex2::bindings::Context::new(ibex2::grant::GrantSet::none());
    runtime
        .install_runtime(ibex2::bindings::Groups::DEFAULT, &context)
        .expect("bindings");
    runtime.harden().expect("harden");
    runtime
        .eval(
            r#"
            globalThis.__clone_crypto_result = "pending";
            (async function () {
              function assert(value, message) {
                if (!value) throw new Error(message || "assertion failed");
              }
              function dataCloneError(value, label) {
                try { structuredClone(value); } catch (error) {
                  assert(error instanceof DOMException, label + " did not throw DOMException");
                  assert(error.name === "DataCloneError", label + " threw " + error.name);
                  return;
                }
                throw new Error(label + " was cloneable");
              }

              dataCloneError(crypto, "crypto");
              dataCloneError(crypto.subtle, "crypto.subtle");
              const key = await crypto.subtle.generateKey(
                {name: "HMAC", hash: "SHA-256", length: 128}, false, ["sign"]
              );
              dataCloneError(key, "CryptoKey");
              Object.setPrototypeOf(key, Object.prototype);
              dataCloneError(key, "CryptoKey with changed prototype");
            })().then(
              () => { globalThis.__clone_crypto_result = "ok"; },
              error => { globalThis.__clone_crypto_result = error.name + ": " + error.message; }
            );
            "#,
        )
        .expect("start clone checks");
    runtime.drain_microtasks().expect("drain clone checks");
    assert_eq!(runtime.eval("__clone_crypto_result").unwrap(), "ok");
}

#[test]
fn transfer_is_empty_or_refused_and_intrinsics_are_captured() {
    check(
        r#"
        assert(structuredClone({ x: 1 }).x === 1);
        assert(structuredClone({ x: 1 }, {}).x === 1);
        assert(structuredClone({ x: 1 }, { transfer: [] }).x === 1);
        assert(structuredClone({ x: 1 }, { transfer: new Set() }).x === 1);
        function typeError(call) {
          try { call(); } catch (error) { assert(error instanceof TypeError); return; }
          throw new Error("expected TypeError");
        }
        typeError(() => structuredClone({}, { transfer: { length: 0 } }));
        typeError(() => structuredClone({}, { transfer: null }));
        typeError(() => structuredClone({}, { transfer: 1 }));
        typeError(() => structuredClone({}, { transfer: { [Symbol.iterator]: 0 } }));
        typeError(() => structuredClone({}, { transfer: [1] }));
        typeError(() => structuredClone({}, 1));
        assert(structuredClone({ x: 1 }, null).x === 1);

        let iteratorReads = 0;
        const changingIterable = {};
        Object.defineProperty(changingIterable, Symbol.iterator, {
          get() {
            iteratorReads++;
            if (iteratorReads === 1) return [][Symbol.iterator];
            return function* () { throw new Error("iterator read twice"); };
          }
        });
        assert(structuredClone({ x: 1 }, { transfer: changingIterable }).x === 1);
        assert(iteratorReads === 1, "transfer iterator method read more than once");

        let iteratorClosed = false;
        function* primitiveThenThrow() {
          try {
            yield 1;
            throw new Error("conversion pulled the iterator twice");
          } finally {
            iteratorClosed = true;
          }
        }
        typeError(() => structuredClone({}, { transfer: primitiveThenThrow() }));
        assert(iteratorClosed, "transfer iterator was not closed after conversion failure");
        dataCloneError(() => structuredClone(new ArrayBuffer(1), { transfer: [new ArrayBuffer(1)] }));
        const source = new Map([[{ x: 1 }, { y: 2 }]]);
        Object.defineProperty(source, "forEach", {
          value: function () { throw new Error("hostile own override"); }
        });
        const clone = structuredClone(source);
        assert(clone.size === 1 && Array.from(clone.values())[0].y === 2);
        "#,
    );
}

#[cfg(feature = "loader")]
#[test]
fn private_fetch_and_sqlite_objects_stay_branded_after_visible_shape_changes() {
    use ibex2::{
        boundary::HostError,
        loader::{ModuleGrants, Root},
        stdlib::{
            abort::AbortSignal,
            app_fs::AppDirectories,
            fetch::{Headers, Request, Response, StreamingResponse, Transport},
        },
    };
    use std::{sync::Arc, time::Duration};

    struct StaticTransport;
    impl Transport for StaticTransport {
        fn open(
            &self,
            request: &Request,
            signal: &AbortSignal,
        ) -> Result<StreamingResponse, HostError> {
            Ok(Response {
                status: 200,
                status_text: "OK".into(),
                headers: Headers::new(),
                body: b"body".to_vec(),
                url: request.url.clone(),
                redirected: false,
            }
            .into_stream(request.body_limit(), signal.clone()))
        }
    }

    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "ibex2-structured-clone-platform-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let data = root.join("data");
    let cache = root.join("cache");
    let temporary = root.join("tmp");
    for path in [&data, &cache, &temporary] {
        std::fs::create_dir_all(path).unwrap();
    }
    std::fs::write(
        root.join("index.js"),
        r#"
        globalThis.platformReady = false;
        globalThis.platformError = "";
        (async function () {
          const response = await fetch("https://clone.example/value");
          const body = response.body;
          const reader = body.getReader();
          const database = await sqlite.open("app:/data/clone.db");
          const statement = await database.prepare("SELECT 1");
          globalThis.platformValues = { response, body, reader, database, statement };
          globalThis.platformReady = true;
        })().catch(function (error) { globalThis.platformError = String(error && error.stack || error); });
        "#,
    )
    .unwrap();

    let directories = AppDirectories::new(data, cache, temporary).unwrap();
    let host = ibex2::host::Host::with_transport(Box::new(StaticTransport))
        .with_app_directories(directories)
        .with_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider));
    let grants = ibex2::grant::GrantSet::parse(
        "net.fetch https://clone.example\nsqlite.open app:/data/clone.db\n",
    )
    .unwrap();
    let bindings = host.endow(grants);
    let context = ibex2::bindings::Context::from_bindings(&bindings);
    let mut runtime = Hermes::new(DynamicCode::Closed).expect("runtime");
    runtime
        .install_runtime(ibex2::bindings::Groups::DEFAULT, &context)
        .expect("bindings");
    runtime
        .set_loader(
            Root::Declared(root.clone()),
            ModuleGrants::parse(
                "[*]\nnet.fetch https://clone.example\nsqlite.open app:/data/clone.db\n",
            )
            .unwrap(),
        )
        .unwrap();
    runtime.harden().unwrap();
    runtime.run_entry("./index.js").unwrap();
    runtime.run_to_quiescence(Duration::from_secs(10));
    assert_eq!(
        runtime.eval("platformReady + ':' + platformError").unwrap(),
        "true:"
    );
    runtime
        .eval(
            r#"
            (function () {
              function assert(value, message) { if (!value) throw new Error(message); }
              function dataCloneError(value) {
                try { structuredClone(value); } catch (error) {
                  assert(error instanceof DOMException && error.name === "DataCloneError", "wrong error");
                  return;
                }
                throw new Error("expected DataCloneError");
              }
              for (const name of Object.keys(platformValues)) {
                const value = platformValues[name];
                Object.setPrototypeOf(value, null);
                Object.defineProperty(value, Symbol.toStringTag, { value: "Changed" });
                dataCloneError(value);
              }
            })()
            "#,
        )
        .unwrap();
    drop(runtime);
    std::fs::remove_dir_all(root).unwrap();
}
