//! Hardened-Hermes coverage for the `crypto.subtle` projection.
#![cfg(feature = "hermes")]

use ibex2::engine::hermes::{DynamicCode, Hermes};

fn runtime() -> Hermes {
    let mut runtime = Hermes::new(DynamicCode::Closed).unwrap();
    let context = ibex2::bindings::Context::new(ibex2::grant::GrantSet::none());
    runtime
        .install_runtime(ibex2::bindings::Groups::DEFAULT, &context)
        .unwrap();
    runtime.harden().unwrap();
    runtime
}

fn run_async(runtime: &mut Hermes, body: &str) {
    runtime
        .eval(&format!(
            r#"
            globalThis.__subtle_result = "pending";
            (async function() {{
              function assert(value, message) {{ if (!value) throw new Error(message || "assertion failed"); }}
              function hex(value) {{
                return Array.from(new Uint8Array(value), x => x.toString(16).padStart(2, "0")).join("");
              }}
              async function rejects(name, call) {{
                try {{ await call(); }} catch (error) {{ assert(error.name === name, error.name + ": " + error.message); return; }}
                throw new Error("expected " + name);
              }}
              {body}
            }})().then(
              () => {{ globalThis.__subtle_result = "ok"; }},
              error => {{ globalThis.__subtle_result = error.name + ": " + error.message; }}
            );
            "#
        ))
        .unwrap();
    runtime.drain_microtasks().unwrap();
    assert_eq!(runtime.eval("__subtle_result").unwrap(), "ok");
}

