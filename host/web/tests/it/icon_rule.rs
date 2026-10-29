//! @ref LLP 1121 stage 14 C2 — an icon's fill rule on the web host: the
//! element the mask is painted for (glue.js `refreshSymbols` writes each
//! `<path>`'s `fill-rule` from it; tests/symbol-hole.test.mjs draws it).

use exact_web::Host;

/// Stage 14 C2: an even-odd icon's form (`evenodd:` before its path) draws
/// its path, the marker stripped, under its viewBox, marked even-odd for
/// the mask (`data-symbol-evenodd`, not `data-symbol-nonzero`); a nonzero
/// icon is as before; a role's even-odd silhouette is marked neither, as
/// before.
#[test]
fn an_even_odd_icon_is_marked_for_its_mask() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      image \"symbol:icon||0 0 24 24|evenodd:M2 2H22V22H2Z M8 8H16V16H8Z|icons/ring.svg\"\n      image \"symbol:icon||0 0 24 24|M2 2H22V22H2Z|icons/sq.svg\"\n      image \"symbol:bookmark-fill\"\n",
    )
    .unwrap();
    let (_, first) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    let ops: Vec<&str> = first
        .split("{\"op\":")
        .filter(|o| o.contains("data-symbol-source"))
        .collect();
    let [ring, square, role] = ops[..] else {
        panic!("{first}")
    };
    assert!(
        ring.contains("\"data-symbol-path\":\"M2 2H22V22H2Z M8 8H16V16H8Z\""),
        "{ring}"
    );
    assert!(ring.contains("\"data-symbol-evenodd\":\"\""), "{ring}");
    assert!(
        ring.contains("\"data-symbol-viewbox\":\"0 0 24 24\""),
        "{ring}"
    );
    assert!(!ring.contains("data-symbol-nonzero"), "{ring}");
    assert!(square.contains("\"data-symbol-nonzero\":\"\""), "{square}");
    assert!(!square.contains("data-symbol-evenodd"), "{square}");
    assert!(
        !role.contains("data-symbol-evenodd") && !role.contains("data-symbol-nonzero"),
        "{role}"
    );
}
