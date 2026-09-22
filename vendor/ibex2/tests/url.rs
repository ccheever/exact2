//! `URL` and `URLSearchParams` through their binding: the shape is JavaScript,
//! every answer is Rust's (LLP 0059.000 §3.4). The parser itself is WPT-tested
//! in `wpt_url.rs`; this is the object on top of it.
#![cfg(feature = "hermes")]

use ibex2::engine::hermes::{DynamicCode, Hermes};

fn runtime() -> Hermes {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib());
    rt.install_bindings().expect("bindings");
    rt
}

fn eval(rt: &mut Hermes, program: &str) -> String {
    rt.eval(program)
        .unwrap_or_else(|e| panic!("{program}: {e}"))
}

#[test]
fn a_url_exposes_the_components_the_parser_normalized() {
    let mut rt = runtime();
    let out = eval(
        &mut rt,
        "(function () { const u = new URL('https://user:pw@Example.com:8080/a/../b?x=1#f'); \
         return [u.href, u.origin, u.protocol, u.username, u.password, u.host, u.hostname, \
         u.port, u.pathname, u.search, u.hash, String(u), JSON.stringify(u)].join('|'); })()",
    );
    assert_eq!(
        out,
        "https://user:pw@example.com:8080/b?x=1#f|https://example.com:8080|https:|user|pw|\
         example.com:8080|example.com|8080|/b|?x=1|#f|https://user:pw@example.com:8080/b?x=1#f|\
         \"https://user:pw@example.com:8080/b?x=1#f\""
    );
    // The default port is empty, and an absent search or hash is empty, not "?"/"#".
    assert_eq!(
        eval(&mut rt, "(function () { const u = new URL('https://h:443/'); return [u.port, u.search, u.hash, u.host].join('|'); })()"),
        "|||h"
    );
}

/// The case Exact's router runs on every navigation: a path against a base.
#[test]
fn a_relative_url_resolves_against_its_base() {
    let mut rt = runtime();
    assert_eq!(
        eval(
            &mut rt,
            "new URL('/settings', 'https://exact.local').pathname"
        ),
        "/settings"
    );
    assert_eq!(
        eval(&mut rt, "new URL('../c?q#h', 'https://h/a/b/').href"),
        "https://h/a/c?q#h"
    );
    assert_eq!(
        eval(&mut rt, "new URL('//other/x', 'https://h/').host"),
        "other"
    );
}

#[test]
fn an_invalid_url_is_a_type_error_and_can_parse_says_so_without_throwing() {
    let mut rt = runtime();
    assert_eq!(
        eval(&mut rt, "(function () { try { new URL('not a url'); return 'no throw'; } catch (e) { return e.constructor.name + ': ' + (e.message.indexOf('invalid URL') >= 0); } })()"),
        "TypeError: true"
    );
    assert_eq!(eval(&mut rt, "String(URL.canParse('not a url'))"), "false");
    assert_eq!(
        eval(&mut rt, "String(URL.canParse('/x', 'https://h'))"),
        "true"
    );
    assert_eq!(eval(&mut rt, "String(URL.parse('nope'))"), "null");
    assert_eq!(
        eval(&mut rt, "URL.parse('/x', 'https://h').href"),
        "https://h/x"
    );
    assert_eq!(
        eval(&mut rt, "(function () { try { URL('https://h'); return 'no throw'; } catch (e) { return e.constructor.name; } })()"),
        "TypeError"
    );
}