#[cfg(feature = "crypto")]
#[test]
fn digest_hmac_and_key_metadata_round_trip() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        assert(crypto.subtle instanceof SubtleCrypto);
        assert(Object.prototype.toString.call(crypto.subtle) === "[object SubtleCrypto]");
        const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode("abc"));
        assert(hex(digest) === "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

        const raw = new Uint8Array(20).fill(11);
        const key = await crypto.subtle.importKey(
          "raw", raw.subarray(0), {name: "HMAC", hash: "SHA-256"}, true, ["sign", "verify"]
        );
        assert(key instanceof CryptoKey && key.type === "secret" && key.extractable);
        assert(key.algorithm.name === "HMAC" && key.algorithm.hash.name === "SHA-256");
        assert(key.algorithm.length === 160 && key.usages.join(",") === "sign,verify");
        const signature = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode("Hi There"));
        assert(hex(signature) === "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
        assert(await crypto.subtle.verify("HMAC", key, signature, new TextEncoder().encode("Hi There")));
        assert(!(await crypto.subtle.verify("HMAC", key, signature, new Uint8Array([1]))));
        assert(hex(await crypto.subtle.exportKey("raw", key)) === hex(raw));
        const jwk = await crypto.subtle.exportKey("jwk", key);
        assert(jwk.kty === "oct" && jwk.alg === "HS256" && jwk.ext === true);
        const imported = await crypto.subtle.importKey("jwk", jwk, {name: "HMAC", hash: "SHA-256"}, true, ["verify"]);
        assert(imported.algorithm.length === 160 && imported.usages[0] === "verify");

        const verifyOnly = await crypto.subtle.importKey("raw", raw, {name: "HMAC", hash: "SHA-256"}, false, ["verify"]);
        await rejects("InvalidAccessError", () => crypto.subtle.sign("HMAC", verifyOnly, raw));
        await rejects("InvalidAccessError", () => crypto.subtle.exportKey("raw", verifyOnly));
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn key_formats_are_case_sensitive_enums() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const algorithm = {name: "HMAC", hash: "SHA-256"};
        const key = await crypto.subtle.importKey(
          "raw", new Uint8Array([1, 2, 3]), algorithm, true, ["sign"]
        );
        for (const format of ["RAW", "unknown"]) {
          await rejects("TypeError", () => crypto.subtle.importKey(
            format, new Uint8Array([1, 2, 3]), algorithm, true, ["sign"]
          ));
          await rejects("TypeError", () => crypto.subtle.exportKey(format, key));
        }
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn non_octet_hmac_generate_sign_and_export_round_trips() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const imported = await crypto.subtle.importKey(
          "raw", new Uint8Array(20).fill(255),
          {name: "HMAC", hash: "SHA-256", length: 155}, true, ["sign", "verify"]
        );
        assert(imported.algorithm.length === 155);
        const importedRaw = new Uint8Array(await crypto.subtle.exportKey("raw", imported));
        assert(importedRaw.length === 20 && importedRaw[19] === 224);
        const importedSignature = await crypto.subtle.sign("HMAC", imported, new TextEncoder().encode("ibex"));
        assert(hex(importedSignature) === "09dc61ba3ab858026005afe0e64a7e864f4be1256bedefce0e16d78dd5a571ae");
        assert(await crypto.subtle.verify("HMAC", imported, importedSignature, new TextEncoder().encode("ibex")));
        await rejects("DataError", () => crypto.subtle.importKey(
          "raw", new Uint8Array(32),
          {name: "HMAC", hash: "SHA-256", length: 128}, true, ["sign"]
        ));

        const generated = await crypto.subtle.generateKey(
          {name: "HMAC", hash: "SHA-256", length: 17}, true, ["sign", "verify"]
        );
        assert(generated.algorithm.length === 17);
        const raw = new Uint8Array(await crypto.subtle.exportKey("raw", generated));
        assert(raw.length === 3 && (raw[2] & 127) === 0);
        const signature = await crypto.subtle.sign("HMAC", generated, new Uint8Array([1, 2, 3]));
        assert(await crypto.subtle.verify("HMAC", generated, signature, new Uint8Array([1, 2, 3])));
        const roundTrip = await crypto.subtle.importKey(
          "raw", raw, {name: "HMAC", hash: "SHA-256", length: 17}, true, ["sign"]
        );
        assert(roundTrip.algorithm.length === 17);
        assert(hex(await crypto.subtle.sign("HMAC", roundTrip, new Uint8Array([1, 2, 3]))) === hex(signature));

        const hkdf = await crypto.subtle.importKey(
          "raw", new Uint8Array([1, 2, 3]), "HKDF", false, ["deriveKey"]
        );
        const derived = await crypto.subtle.deriveKey(
          {name: "HKDF", hash: "SHA-256", salt: new Uint8Array(), info: new Uint8Array()},
          hkdf, {name: "HMAC", hash: "SHA-256", length: 24}, true, ["sign"]
        );
        const derivedRaw = new Uint8Array(await crypto.subtle.exportKey("raw", derived));
        assert(derived.algorithm.length === 24 && derivedRaw.length === 3);
        await rejects("OperationError", () => crypto.subtle.deriveKey(
          {name: "HKDF", hash: "SHA-256", salt: new Uint8Array(), info: new Uint8Array()},
          hkdf, {name: "HMAC", hash: "SHA-256", length: 17}, true, ["sign"]
        ));
        await rejects("TypeError", () => crypto.subtle.deriveKey(
          {name: "HKDF", hash: "SHA-256", salt: new Uint8Array(), info: new Uint8Array()},
          hkdf, {name: "HMAC", hash: "SHA-256", length: 0}, true, ["sign"]
        ));
        await rejects("OperationError", () => crypto.subtle.generateKey(
          {name: "HMAC", hash: "SHA-256", length: 0}, true, ["sign"]
        ));
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn aes_gcm_round_trip_and_refusals() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const key = await crypto.subtle.generateKey({name: "AES-GCM", length: 256}, true, ["encrypt", "decrypt"]);
        const params = {name: "AES-GCM", iv: new Uint8Array(12), additionalData: new Uint8Array([1,2,3]), tagLength: 128};
        const plain = new TextEncoder().encode("ibex");
        const encrypted = await crypto.subtle.encrypt(params, key, plain);
        assert(new Uint8Array(encrypted).length === plain.length + 16);
        assert(new TextDecoder().decode(await crypto.subtle.decrypt(params, key, encrypted)) === "ibex");
        await rejects("OperationError", () => crypto.subtle.decrypt(
          {name: "AES-GCM", iv: new Uint8Array(12), additionalData: new Uint8Array([9]), tagLength: 128}, key, encrypted
        ));
        await rejects("OperationError", () => crypto.subtle.encrypt(
          {name: "AES-GCM", iv: new Uint8Array(8), tagLength: 128}, key, plain
        ));
        await rejects("OperationError", () => crypto.subtle.encrypt(
          {name: "AES-GCM", iv: new Uint8Array(12), tagLength: 96}, key, plain
        ));
        await rejects("NotSupportedError", () => crypto.subtle.generateKey(
          {name: "AES-GCM", length: 192}, true, ["encrypt"]
        ));
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn hkdf_and_pbkdf2_derive_bits_and_keys() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const ikm = new Uint8Array(22).fill(11);
        const hkdf = await crypto.subtle.importKey("raw", ikm, "HKDF", false, ["deriveBits", "deriveKey"]);
        const salt = new Uint8Array([0,1,2,3,4,5,6,7,8,9,10,11,12]);
        const info = new Uint8Array([240,241,242,243,244,245,246,247,248,249]);
        const hkdfBits = await crypto.subtle.deriveBits({name: "HKDF", hash: "SHA-256", salt, info}, hkdf, 336);
        assert(hex(hkdfBits) === "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865");
        await rejects("OperationError", () => crypto.subtle.deriveBits(
          {name: "HKDF", hash: "SHA-256", salt, info}, hkdf, null
        ));
        await rejects("OperationError", () => crypto.subtle.deriveBits(
          {name: "HKDF", hash: "SHA-256", salt, info}, hkdf
        ));
        await rejects("OperationError", () => crypto.subtle.deriveBits(
          {name: "HKDF", hash: "SHA-256", salt, info}, hkdf, undefined
        ));
        const derivedHmac = await crypto.subtle.deriveKey(
          {name: "HKDF", hash: "SHA-256", salt, info}, hkdf,
          {name: "HMAC", hash: "SHA-256", length: 256}, false, ["sign"]
        );
        assert(derivedHmac.algorithm.length === 256 && derivedHmac.usages[0] === "sign");

        const password = await crypto.subtle.importKey(
          "raw", new TextEncoder().encode("password"), "PBKDF2", false, ["deriveBits", "deriveKey"]
        );
        const pbkdf = {name: "PBKDF2", hash: "SHA-256", salt: new TextEncoder().encode("salt"), iterations: 1};
        assert(hex(await crypto.subtle.deriveBits(pbkdf, password, 256)) ===
          "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b");
        await rejects("OperationError", () => crypto.subtle.deriveBits(pbkdf, password, null));
        await rejects("OperationError", () => crypto.subtle.deriveBits(pbkdf, password));
        await rejects("OperationError", () => crypto.subtle.deriveBits(pbkdf, password, undefined));
        const aes = await crypto.subtle.deriveKey(pbkdf, password, {name: "AES-GCM", length: 128}, false, ["encrypt"]);
        assert(aes.algorithm.name === "AES-GCM" && aes.algorithm.length === 128 && !aes.extractable);
        await rejects("SyntaxError", () => crypto.subtle.importKey("raw", ikm, "HKDF", true, ["deriveBits"]));
        "#,
    );
}

