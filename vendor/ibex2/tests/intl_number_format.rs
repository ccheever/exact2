//! Native ICU-backed NumberFormat behavior on the Linux vanilla-Hermes profile.
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
fn formats_locales_styles_precision_and_numbering_systems() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              function f(locale, options, value) {
                return new Intl.NumberFormat(locale, options).format(value);
              }
              var grouped = f('de-DE', {}, 1234.5);
              var currency = f('en-US', {style:'currency', currency:'USD'}, 1234.5);
              var percent = f('en-US', {style:'percent'}, 0.56);
              var unit = f('en-US', {style:'unit', unit:'kilometer-per-hour'}, 80);
              var scientific = f('en-US', {notation:'scientific'}, 12345);
              var engineering = f('en-US', {notation:'engineering'}, 12345);
              var compact = f('en-US', {notation:'compact'}, 1200);
              var significant = f('en-US', {minimumSignificantDigits:4, maximumSignificantDigits:4}, 12.3);
              var arab = f('ar-EG', {numberingSystem:'arab', useGrouping:false}, 123);
              return [
                grouped.indexOf('1.234') >= 0 && grouped.indexOf(',5') >= 0,
                currency.indexOf('$') >= 0 && currency.indexOf('1,234.50') >= 0,
                percent === '56%',
                unit.indexOf('80') >= 0 && /km|kilometer/.test(unit),
                /E4/.test(scientific), /E3/.test(engineering), compact === '1.2K',
                significant.indexOf('12.30') >= 0,
                arab !== '123' && arab.length === 3
              ].join(':');
            })()"#,
        ),
        "true:true:true:true:true:true:true:true:true"
    );
}

#[test]
fn parts_join_format_and_classify_special_values_and_signs() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var nf = new Intl.NumberFormat('en-US', {
                style:'currency', currency:'USD', signDisplay:'always'
              });
              function check(value, expected) {
                var parts = nf.formatToParts(value);
                var joined = parts.map(function (part) { return part.value; }).join('');
                var types = parts.map(function (part) { return part.type; });
                return joined === nf.format(value) && expected.every(function (type) {
                  return types.indexOf(type) >= 0;
                });
              }
              var decimal = new Intl.NumberFormat('en-US', {minimumFractionDigits:2});
              var fractionParts = decimal.formatToParts(1234.5);
              var fractionTypes = fractionParts.map(function (part) { return part.type; });
              return [
                check(12, ['plusSign','currency','integer']),
                check(-0, ['minusSign','currency','integer']),
                check(NaN, ['plusSign','currency','nan']),
                check(Infinity, ['plusSign','currency','infinity']),
                fractionParts.map(function (part) { return part.value; }).join('') === decimal.format(1234.5),
                ['integer','group','decimal','fraction'].every(function (type) {
                  return fractionTypes.indexOf(type) >= 0;
                })
              ].join(':');
            })()"#,
        ),
        "true:true:true:true:true:true"
    );
}

#[test]
fn normalizes_nan_sign_and_preserves_real_signed_values() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              function show(options, value) {
                var nf = new Intl.NumberFormat('en-US', options);
                return nf.format(value) + '=' + nf.formatToParts(value).map(function (part) {
                  return part.type + '(' + part.value + ')';
                }).join(',');
              }
              return [
                show({}, NaN), show({signDisplay:'always'}, NaN),
                show({style:'currency',currency:'USD'}, NaN),
                show({style:'currency',currency:'USD',signDisplay:'always'}, NaN),
                show({}, -0), show({}, 0), show({signDisplay:'always'}, 0)
              ].join('|');
            })()"#,
        ),
        concat!(
            "NaN=nan(NaN)|+NaN=plusSign(+),nan(NaN)|",
            "$NaN=currency($),nan(NaN)|+$NaN=plusSign(+),currency($),nan(NaN)|",
            "-0=minusSign(-),integer(0)|0=integer(0)|+0=plusSign(+),integer(0)"
        )
    );
}

#[test]
fn sign_display_matrix_includes_special_and_rounded_zero_values() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var values = [NaN, -0, 0, -1, 1, -Infinity, Infinity, -0.001, 0.001];
              return ['auto','never','always','exceptZero'].map(function (mode) {
                var nf = new Intl.NumberFormat('en-US', {
                  signDisplay:mode, maximumFractionDigits:0, useGrouping:false
                });
                return mode + '=' + values.map(nf.format).join('|');
              }).join(';');
            })()"#,
        ),
        concat!(
            "auto=NaN|-0|0|-1|1|-∞|∞|-0|0;",
            "never=NaN|0|0|1|1|∞|∞|0|0;",
            "always=+NaN|-0|+0|-1|+1|-∞|+∞|-0|+0;",
            "exceptZero=NaN|0|0|-1|+1|-∞|+∞|0|0"
        )
    );
}

