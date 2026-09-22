//! Locale-sensitive String case mapping on the Linux vanilla-Hermes profile.
#![cfg(all(feature = "hermes", target_os = "linux"))]

use ibex2::engine::hermes::{DynamicCode, Hermes};

fn runtime(hardened: bool) -> Hermes {
    let mut runtime = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(runtime.install_stdlib());
    runtime.install_bindings().expect("bindings");
    if hardened {
        runtime.harden().expect("harden");
    }
    runtime
}

fn eval(runtime: &mut Hermes, source: &str) -> String {
    runtime
        .eval(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn maps_locale_families_and_uses_only_the_first_requested_locale() {
    let mut runtime = runtime(false);
    assert_eq!(eval(&mut runtime, "'I'.toLocaleLowerCase('tr')"), "ı");
    assert_eq!(eval(&mut runtime, "'i'.toLocaleUpperCase('tr')"), "İ");
    assert_eq!(
        eval(&mut runtime, "'straße'.toLocaleUpperCase('de-DE')"),
        "STRASSE"
    );
    assert_eq!(
        eval(&mut runtime, "'I'.toLocaleLowerCase(['zz-ZZ', 'tr'])"),
        "i"
    );
}

#[test]
fn preserves_receiver_then_locale_coercion_and_errors() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var order = [];
              var receiver = { toString: function () { order.push('receiver'); return 'I'; } };
              var locales = {
                get length() { order.push('length'); return 1; },
                get 0() { order.push('locale'); return 'tr'; }
              };
              var value = String.prototype.toLocaleLowerCase.call(receiver, locales);
              return order.join(',') + ':' + value;
            })()"#,
        ),
        "receiver,length,locale:ı"
    );
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var touched = false;
              var receiver = { toString: function () { throw new Error('receiver'); } };
              var locales = { get length() { touched = true; return 0; } };
              try { String.prototype.toLocaleLowerCase.call(receiver, locales); }
              catch (error) { return error.message + ':' + String(touched); }
              return 'accepted';
            })()"#,
        ),
        "receiver:false"
    );
    assert_eq!(
        eval(
            &mut runtime,
            "(function () { try { String.prototype.toLocaleLowerCase.call(Symbol('x')); } catch (error) { return error.name; } return 'accepted'; })()"
        ),
        "TypeError"
    );
}

#[test]
fn preserves_utf16_code_units_and_validates_empty_string_locales() {
    let mut runtime = runtime(false);
    assert_eq!(eval(&mut runtime, "''.toLocaleLowerCase('en-US')"), "");
    assert_eq!(eval(&mut runtime, "''.toLocaleUpperCase('en-US')"), "");
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var value = '\ud800A\0\ud83d\ude00\udc00'.toLocaleLowerCase('en-US');
              var units = [];
              for (var i = 0; i < value.length; ++i) units.push(value.charCodeAt(i).toString(16));
              return units.join(',');
            })()"#,
        ),
        "d800,61,0,d83d,de00,dc00"
    );
    assert_eq!(
        eval(
            &mut runtime,
            "(function () { try { ''.toLocaleLowerCase('not_a_locale'); } catch (error) { return error.name; } return 'accepted'; })()"
        ),
        "RangeError"
    );
}

#[test]
fn hardening_keeps_methods_and_removes_the_private_binding() {
    let mut runtime = runtime(true);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var lower = Object.getOwnPropertyDescriptor(String.prototype, 'toLocaleLowerCase');
              return [
                typeof globalThis.__ibex2_intl_case,
                Object.isFrozen(String.prototype),
                lower.enumerable,
                lower.writable,
                lower.configurable,
                'I'.toLocaleLowerCase('tr')
              ].join(':');
            })()"#,
        ),
        "undefined:true:false:false:false:ı"
    );
}
