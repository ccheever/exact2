//! OS randomness from Rust with no engine, and the same functions through JS.
use ibex2::stdlib::crypto::{get_random_values, random_uuid, MAX_RANDOM_BYTES};

#[test]
fn rust_consumers_fill_only_the_selected_bytes() {
    let mut bytes = [0xa5; 128];
    get_random_values(&mut bytes[16..112]).unwrap();
    assert_eq!(&bytes[..16], &[0xa5; 16]);
    assert_eq!(&bytes[112..], &[0xa5; 16]);
    assert_ne!(&bytes[16..112], &[0xa5; 96]);
    let before = bytes;
    get_random_values(&mut bytes[16..112]).unwrap();
    assert_ne!(bytes, before);
}

#[test]
fn quota_is_checked_before_mutation_and_empty_is_valid() {
    get_random_values(&mut []).unwrap();
    let mut bytes = vec![0xa5; MAX_RANDOM_BYTES + 1];
    let err = get_random_values(&mut bytes).unwrap_err();
    assert!(err.to_string().starts_with("QuotaExceededError:"));
    assert!(bytes.iter().all(|b| *b == 0xa5));
    get_random_values(&mut bytes[..MAX_RANDOM_BYTES]).unwrap();
    assert_eq!(bytes[MAX_RANDOM_BYTES], 0xa5);
}

