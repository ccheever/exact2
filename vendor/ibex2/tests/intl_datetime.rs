//! Linux's vanilla Hermes DateTimeFormat has real ICU `format` but a dummy
//! epoch-number `formatToParts`, and its resolved options cannot reconstruct
//! the formatter. These tests pin Ibex's one-native-formatter replacement.
#![cfg(all(feature = "hermes", target_os = "linux"))]

use ibex2::engine::hermes::{DynamicCode, Hermes};

fn runtime() -> Hermes {
    let mut runtime = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(runtime.install_stdlib());
    runtime.install_bindings().expect("bindings");
    runtime
}

fn eval(runtime: &mut Hermes, source: &str) -> String {
    runtime
        .eval(source)
        .unwrap_or_else(|error| panic!("{source}: {error}"))
}

#[test]
fn explicit_components_and_parts_come_from_one_formatter() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const f = new Intl.DateTimeFormat("en-US", {
                timeZone: "UTC", year: "numeric", month: "2-digit", day: "2-digit",
                hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23"
              });
              const text = f.format(0), p = f.formatToParts(0);
              return String(p.map(x => x.value).join("") === text) + "|" + text + "|" +
                p.map(x => x.type).join(",");
            })()"#,
        ),
        "true|01/01/1970, 00:00:00|month,literal,day,literal,year,literal,hour,literal,minute,literal,second"
    );
}

#[test]
fn fractional_milliseconds_are_time_clipped_before_format_and_parts() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const f = new Intl.DateTimeFormat("en-US", {
                timeZone:"UTC", year:"numeric", month:"2-digit", day:"2-digit",
                hour:"2-digit", minute:"2-digit", second:"2-digit", hourCycle:"h23"
              });
              const text0 = f.format(0), parts0 = JSON.stringify(f.formatToParts(0));
              return [f.format(-0.1) === text0,
                JSON.stringify(f.formatToParts(-0.1)) === parts0,
                f.format(0.9) === text0,
                JSON.stringify(f.formatToParts(0.9)) === parts0].join("|");
            })()"#,
        ),
        "true|true|true|true"
    );
}

#[test]
fn locale_extensions_calendars_numbering_and_hour_cycles_are_effective() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const f = new Intl.DateTimeFormat("de-DE-u-ca-buddhist-nu-arab-hc-h23", {
                timeZone: "UTC", year: "numeric", hour: "numeric"
              });
              const r = f.resolvedOptions(), text = f.format(0), p = f.formatToParts(0);
              return [r.locale, r.calendar, r.numberingSystem, r.timeZone, r.hourCycle,
                r.hour12, p.map(x => x.value).join("") === text,
                /[٠-٩]/.test(text), p.some(x => x.type === "year"),
                p.some(x => x.type === "hour")].join("|");
            })()"#,
        ),
        "de-DE-u-ca-buddhist-hc-h23-nu-arab|buddhist|arab|UTC|h23|false|true|true|true|true"
    );

    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const a = new Intl.DateTimeFormat("en-US", {timeZone:"UTC",hour:"numeric",hour12:true});
              const b = new Intl.DateTimeFormat("en-US", {timeZone:"UTC",hour:"numeric",hour12:false});
              return [a.resolvedOptions().hour12, b.resolvedOptions().hour12,
                a.format(0), b.format(0)].join("|");
            })()"#,
        ),
        "true|false|12\u{202f}AM|24"
    );
}

#[test]
fn styles_time_zones_and_date_prototype_methods_share_the_binding() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const options = {timeZone:"America/New_York", dateStyle:"short", timeStyle:"short"};
              const f = new Intl.DateTimeFormat("en-US", options), date = new Date(0);
              const r = f.resolvedOptions(), p = f.formatToParts(0);
              return [f.format(0), date.toLocaleString("en-US", options),
                p.map(x => x.value).join("") === f.format(0), r.dateStyle,
                r.timeStyle, r.timeZone].join("|");
            })()"#,
        ),
        "12/31/69, 7:00\u{202f}PM|12/31/69, 7:00\u{202f}PM|true|short|short|America/New_York"
    );

    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const d = new Date(0), date = {timeZone:"UTC",year:"numeric",month:"numeric",day:"numeric"};
              const time = {timeZone:"UTC",hour:"numeric",minute:"numeric",second:"numeric",hourCycle:"h23"};
              return [d.toLocaleDateString("en-US", date), d.toLocaleTimeString("en-US", time)].join("|");
            })()"#,
        ),
        "1/1/1970|00:00:00"
    );
}