#[test]
fn resolved_options_and_locale_negotiation_match_selected_configuration() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var same = new Intl.NumberFormat('de-u-nu-arab', {numberingSystem:'arab'}).resolvedOptions();
              var changed = new Intl.NumberFormat('de-u-nu-arab', {numberingSystem:'latn'}).resolvedOptions();
              var privateUse = new Intl.NumberFormat('en-x-u-nu-arab').resolvedOptions();
              var upperOption = new Intl.NumberFormat('en', {numberingSystem:'ARAB'}).resolvedOptions();
              var upperSame = new Intl.NumberFormat('de-u-nu-arab', {numberingSystem:'ARAB'}).resolvedOptions();
              var unsupportedOverride = new Intl.NumberFormat(
                'de-u-nu-arab', {numberingSystem:'foobar'}).resolvedOptions();
              var unsupportedType = new Intl.NumberFormat('en-u-nu-arab-foobar').resolvedOptions();
              var currency = new Intl.NumberFormat(['zz-ZZ','de-DE'], {
                style:'currency', currency:'KWD', currencyDisplay:'code',
                currencySign:'accounting', minimumIntegerDigits:2,
                minimumFractionDigits:3, maximumFractionDigits:4,
                useGrouping:false, signDisplay:'exceptZero'
              }).resolvedOptions();
              var supported = Intl.NumberFormat.supportedLocalesOf(['de-DE','zz-ZZ']);
              return [
                same.locale.indexOf('-u-nu-arab') >= 0, same.numberingSystem === 'arab',
                changed.locale.indexOf('-u-nu-') < 0, changed.numberingSystem === 'latn',
                privateUse.numberingSystem === 'latn' && privateUse.locale.indexOf('-u-nu-') < 0,
                upperOption.numberingSystem === 'arab',
                upperSame.numberingSystem === 'arab' && upperSame.locale.indexOf('-u-nu-arab') >= 0,
                unsupportedOverride.numberingSystem === 'arab',
                unsupportedOverride.locale.indexOf('-u-nu-arab') >= 0,
                unsupportedType.numberingSystem === 'latn' && unsupportedType.locale.indexOf('-u-nu-') < 0,
                currency.locale.indexOf('de') === 0, currency.style === 'currency',
                currency.currency === 'KWD', currency.currencyDisplay === 'code',
                currency.currencySign === 'accounting', currency.minimumIntegerDigits === 2,
                currency.minimumFractionDigits === 3, currency.maximumFractionDigits === 4,
                currency.useGrouping === false, currency.signDisplay === 'exceptZero',
                supported.length === 1 && supported[0] === 'de-DE'
              ].join(':');
            })()"#,
        ),
        "true:true:true:true:true:true:true:true:true:true:true:true:true:true:true:true:true:true:true:true:true"
    );
}

#[test]
fn coercion_order_and_ecmascript_conversion_rules_are_preserved() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var results = [];
              function errorName(thunk) {
                try { thunk(); return 'accepted'; } catch (error) { return error.name; }
              }
              results.push(errorName(function () {
                new Intl.NumberFormat('en', {style:Symbol('decimal')});
              }));
              results.push(errorName(function () {
                new Intl.NumberFormat('en', {minimumIntegerDigits:1n});
              }));
              var nullable = {
                [Symbol.toPrimitive]: null,
                valueOf: function () { return 4; }
              };
              results.push(new Intl.NumberFormat('en', {minimumIntegerDigits:nullable})
                .resolvedOptions().minimumIntegerDigits);

              var order = [];
              var fraction = { valueOf: function () { order.push('coerce fraction'); return 2; } };
              var options = {
                get minimumFractionDigits() { order.push('get min fraction'); return fraction; },
                get maximumFractionDigits() { order.push('get max fraction'); return fraction; },
                get minimumSignificantDigits() { order.push('get min significant'); return 3; },
                get maximumSignificantDigits() { order.push('get max significant'); return 5; }
              };
              new Intl.NumberFormat('en', options);
              results.push(order.join(','));

              var touched = false;
              var value = { valueOf: function () { touched = true; return 1; } };
              results.push(errorName(function () {
                Intl.NumberFormat.prototype.formatToParts.call({}, value);
              }) + ':' + touched);
              return results.join('|');
            })()"#,
        ),
        "TypeError|TypeError|4|get min fraction,get max fraction,get min significant,get max significant|TypeError:false"
    );
}

#[test]
fn construction_uses_new_target_and_never_symbol_has_instance() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var NF = Intl.NumberFormat;
              var log = [], customPrototype = Object.create(NF.prototype);
              function DerivedTarget() {}
              var Derived = new Proxy(DerivedTarget, {get:function (target, name, receiver) {
                if (name === 'prototype') { log.push('prototype'); return customPrototype; }
                return Reflect.get(target, name, receiver);
              }});
              var locales = {length:1, get 0() { log.push('locale'); return 'en-US'; }};
              var derived = Reflect.construct(NF, [locales], Derived);
              var samePrototype = Object.getPrototypeOf(derived) === customPrototype;
              var works = Object.getOwnPropertyDescriptor(NF.prototype, 'format').get.call(derived)(12) === '12';
              Object.defineProperty(NF, Symbol.hasInstance, {
                value: function () { throw new Error('consulted'); }, configurable: true
              });
              var ordinary = NF('en-US');
              return [samePrototype, works, log.join(','),
                ordinary.resolvedOptions().locale.indexOf('en') === 0].join(':');
            })()"#,
        ),
        "true:true:prototype,locale:true"
    );
}