#[test]
fn rust_uuids_have_the_web_format_and_fresh_randomness() {
    let mut seen = std::collections::HashSet::new();
    for _ in 0..64 {
        let uuid = random_uuid().unwrap();
        assert_eq!(uuid.len(), 36);
        for (i, byte) in uuid.bytes().enumerate() {
            if matches!(i, 8 | 13 | 18 | 23) {
                assert_eq!(byte, b'-');
            } else {
                assert!(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
            }
        }
        assert_eq!(uuid.as_bytes()[14], b'4');
        assert!(b"89ab".contains(&uuid.as_bytes()[19]));
        assert!(seen.insert(uuid));
    }
}

#[test]
fn raw_boundary_refuses_invalid_spans_and_enforces_the_quota() {
    use ibex2::boundary_abi::{
        ibex2_host_call, ibex2_host_release, AbiValue, Op, TAG_BYTES, TAG_NUMBER,
    };
    let mut bytes = vec![0xa5; MAX_RANDOM_BYTES + 2];
    let buffer = AbiValue {
        tag: TAG_BYTES,
        number: 0.0,
        data: bytes.as_mut_ptr(),
        len: bytes.len(),
    };
    let number = |value| AbiValue {
        tag: TAG_NUMBER,
        number: value,
        data: std::ptr::null(),
        len: 0,
    };
    let call = |args: &[AbiValue]| {
        let mut out = number(0.0);
        // All spans point into `bytes`, which is live and not borrowed during
        // the call. No engine or runtime queue is needed by this operation.
        let status = unsafe {
            ibex2_host_call(
                std::ptr::null(),
                Op::CryptoGetRandomValues as u32,
                args.as_ptr(),
                args.len(),
                &mut out,
            )
        };
        unsafe { ibex2_host_release(&mut out) };
        status
    };
    for (offset, length) in [
        (-1.0, 1.0),
        (0.5, 1.0),
        (0.0, f64::NAN),
        (f64::INFINITY, 0.0),
        (0.0, 65_537.0),
        (65_538.0, 1.0),
        (f64::MAX, 0.0),
        (0.0, -1.0),
    ] {
        assert_ne!(call(&[buffer, number(offset), number(length)]), 0);
        assert!(bytes.iter().all(|b| *b == 0xa5));
    }
    assert_ne!(call(&[]), 0);
    assert_ne!(call(&[number(3.0), number(0.0), number(0.0)]), 0);
    assert_eq!(call(&[buffer, number(1.0), number(65_536.0)]), 0);
    assert_eq!(bytes[0], 0xa5);
    assert_eq!(bytes[MAX_RANDOM_BYTES + 1], 0xa5);
    let empty = AbiValue {
        len: 0,
        data: std::ptr::null(),
        ..buffer
    };
    assert_eq!(call(&[empty, number(0.0), number(0.0)]), 0);
}

#[cfg(feature = "hermes")]
mod javascript {
    use ibex2::engine::hermes::{DynamicCode, Hermes};

    fn check(body: &str) {
        let mut rt = Hermes::new(DynamicCode::Closed).unwrap();
        assert!(rt.install_stdlib());
        rt.install_bindings().unwrap();
        rt.harden().unwrap();
        rt.eval(&format!(
            r#"(function () {{
            function assert(value) {{ if (!value) throw new Error('assertion failed'); }}
            function throws(name, fn, code) {{
                try {{ fn(); }} catch (error) {{
                    assert(error.name === name);
                    if (name === 'TypeError' || name === 'RangeError') assert(error instanceof globalThis[name]);
                    else {{ assert(error instanceof DOMException); assert(error.code === code); }}
                    return;
                }}
                throw new Error('expected ' + name);
            }}
            {body}
        }})()"#
        ))
        .unwrap();
    }

    #[test]
    fn all_integer_views_return_the_same_object_and_preserve_neighboring_bytes() {
        check(
            r#"
            const names = ['Int8Array', 'Uint8Array', 'Uint8ClampedArray',
              'Int16Array', 'Uint16Array', 'Int32Array', 'Uint32Array',
              'BigInt64Array', 'BigUint64Array'];
            for (const name of names) {
                const Type = globalThis[name];
                if (!Type) continue;
                const bytes = new Uint8Array(256).fill(165);
                const view = new Type(bytes.buffer, 16, 128 / Type.BYTES_PER_ELEMENT);
                assert(crypto.getRandomValues(view) === view);
                assert(bytes.slice(0, 16).every(x => x === 165));
                assert(bytes.slice(144).every(x => x === 165));
                assert(bytes.slice(16, 144).some(x => x !== 165));
                const empty = new Type(bytes.buffer, 256, 0);
                assert(crypto.getRandomValues(empty) === empty);
            }
        "#,
        );
    }

    #[test]
    fn webcrypto_type_quota_and_receiver_errors() {
        check(
            r#"
            for (const value of [undefined, null, 1, 'x', [], {}, new ArrayBuffer(1),
                Object.create(Uint8Array.prototype), new Proxy(new Uint8Array(1), {})]) {
                throws('TypeError', () => crypto.getRandomValues(value));
            }
            for (const value of [new Float32Array(1), new Float64Array(1),
                new DataView(new ArrayBuffer(1)), new Float64Array(10000)]) {
                throws('TypeMismatchError', () => crypto.getRandomValues(value), 17);
            }
            const over = new Uint32Array(16385).fill(165);
            throws('QuotaExceededError', () => crypto.getRandomValues(over), 22);
            assert(over.every(x => x === 165));
            crypto.getRandomValues(new Uint32Array(16384));
            throws('TypeError', () => new Crypto());
            throws('TypeError', () => crypto.randomUUID.call({}));
            throws('TypeError', () => crypto.getRandomValues.call({}, new Uint8Array(1)));
            throws('TypeError', () => crypto.getRandomValues());
            if (typeof SharedArrayBuffer !== 'undefined') {
                throws('TypeError', () => crypto.getRandomValues(new Uint8Array(new SharedArrayBuffer(8))));
            }
        "#,
        );
    }

    #[test]
    fn shadowed_view_properties_cannot_redirect_the_native_write() {
        check(
            r#"
            const bytes = new Uint8Array(256).fill(165);
            const view = new Uint8Array(bytes.buffer, 32, 128);
            for (const key of ['buffer', 'byteOffset', 'byteLength']) {
                Object.defineProperty(view, key, {get() { throw new Error('shadow getter ran'); }});
            }
            Object.defineProperty(view, Symbol.toStringTag, {value: 'Float64Array'});
            assert(crypto.getRandomValues(view) === view);
            assert(bytes.slice(0, 32).every(x => x === 165));
            assert(bytes.slice(160).every(x => x === 165));
            assert(bytes.slice(32, 160).some(x => x !== 165));
            const floats = new Float64Array(1);
            Object.defineProperty(floats, Symbol.toStringTag, {value: 'Uint8Array'});
            throws('TypeMismatchError', () => crypto.getRandomValues(floats), 17);
        "#,
        );
    }

    #[test]
    fn uuid_is_available_after_hardening_without_exposing_raw_ops() {
        check(
            r#"
            assert(crypto instanceof Crypto);
            assert(Object.prototype.toString.call(crypto) === '[object Crypto]');
            assert(Object.isFrozen(Crypto.prototype));
            assert(Object.isFrozen(crypto));
            assert(!('__ibex2_random_uuid' in globalThis));
            assert(!('__ibex2_get_random_values' in globalThis));
            const seen = new Set();
            for (let i = 0; i < 64; i++) {
                const value = crypto.randomUUID();
                assert(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value));
                assert(!seen.has(value));
                seen.add(value);
            }
        "#,
        );
    }

    #[test]
    fn quota_exception_preserves_webidl_options_and_validation() {
        check(
            r#"
            const empty = new QuotaExceededError();
            assert(empty.name === 'QuotaExceededError' && empty.code === 22);
            assert(empty.quota === null && empty.requested === null);
            assert(empty instanceof DOMException);
            const e = new QuotaExceededError('full', {quota: '10', requested: 12});
            assert(e.quota === 10 && e.requested === 12 && e.message === 'full');
            assert(Object.prototype.toString.call(e) === '[object QuotaExceededError]');
            assert(new QuotaExceededError('', null).quota === null);
            assert(new QuotaExceededError('', {quota: null}).quota === 0);
            for (const value of [NaN, Infinity, -Infinity, Symbol()]) {
                throws('TypeError', () => new QuotaExceededError('', {quota: value}));
            }
            if (typeof BigInt === 'function') {
                throws('TypeError', () => new QuotaExceededError('', {quota: BigInt(1)}));
            }
            throws('TypeError', () => new QuotaExceededError('', 3));
            throws('RangeError', () => new QuotaExceededError('', {quota: -1}));
            throws('RangeError', () => new QuotaExceededError('', {quota: 10, requested: 9}));
            throws('TypeError', () => QuotaExceededError());
            const getter = Object.getOwnPropertyDescriptor(QuotaExceededError.prototype, 'quota').get;
            throws('TypeError', () => getter.call(new DOMException()));
        "#,
        );
    }

    #[test]
    fn dom_exceptions_have_web_names_codes_and_error_inheritance() {
        check(
            r#"
            const plain = new DOMException();
            assert(plain.name === 'Error' && plain.message === '' && plain.code === 0);
            const e = new DOMException('too many bytes', 'QuotaExceededError');
            assert(e instanceof Error && e instanceof DOMException);
            assert(e.toString() === 'QuotaExceededError: too many bytes');
            assert(e.code === DOMException.QUOTA_EXCEEDED_ERR && e.code === 22);
            assert(e.QUOTA_EXCEEDED_ERR === 22);
            assert(Object.prototype.toString.call(e) === '[object DOMException]');
            assert(new DOMException('', 'OperationError').code === 0);
            for (const name of ['DOMStringSizeError', 'NoDataAllowedError', 'ValidationError']) {
                assert(new DOMException('', name).code === 0);
            }
            const text = {toString() { return 'string hint'; }, valueOf() { return 42; }};
            assert(new DOMException(text).message === 'string hint');
            throws('TypeError', () => new DOMException(Symbol()));
            throws('TypeError', () => DOMException());
            const getter = Object.getOwnPropertyDescriptor(DOMException.prototype, 'code').get;
            throws('TypeError', () => getter.call({}));
        "#,
        );
    }
}