#[test]
fn supported_locales_and_resolved_options_are_real() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"JSON.stringify(Intl.DateTimeFormat.supportedLocalesOf(["en-US", "de-DE", "zz-ZZ", "en-US"]))"#,
        ),
        r#"["en-US","de-DE"]"#
    );
    assert_eq!(
        eval(
            &mut runtime,
            r#"JSON.stringify(new Intl.DateTimeFormat("en-US", {
              timeZone:"Etc/UTC",year:"numeric",month:"long",day:"2-digit",
              hour:"numeric",minute:"numeric",hourCycle:"h23",timeZoneName:"long"
            }).resolvedOptions())"#,
        ),
        r#"{"locale":"en-US","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","hourCycle":"h23","hour12":false,"year":"numeric","month":"long","day":"2-digit","hour":"2-digit","minute":"2-digit","timeZoneName":"long"}"#
    );
}

#[test]
fn invalid_values_throw_at_the_ecma_402_boundary() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              function name(work) { try { work(); return "none"; } catch (e) { return e.name; } }
              const f = new Intl.DateTimeFormat("en-US", {timeZone:"UTC"});
              return [
                name(() => f.format(NaN)), name(() => f.format(Infinity)),
                name(() => f.format(8640000000000001)),
                name(() => new Intl.DateTimeFormat("en-US", {timeZone:"Mars/Olympus"})),
                name(() => new Intl.DateTimeFormat("en-US", {timeZone:"Asia/Kolkata"})),
                name(() => new Intl.DateTimeFormat("en-US", {calendar:"x"})),
                name(() => new Intl.DateTimeFormat("en-US", {hourCycle:"h99"})),
                name(() => new Intl.DateTimeFormat("en-US", {dateStyle:"short",year:"numeric"})),
                name(() => f.format(1n))
              ].join("|");
            })()"#,
        ),
        "RangeError|RangeError|RangeError|RangeError|RangeError|RangeError|RangeError|TypeError|TypeError"
    );
}

#[test]
fn option_gets_and_coercions_follow_the_selected_2020_order() {
    let mut runtime = runtime();
    let order = eval(
        &mut runtime,
        r#"(function () {
          const log = [];
          const values = {timeZone:"UTC",year:"numeric"};
          const options = new Proxy({}, {get(_target, key) { log.push(String(key)); return values[key]; }});
          new Intl.DateTimeFormat("en-US", options);
          return log.join(",");
        })()"#,
    );
    assert_eq!(
        order,
        "weekday,year,month,day,hour,minute,second,dateStyle,timeStyle,localeMatcher,calendar,numberingSystem,hour12,hourCycle,timeZone,weekday,era,year,month,day,hour,minute,second,timeZoneName,formatMatcher,dateStyle,timeStyle"
    );

    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const log = [];
              const calendar = { [Symbol.toPrimitive]: null,
                toString() { log.push("calendar.toString"); return "gregory"; } };
              const zone = { [Symbol.toPrimitive](hint) { log.push("zone:" + hint); return "UTC"; } };
              new Intl.DateTimeFormat("en-US", {calendar, timeZone:zone, year:"numeric"});
              return log.join("|");
            })()"#,
        ),
        "calendar.toString|zone:string"
    );

    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const log = [];
              const options = new Proxy({}, {get(_target, key) {
                log.push(String(key)); if (key === "timeZone") return "Not/AZone";
              }});
              try { new Intl.DateTimeFormat("en-US", options); } catch (e) {}
              return log.join(",");
            })()"#,
        ),
        "weekday,year,month,day,hour,minute,second,dateStyle,timeStyle,localeMatcher,calendar,numberingSystem,hour12,hourCycle,timeZone"
    );

    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const log = [], invalid = new Date(NaN);
              const locales = { get 0() { log.push("locale"); return "en-US"; }, length: 1 };
              const options = new Proxy({}, {get(_target, key) { log.push(String(key)); }});
              const invalidResult = invalid.toLocaleString(locales, options);
              new Date(0).toLocaleDateString(locales, options);
              return invalidResult + "|" + log.slice(0, 4).join(",");
            })()"#,
        ),
        // Invalid Date observes neither argument. A valid Date starts with the
        // outer ToDateTimeOptions pass, before CanonicalizeLocaleList.
        "Invalid Date|weekday,year,month,day"
    );
}