#[cfg(feature = "crypto-asymmetric")]
#[test]
fn ecdsa_p256_key_pair_formats_and_stock_ring_hashes() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const pair = await crypto.subtle.generateKey(
          {name: "ECDSA", namedCurve: "P-256"}, false, ["sign", "verify"]
        );
        assert(pair.publicKey.type === "public" && pair.publicKey.extractable);
        assert(pair.privateKey.type === "private" && !pair.privateKey.extractable);
        assert(pair.publicKey.algorithm.name === "ECDSA" && pair.publicKey.algorithm.namedCurve === "P-256");
        assert(pair.publicKey.usages.join(",") === "verify" && pair.privateKey.usages.join(",") === "sign");
        const data = new TextEncoder().encode("ibex ecdsa");
        const signature256 = await crypto.subtle.sign(
          {name: "ECDSA", hash: "SHA-256"}, pair.privateKey, data
        );
        assert(new Uint8Array(signature256).length === 64);
        assert(await crypto.subtle.verify(
          {name: "ECDSA", hash: "SHA-256"}, pair.publicKey, signature256, data
        ));
        for (const hash of ["SHA-384", "SHA-512"]) {
          await rejects("NotSupportedError", () => crypto.subtle.sign(
            {name: "ECDSA", hash}, pair.privateKey, data
          ));
          await rejects("NotSupportedError", () => crypto.subtle.verify(
            {name: "ECDSA", hash}, pair.publicKey, signature256, data
          ));
        }
        await rejects("InvalidAccessError", () => crypto.subtle.exportKey("pkcs8", pair.privateKey));

        const exportedPair = await crypto.subtle.generateKey(
          {name: "ECDSA", namedCurve: "P-256"}, true, ["sign", "verify"]
        );
        const raw = await crypto.subtle.exportKey("raw", exportedPair.publicKey);
        const spki = await crypto.subtle.exportKey("spki", exportedPair.publicKey);
        const pkcs8 = await crypto.subtle.exportKey("pkcs8", exportedPair.privateKey);
        const publicJwk = await crypto.subtle.exportKey("jwk", exportedPair.publicKey);
        const privateJwk = await crypto.subtle.exportKey("jwk", exportedPair.privateKey);
        assert(new Uint8Array(raw)[0] === 4 && raw.byteLength === 65 && spki.byteLength === 91);
        assert(pkcs8.byteLength === 138 && publicJwk.kty === "EC" && publicJwk.crv === "P-256");
        assert(privateJwk.d && !publicJwk.d && privateJwk.alg === "ES256");
        const importedPublic = await crypto.subtle.importKey(
          "spki", spki, {name: "ECDSA", namedCurve: "P-256"}, true, ["verify"]
        );
        const importedPrivate = await crypto.subtle.importKey(
          "jwk", privateJwk, {name: "ECDSA", namedCurve: "P-256"}, true, ["sign"]
        );
        await rejects("DataError", () => crypto.subtle.importKey(
          "jwk", Object.assign({}, privateJwk, {key_ops: ["sign", "sign"]}),
          {name: "ECDSA", namedCurve: "P-256"}, true, ["sign"]
        ));
        const privateWithPublicOp = await crypto.subtle.importKey(
          "jwk", Object.assign({}, privateJwk, {key_ops: ["sign", "verify"]}),
          {name: "ECDSA", namedCurve: "P-256"}, true, ["sign"]
        );
        const signature = await crypto.subtle.sign({name: "ECDSA", hash: "SHA-256"}, importedPrivate, data);
        assert(await crypto.subtle.verify({name: "ECDSA", hash: "SHA-256"}, importedPublic, signature, data));
        assert((await crypto.subtle.sign(
          {name: "ECDSA", hash: "SHA-256"}, privateWithPublicOp, data
        )).byteLength === 64);
        await rejects("DataError", () => crypto.subtle.importKey(
          "raw", new Uint8Array(33).fill(2), {name: "ECDSA", namedCurve: "P-256"}, true, ["verify"]
        ));
        "#,
    );
}