/// Setters are the spec's: they re-parse in Rust, fail silently, and only
/// `href` throws.
#[test]
fn setters_rewrite_the_url_with_the_specs_semantics() {
    let mut rt = runtime();
    let out = eval(
        &mut rt,
        "(function () { const u = new URL('https://h/p?a=1#x'); const log = []; \
         u.search = 'b=2'; log.push(u.href); \
         u.hash = ''; log.push(u.href); \
         u.pathname = 'q r'; log.push(u.pathname); \
         u.port = '8080'; log.push(u.host); \
         u.port = 'abc'; log.push(u.port); \
         u.host = 'h2:99'; log.push(u.hostname + ':' + u.port); \
         u.host = 'h3:80abc'; log.push(u.hostname + ':' + u.port); \
         u.host = 'h4:'; log.push(u.hostname + ':' + u.port); \
         u.protocol = 'http'; log.push(u.protocol); \
         u.protocol = 'mailto'; log.push(u.protocol); \
         u.protocol = 'https:garbage'; log.push(u.protocol); /* and 80 is now the default port, so it drops */ \
         u.username = 'me'; u.password = 'pw'; log.push(u.href); \
         u.href = 'https://z/'; log.push(u.origin); \
         try { u.href = 'nope'; log.push('no throw'); } catch (e) { log.push(e.constructor.name); } \
         log.push(u.href); \
         return log.join('|'); })()",
    );
    assert_eq!(
        out,
        "https://h/p?b=2#x|https://h/p?b=2|/q%20r|h:8080|8080|h2:99|h3:80|h4:80|http:|http:|https:|\
         https://me:pw@h4/q%20r?b=2|https://z|TypeError|https://z/"
    );
    assert_eq!(
        eval(
            &mut rt,
            "(function () { const u = new URL('https://h/'); u.origin = 'x'; return u.origin; })()"
        ),
        "https://h",
        "origin is read-only"
    );
}

/// `url.searchParams` is a live view over `url.search`, and the same object
/// every time.
#[test]
fn search_params_on_a_url_are_a_live_view() {
    let mut rt = runtime();
    let out = eval(
        &mut rt,
        "(function () { const u = new URL('https://h/?a=1&b=2'); const log = []; \
         u.searchParams.append('c', '3'); u.searchParams.delete('a'); log.push(u.href); \
         u.search = '?z=9'; log.push(u.searchParams.get('z')); \
         log.push(String(u.searchParams === u.searchParams)); \
         u.searchParams.set('q', 'a b&c'); log.push(u.search); \
         u.searchParams.delete('z'); u.searchParams.delete('q'); log.push(u.href); \
         return log.join('|'); })()",
    );
    assert_eq!(out, "https://h/?b=2&c=3|9|true|?z=9&q=a+b%26c|https://h/");
}

#[test]
fn search_params_stand_alone_with_every_construction_form_and_method() {
    let mut rt = runtime();
    let out = eval(
        &mut rt,
        "(function () { const log = []; \
         const p = new URLSearchParams('?b=2&a=1&a=3'); \
         p.sort(); log.push(String(p)); \
         log.push(p.getAll('a').join(',')); \
         log.push(String(p.has('a', '3')) + String(p.has('a', '2')) + String(p.has('zz'))); \
         p.delete('a', '1'); log.push(String(p)); \
         log.push([...p].map(e => e.join(':')).join(',')); \
         log.push([...p.keys()].join(',') + '/' + [...p.values()].join(',')); \
         log.push(String(p.size)); \
         const seen = []; p.forEach(function (v, k) { seen.push(k + '=' + v + this.tag); }, { tag: '!' }); log.push(seen.join(',')); \
         log.push(new URLSearchParams([['x', '1'], ['y', '2']]).toString()); \
         log.push(new URLSearchParams({ k: 'v w', n: 1 }).toString()); \
         log.push(new URLSearchParams(p).toString()); \
         log.push(new URLSearchParams('n=%E2%9C%93+x').get('n')); \
         log.push(String(new URLSearchParams().get('missing'))); \
         try { new URLSearchParams([['only-one']]); log.push('no throw'); } catch (e) { log.push(e.constructor.name); } \
         log.push(Object.prototype.toString.call(p)); \
         return log.join('|'); })()",
    );
    assert_eq!(
        out,
        "a=1&a=3&b=2|1,3|truefalsefalse|a=3&b=2|a:3,b:2|a,b/3,2|2|a=3!,b=2!|x=1&y=2|k=v+w&n=1|\
         a=3&b=2|✓ x|null|TypeError|[object URLSearchParams]"
    );
}