#[test]
fn time_zone_names_are_ascii_case_insensitive_and_canonical() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const a = new Intl.DateTimeFormat("en-US", {timeZone:"aMeRiCa/nEw_yOrK",year:"numeric"});
              const b = new Intl.DateTimeFormat("en-US", {timeZone:"utc",year:"numeric"});
              const c = new Intl.DateTimeFormat("en-US", {timeZone:"UTC",calendar:"BUDDHIST",numberingSystem:"ARAB",year:"numeric"});
              const d = new Intl.DateTimeFormat("en-x-u-ca-buddhist-nu-arab", {timeZone:"UTC",year:"numeric"});
              const e = new Intl.DateTimeFormat("en-u-ca-buddhist-nu-arab-hc-h23", {
                timeZone:"UTC", calendar:"foobar", numberingSystem:"foobar",
                hour12:true, hour:"numeric"
              });
              const dr = d.resolvedOptions();
              const er = e.resolvedOptions();
              return a.resolvedOptions().timeZone + "|" + b.resolvedOptions().timeZone + "|" +
                c.resolvedOptions().calendar + "|" + c.resolvedOptions().numberingSystem + "|" +
                dr.calendar + "|" + dr.numberingSystem + "|" + er.locale + "|" +
                er.calendar + "|" + er.numberingSystem + "|" +
                (er.locale.indexOf("-hc-") < 0 && er.locale.indexOf("-hc-h23") < 0);
            })()"#,
        ),
        "America/New_York|UTC|buddhist|arab|gregory|latn|en-u-ca-buddhist-nu-arab|buddhist|arab|true"
    );
}

#[test]
fn public_methods_have_builtin_names_lengths_and_are_not_constructors() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              const dtf = new Intl.DateTimeFormat("en-US", {timeZone:"UTC"});
              const getter = Object.getOwnPropertyDescriptor(
                Intl.DateTimeFormat.prototype, "format").get;
              const methods = [Intl.DateTimeFormat.supportedLocalesOf, getter, dtf.format,
                Intl.DateTimeFormat.prototype.formatToParts,
                Intl.DateTimeFormat.prototype.resolvedOptions,
                Date.prototype.toLocaleString, Date.prototype.toLocaleDateString,
                Date.prototype.toLocaleTimeString];
              function rejectsConstruction(fn) {
                try { Reflect.construct(fn, []); return false; }
                catch (error) { return error.name === "TypeError"; }
              }
              return [
                methods.every(rejectsConstruction),
                methods.map(fn => fn.name).join(","),
                methods.map(fn => fn.length).join(",")
              ].join("|");
            })()"#,
        ),
        "true|supportedLocalesOf,get format,,formatToParts,resolvedOptions,toLocaleString,toLocaleDateString,toLocaleTimeString|1,0,1,1,0,0,0,0"
    );
}