#[cfg(feature = "crypto-asymmetric")]
#[test]
fn asymmetric_generation_requires_a_private_key_usage() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        for (const algorithm of [
          {name: "ECDSA", namedCurve: "P-256"},
          {name: "Ed25519"}
        ]) {
          await rejects("SyntaxError", () => crypto.subtle.generateKey(algorithm, true, ["verify"]));
          await rejects("SyntaxError", () => crypto.subtle.generateKey(algorithm, true, []));
        }
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn hostile_integer_sizes_are_rejected_by_name() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        for (const length of [NaN, Infinity, -1, 4294967296, 2 ** 53]) {
          await rejects("TypeError", () => crypto.subtle.generateKey(
            {name: "HMAC", hash: "SHA-256", length}, true, ["sign"]
          ));
        }
        const rounded = await crypto.subtle.generateKey(
          {name: "HMAC", hash: "SHA-256", length: 16.9}, true, ["sign"]
        );
        assert(rounded.algorithm.length === 16);
        assert(new Uint8Array(await crypto.subtle.exportKey("raw", rounded)).length === 2);
        await rejects("OperationError", () => crypto.subtle.generateKey(
          {name: "HMAC", hash: "SHA-256", length: 1000008}, true, ["sign"]
        ));
        await rejects("OperationError", () => crypto.subtle.importKey(
          "raw", new Uint8Array(125001), {name: "HMAC", hash: "SHA-256"}, true, ["sign"]
        ));

        const ikm = new Uint8Array(32);
        const hkdf = await crypto.subtle.importKey(
          "raw", ikm, "HKDF", false, ["deriveBits", "deriveKey"]
        );
        const hkdfParams = {
          name: "HKDF", hash: "SHA-256", salt: new Uint8Array(), info: new Uint8Array()
        };
        for (const length of [NaN, Infinity, -Infinity, -1, 4294967296, 2 ** 53]) {
          await rejects("TypeError", () => crypto.subtle.deriveBits(hkdfParams, hkdf, length));
        }
        await rejects("TypeError", () => crypto.subtle.deriveBits(hkdfParams, hkdf, 1n));
        await rejects("TypeError", () => crypto.subtle.deriveBits(hkdfParams, hkdf, Symbol()));
        await rejects("OperationError", () => crypto.subtle.deriveBits(hkdfParams, hkdf, 65288));
        await rejects("OperationError", () => crypto.subtle.deriveKey(
          hkdfParams, hkdf,
          {name: "HMAC", hash: "SHA-256", length: 1000008}, true, ["sign"]
        ));

        const password = await crypto.subtle.importKey(
          "raw", new Uint8Array([1]), "PBKDF2", false, ["deriveBits"]
        );
        const pbkdf = iterations => ({
          name: "PBKDF2", hash: "SHA-256", salt: new Uint8Array(), iterations
        });
        await rejects("TypeError", () => crypto.subtle.deriveBits(pbkdf(4294967296), password, 8));
        await rejects("OperationError", () => crypto.subtle.deriveBits(pbkdf(0), password, 8));
        await rejects("OperationError", () => crypto.subtle.deriveBits(pbkdf(1000001), password, 8));
        await rejects("OperationError", () => crypto.subtle.deriveBits(pbkdf(1), password, 1000008));

        const deriveKeyOnly = await crypto.subtle.importKey(
          "raw", ikm, "HKDF", false, ["deriveKey"]
        );
        await rejects("InvalidAccessError", () => crypto.subtle.deriveBits(
          hkdfParams, deriveKeyOnly, null
        ));
        await rejects("InvalidAccessError", () => crypto.subtle.deriveBits(
          hkdfParams, deriveKeyOnly
        ));
        await rejects("InvalidAccessError", () => crypto.subtle.deriveBits(
          hkdfParams, deriveKeyOnly, undefined
        ));
        await rejects("InvalidAccessError", () => crypto.subtle.deriveBits(
          hkdfParams, password, null
        ));
        "#,
    );
}