#[test]
#[ignore = "pinned Hermes cannot express the built-in constructor fallback through public JSI; tracked in the Linux Intl issue"]
fn non_object_new_target_prototype_uses_the_intl_prototype_fallback() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              function PrimitiveTarget() {}
              PrimitiveTarget.prototype = 0;
              var fallback = Reflect.construct(Intl.NumberFormat, ['en'], PrimitiveTarget);
              return Object.getPrototypeOf(fallback) === Intl.NumberFormat.prototype;
            })()"#,
        ),
        "true"
    );
}

#[test]
fn number_and_bigint_locale_methods_share_exact_native_formatting() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var options = {useGrouping:false, maximumFractionDigits:2};
              var nf = new Intl.NumberFormat('en-US', options);
              var huge = 123456789012345678901234567890n;
              return [
                (1234.5).toLocaleString('en-US', options) === nf.format(1234.5),
                huge.toLocaleString('en-US', {useGrouping:false}) === '123456789012345678901234567890',
                huge.toLocaleString('de-DE') === new Intl.NumberFormat('de-DE').format(huge)
              ].join(':');
            })()"#,
        ),
        "true:true:true"
    );
}

#[test]
fn bound_format_retains_native_state_without_exposing_handles() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var formatter = new Intl.NumberFormat('de-DE', {minimumFractionDigits:2});
              globalThis.savedIntlFormat = formatter.format;
              return [formatter.format === formatter.format,
                Object.getOwnPropertyNames(formatter).length,
                typeof globalThis.__ibex2_intl_number_format].join(':');
            })()"#,
        ),
        "true:0:undefined"
    );
    assert!(runtime.collect_garbage());
    assert_eq!(
        eval(
            &mut runtime,
            "savedIntlFormat(1234.5).indexOf('1.234,50') >= 0 ? 'alive' : 'wrong'",
        ),
        "alive"
    );
}

#[test]
fn public_methods_have_builtin_names_lengths_and_are_not_constructors() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var nf = new Intl.NumberFormat('en-US');
              var getter = Object.getOwnPropertyDescriptor(
                Intl.NumberFormat.prototype, 'format').get;
              var methods = [Intl.NumberFormat.supportedLocalesOf, getter, nf.format,
                Intl.NumberFormat.prototype.formatToParts,
                Intl.NumberFormat.prototype.resolvedOptions,
                Number.prototype.toLocaleString, BigInt.prototype.toLocaleString];
              function rejectsConstruction(fn) {
                try { Reflect.construct(fn, []); return false; }
                catch (error) { return error.name === 'TypeError'; }
              }
              return [
                methods.every(rejectsConstruction),
                methods.map(function (fn) { return fn.name; }).join(','),
                methods.map(function (fn) { return fn.length; }).join(',')
              ].join('|');
            })()"#,
        ),
        "true|supportedLocalesOf,get format,,formatToParts,resolvedOptions,toLocaleString,toLocaleString|1,0,1,1,0,0,0"
    );
}

#[test]
fn invalid_options_throw_the_required_error_kinds() {
    let mut runtime = runtime(false);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              function kind(options) {
                try { new Intl.NumberFormat('en', options); return 'accepted'; }
                catch (error) { return error.name; }
              }
              return [
                kind({style:'currency'}), kind({style:'currency',currency:'US'}),
                kind({style:'unit'}), kind({style:'unit',unit:'horses'}),
                kind({numberingSystem:'ab'}), kind({minimumIntegerDigits:0}),
                kind({minimumFractionDigits:4,maximumFractionDigits:2}),
                kind({minimumSignificantDigits:4,maximumSignificantDigits:2}),
                kind({notation:'unknown'}), kind({signDisplay:'sometimes'})
              ].join(':');
            })()"#,
        ),
        "TypeError:RangeError:TypeError:RangeError:RangeError:RangeError:RangeError:RangeError:RangeError:RangeError"
    );
}

#[test]
fn hardening_preserves_public_surface_and_removes_private_factory() {
    let mut runtime = runtime(true);
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              var nf = new Intl.NumberFormat('en-US');
              return [
                typeof globalThis.__ibex2_intl_number_format,
                Object.isFrozen(Intl.NumberFormat.prototype),
                Intl.NumberFormat.length,
                Intl.NumberFormat.supportedLocalesOf.length,
                Object.getOwnPropertyDescriptor(Intl.NumberFormat, 'prototype').writable,
                Object.getOwnPropertyDescriptor(Intl.NumberFormat.prototype, 'format').get.name,
                JSON.stringify(nf.format.name), nf.format.length,
                Intl.NumberFormat.prototype.constructor === Intl.NumberFormat,
                Object.prototype.toString.call(nf),
                nf.format(1234)
              ].join(':');
            })()"#,
        ),
        "undefined:true:0:1:false:get format:\"\":1:true:[object Intl.NumberFormat]:1,234"
    );
}