#[test]
fn subclass_brand_bound_function_and_receiver_rules_hold() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              class Child extends Intl.DateTimeFormat {}
              function Other() {}
              Other.prototype = Object.create(Intl.DateTimeFormat.prototype);
              const child = new Child("en-US", {timeZone:"UTC",year:"numeric"});
              const reflected = Reflect.construct(Intl.DateTimeFormat,
                ["en-US", {timeZone:"UTC",year:"numeric"}], Other);
              const called = Intl.DateTimeFormat("en-US", {timeZone:"UTC",year:"numeric"});
              const log = [], customPrototype = Object.create(Intl.DateTimeFormat.prototype);
              function ObjectTarget() {}
              const observedTarget = new Proxy(ObjectTarget, {get(target, name, receiver) {
                if (name === "prototype") { log.push("prototype"); return customPrototype; }
                return Reflect.get(target, name, receiver);
              }});
              const locales = {length:1, get 0() { log.push("locale"); return "en-US"; }};
              const custom = Reflect.construct(Intl.DateTimeFormat,
                [locales, {timeZone:"UTC",year:"numeric"}], observedTarget);
              const getter = Object.getOwnPropertyDescriptor(Intl.DateTimeFormat.prototype,"format").get;
              let touched = false;
              let fakeParts, fakeGetter;
              try { Intl.DateTimeFormat.prototype.formatToParts.call({}, {valueOf(){touched=true;return 0;}}); }
              catch (e) { fakeParts = e.name; }
              try { getter.call({}); } catch (e) { fakeGetter = e.name; }
              return [child instanceof Child, child.format(0), reflected instanceof Other,
                reflected.format(0), called instanceof Intl.DateTimeFormat,
                Object.getPrototypeOf(custom) === customPrototype, log.join(","),
                custom.format(0), fakeParts,
                fakeGetter, touched, child.format === child.format,
                Intl.DateTimeFormat.prototype.constructor === Intl.DateTimeFormat,
                Intl.DateTimeFormat.length, Intl.DateTimeFormat.supportedLocalesOf.length,
                Object.getOwnPropertyDescriptor(Intl.DateTimeFormat,"prototype").writable,
                getter.name, JSON.stringify(child.format.name), child.format.length].join("|");
            })()"#,
        ),
        "true|1970|true|1970|true|true|prototype,locale|1970|TypeError|TypeError|false|true|true|0|1|false|get format|\"\"|1"
    );
}

#[test]
#[ignore = "pinned Hermes cannot express the built-in constructor fallback through public JSI; tracked in the Linux Intl issue"]
fn non_object_new_target_prototype_uses_the_intl_prototype_fallback() {
    let mut runtime = runtime();
    assert_eq!(
        eval(
            &mut runtime,
            r#"(function () {
              function PrimitiveTarget() {}
              PrimitiveTarget.prototype = 0;
              const value = Reflect.construct(Intl.DateTimeFormat,
                ["en-US", {timeZone:"UTC",year:"numeric"}], PrimitiveTarget);
              return Object.getPrototypeOf(value) === Intl.DateTimeFormat.prototype;
            })()"#,
        ),
        "true"
    );
}

#[test]
fn bound_format_keeps_native_state_alive_across_gc_and_hardening() {
    let mut runtime = runtime();
    eval(
        &mut runtime,
        r#"(function () {
          let formatter = new Intl.DateTimeFormat("en-US", {timeZone:"UTC",year:"numeric"});
          globalThis.savedDateFormat = formatter.format;
          formatter = null;
        })()"#,
    );
    assert!(runtime.collect_garbage());
    assert_eq!(eval(&mut runtime, "savedDateFormat(0)"), "1970");
    runtime.harden().expect("harden");
    assert_eq!(eval(&mut runtime, "savedDateFormat(0)"), "1970");
    assert_eq!(
        eval(
            &mut runtime,
            "Object.isFrozen(Intl.DateTimeFormat.prototype) && Intl.DateTimeFormat.prototype.constructor === Intl.DateTimeFormat",
        ),
        "true"
    );
    assert_eq!(
        eval(
            &mut runtime,
            r#"Object.getOwnPropertyNames(globalThis).filter(x => x.indexOf("ibex2_intl") >= 0).join(",")"#,
        ),
        ""
    );
}