#[cfg(feature = "crypto-asymmetric")]
#[test]
fn ed25519_known_answer_and_format_round_trips() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const seed = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        const publicHex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        const expected = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155" +
          "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";
        function bytes(value) {
          return new Uint8Array(value.match(/../g).map(x => parseInt(x, 16)));
        }
        const pkcs8 = new Uint8Array([48,46,2,1,0,48,5,6,3,43,101,112,4,34,4,32, ...bytes(seed)]);
        const spki = new Uint8Array([48,42,48,5,6,3,43,101,112,3,33,0, ...bytes(publicHex)]);
        const privateKey = await crypto.subtle.importKey("pkcs8", pkcs8, "Ed25519", true, ["sign"]);
        const publicKey = await crypto.subtle.importKey("spki", spki, "Ed25519", true, ["verify"]);
        const signature = await crypto.subtle.sign("Ed25519", privateKey, new Uint8Array());
        assert(hex(signature) === expected);
        assert(await crypto.subtle.verify("Ed25519", publicKey, signature, new Uint8Array()));
        assert(hex(await crypto.subtle.exportKey("raw", publicKey)) === publicHex);
        assert(hex(await crypto.subtle.exportKey("pkcs8", privateKey)) === hex(pkcs8));
        assert(hex(await crypto.subtle.exportKey("spki", publicKey)) === hex(spki));
        const jwk = await crypto.subtle.exportKey("jwk", privateKey);
        assert(jwk.kty === "OKP" && jwk.crv === "Ed25519" && jwk.alg === "Ed25519" && jwk.d);
        await rejects("DataError", () => crypto.subtle.importKey(
          "jwk", Object.assign({}, jwk, {key_ops: ["sign", "sign"]}),
          "Ed25519", true, ["sign"]
        ));
        const imported = await crypto.subtle.importKey("jwk", jwk, "Ed25519", true, ["sign"]);
        assert(hex(await crypto.subtle.sign("Ed25519", imported, new Uint8Array())) === expected);

        const pair = await crypto.subtle.generateKey("Ed25519", false, ["sign", "verify"]);
        assert(pair.publicKey.extractable && !pair.privateKey.extractable);
        const roundTrip = await crypto.subtle.sign("Ed25519", pair.privateKey, new Uint8Array([1,2,3]));
        assert(await crypto.subtle.verify("Ed25519", pair.publicKey, roundTrip, new Uint8Array([1,2,3])));
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn hmac_generation_does_not_inherit_get_random_values_quota() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const key = await crypto.subtle.generateKey(
          {name: "HMAC", hash: "SHA-256", length: 600000}, true, ["sign"]
        );
        assert(key.algorithm.length === 600000);
        assert((await crypto.subtle.exportKey("raw", key)).byteLength === 75000);
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn import_rejections_preflight_borrowed_material_and_jwk_fields() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        await rejects("DataError", () => crypto.subtle.importKey(
          "raw", new Uint8Array(1024 * 1024), "AES-GCM", true, ["encrypt"]
        ));
        await rejects("SyntaxError", () => crypto.subtle.importKey(
          "raw", new Uint8Array(1024 * 1024), "HKDF", true, ["deriveBits"]
        ));
        await rejects("DataError", () => crypto.subtle.importKey(
          "jwk", {kty: "x".repeat(65), k: "AA"},
          {name: "HMAC", hash: "SHA-256"}, true, ["sign"]
        ));
        await rejects("OperationError", () => crypto.subtle.importKey(
          "jwk", {kty: "oct", k: "A".repeat(166668)},
          {name: "HMAC", hash: "SHA-256"}, true, ["sign"]
        ));
        await rejects("DataError", () => crypto.subtle.importKey(
          "jwk", {kty: "oct", k: "AA", key_ops: ["s".repeat(129)]},
          {name: "HMAC", hash: "SHA-256"}, true, ["sign"]
        ));
        await rejects("DataError", () => crypto.subtle.importKey(
          "jwk", {kty: "oct", k: "AA", key_ops: ["sign,verify"]},
          {name: "HMAC", hash: "SHA-256"}, true, ["sign", "verify"]
        ));
        await rejects("DataError", () => crypto.subtle.importKey(
          "jwk", {kty: "oct", k: "AA", key_ops: ["sign", "sign"]},
          {name: "HMAC", hash: "SHA-256"}, true, ["sign"]
        ));
        "#,
    );
}

#[cfg(feature = "crypto-asymmetric")]
#[test]
fn asymmetric_imports_preflight_der_and_jwk_component_sizes() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        await rejects("DataError", () => crypto.subtle.importKey(
          "spki", new Uint8Array(1024 * 1024),
          {name: "ECDSA", namedCurve: "P-256"}, true, ["verify"]
        ));
        await rejects("DataError", () => crypto.subtle.importKey(
          "jwk", {kty: "EC", crv: "P-256", x: "A".repeat(44), y: "A".repeat(43)},
          {name: "ECDSA", namedCurve: "P-256"}, true, ["verify"]
        ));
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn out_of_scope_algorithms_and_formats_are_named_refusals() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        const bytes = new Uint8Array(32);
        for (const name of ["SHA-1", "MD5"]) {
          await rejects("NotSupportedError", () => crypto.subtle.digest(name, bytes));
        }
        for (const name of ["RSA-PSS", "RSA-OAEP", "AES-CBC", "AES-CTR", "X25519", "ECDH"]) {
          await rejects("NotSupportedError", () => crypto.subtle.generateKey({name, length: 128}, true, ["encrypt"]));
        }
        for (const namedCurve of ["P-384", "P-521"]) {
          await rejects("NotSupportedError", () => crypto.subtle.generateKey(
            {name: "ECDSA", namedCurve}, true, ["sign"]
          ));
        }
        await rejects("NotSupportedError", () => crypto.subtle.importKey("pkcs8", bytes, {name: "HMAC", hash: "SHA-256"}, true, ["sign"]));
        await rejects("NotSupportedError", () => crypto.subtle.importKey("spki", bytes, {name: "HMAC", hash: "SHA-256"}, true, ["verify"]));
        await rejects("NotSupportedError", () => crypto.subtle.wrapKey());
        await rejects("NotSupportedError", () => crypto.subtle.unwrapKey());
        "#,
    );
}

#[cfg(all(feature = "crypto", not(feature = "crypto-asymmetric")))]
#[test]
fn asymmetric_feature_off_refuses_by_name() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        await rejects("NotSupportedError", () => crypto.subtle.generateKey(
          {name: "ECDSA", namedCurve: "P-256"}, true, ["sign"]
        ));
        try {
          await crypto.subtle.generateKey("Ed25519", true, ["sign"]);
          throw new Error("expected Ed25519 refusal");
        } catch (error) {
          assert(error.name === "NotSupportedError");
          assert(error.message.indexOf("omitted") >= 0);
        }
        "#,
    );
}

#[cfg(feature = "crypto")]
#[test]
fn garbage_collection_releases_rust_key_handles() {
    let mut runtime = runtime();
    runtime
        .eval(
            r#"
            globalThis.__held_key = null;
            globalThis.__key_ready = "pending";
            void crypto.subtle.generateKey(
              {name: "HMAC", hash: "SHA-256"}, false, ["sign"]
            ).then(
              key => {
                globalThis.__held_key = key;
                globalThis.__key_ready = "ok";
              },
              error => {
                globalThis.__key_ready = error.name + ": " + error.message;
              }
            );
            "#,
        )
        .unwrap();
    runtime.drain_microtasks().unwrap();
    assert_eq!(runtime.eval("__key_ready").unwrap(), "ok");
    assert_eq!(runtime.crypto_key_count(), 1);
    // Avoid the general async-test wrapper here: its suspended async activation
    // can retain the last `await` result in a Hermes register even after the
    // global is cleared. Hermes also scans just-finished Promise-reaction
    // registers conservatively, so advance through fresh entrances before each
    // bounded full collection. A real NativeState leak remains nonzero through
    // every iteration and still fails the exact assertion below.
    runtime
        .eval("globalThis.__held_key = null; globalThis.__key_ready = null; void 0")
        .unwrap();
    for _ in 0..8 {
        runtime.eval("void 0").unwrap();
        assert!(runtime.collect_garbage());
        if runtime.crypto_key_count() == 0 {
            break;
        }
    }
    assert_eq!(runtime.crypto_key_count(), 0);
}

#[cfg(not(feature = "crypto"))]
#[test]
fn feature_off_keeps_the_binding_and_rejects_by_name() {
    let mut runtime = runtime();
    run_async(
        &mut runtime,
        r#"
        assert(crypto.subtle instanceof SubtleCrypto);
        await rejects("NotSupportedError", () => crypto.subtle.digest("SHA-256", new Uint8Array()));
        "#,
    );
}
