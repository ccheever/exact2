//! The packages' last path icons as icon tokens (LLP 1121 stage 14 C, after
//! C2's field; theme2's path-icons terms 2 and 3, the C2 plan's Q4 and Q5,
//! and theme2's stage 14 C ruling). ComposeFrame's gallery, camera and GIF,
//! AtCard's Attie mark and VerifiedBadge's paths were inline `svg` with
//! `fill-rule="evenodd"`; each one's holes wind against its outline, so it
//! fills the same nonzero (the gate checks each with two reference fills), and
//! its SVG is written nonzero with the `d` it replaced byte for byte (the
//! strings below are main's sources before the move, at e30590f56f): no
//! icon is even-odd, so no app using the packages gets a new compat id. The
//! verified badge's ground was a `<circle cx=12 cy=12 r=11.5>`: its SVG is
//! two arcs, new geometry, checked here to describe the same circle and
//! judged in pixels by the gate's edge rule. Each badge is two one-colour
//! templates stacked, its ground in `ink` and its check in `mark`. The moved
//! functions and `ToolGlyph` are gone, so nothing draws both ways; BarItem's
//! and HeaderButton's icons stay their caller's `d` (Q4).

use std::path::{Path, PathBuf};

/// (package, icon, the path it replaced on main).
const MOVED: [(&str, &str, &str); 7] = [
    // `iconGallery`, ToolGlyph in ComposeFrame's toolbar.
    ("actions", "gallery", "M19 15.355c-1.112.696-2.385 1.1-4 1.1-2.81 0-4.796-1.756-6.312-3.39a1 1 0 0 0-1.273-.16L5 14.71V17a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-1.646ZM16 9a1 1 0 1 0-2 0 1 1 0 0 0 2 0Zm3-2a2 2 0 0 0-2-2H7a2 2 0 0 0-2 2v5.213l1.246-.932.044-.031a3 3 0 0 1 3.862.455c1.468 1.581 2.942 2.75 4.848 2.75 1.704 0 2.854-.558 4-1.621V7Zm-1 2a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm3 8a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4v10Z"),
    // `iconCamera`.
    ("actions", "camera", "M8.371 3.89A2 2 0 0 1 10.035 3h3.930a2 2 0 0 1 1.664.89L17.035 6H20a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h2.965L8.37 3.89ZM13.965 5h-3.93L8.63 7.11A2 2 0 0 1 6.965 8H4v11h16V8h-2.965a2 2 0 0 1-1.664-.89L13.965 5ZM12 11a2 2 0 1 0 0 4 2 2 0 0 0 0-4Zm-4 2a4 4 0 1 1 8 0 4 4 0 0 1-8 0Z"),
    // `iconGif`.
    ("actions", "gif", "M6 12.287c0 1.54.92 2.463 2.48 2.463 1.434 0 2.353-.807 2.353-2.06v-.166c0-.578-.267-.834-.884-.834h-.806c-.416 0-.632.182-.632.535 0 .357.22.55.632.55h.146v.063c0 .36-.299.609-.735.609-.597 0-.904-.4-.904-1.168v-.52c0-.775.307-1.155.951-1.155.464 0 .7.31 1.018.477a.8.8 0 0 0 .409.108c.385 0 .656-.263.656-.636 0-.353-.26-.679-.664-.915-.409-.24-.96-.388-1.548-.388C6.955 9.250 6 10.2 6 11.67v.617Zm6.358 2.385c.526 0 .813-.31.813-.872v-3.627c0-.558-.295-.873-.825-.873s-.825.31-.825.873V13.8c0 .558.302.872.837.872Zm2.565 0c.519 0 .802-.306.802-.872v-.993h1.47c.361 0 .608-.252.608-.613 0-.365-.243-.613-.608-.613h-1.47v-.973h1.666c.35 0 .609-.26.609-.629 0-.368-.26-.64-.609-.64h-2.495c-.507 0-.821.314-.821.826V13.8c0 .554.31.872.848.872ZM19 7a2 2 0 0 0-2-2H7a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7Zm2 10a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4v10Z"),
    // `attieIcon`, AtCard's visit button.
    ("cards", "attie", "M14.398 2c4.502 0 8.151 3.634 8.151 8.117q0 .678-.108 1.325a5.67 5.67 0 0 1 1.507 3.855c0 8.837-23.948 8.837-23.948 0a5.68 5.68 0 0 1 1.977-4.31 5 5 0 0 1-.153-1.234c0-2.775 2.259-5.025 5.046-5.025q.67.002 1.289.166A8.15 8.15 0 0 1 14.399 2Zm-5.07 9.032a1.6 1.6 0 0 0-.46.008 1.09 1.09 0 0 1-1.628 1.414c-.137.3-.236.637-.284.997-.198 1.484.543 2.807 1.656 2.956 1.112.148 2.174-.934 2.372-2.419s-.544-2.807-1.656-2.956Zm6.008 0a1.6 1.6 0 0 0-.46.008 1.09 1.09 0 0 1-1.628 1.414c-.137.3-.236.637-.284.997-.198 1.484.544 2.807 1.656 2.956 1.112.148 2.174-.935 2.372-2.419s-.544-2.807-1.656-2.956Z"),
    // VerifiedBadge's "verified" check (`fill=mark`).
    ("cards", "verified-check", "M17.659 8.175a1.36 1.36 0 0 1 0 1.925l-6.224 6.223a1.36 1.36 0 0 1-1.925 0L6.4 13.212a1.361 1.361 0 0 1 1.925-1.925l2.149 2.148 5.26-5.26a1.36 1.36 0 0 1 1.925 0Z"),
    // VerifiedBadge's "verifier" ground (`fill=ink`).
    ("cards", "verifier", "M8.792 1.615a4.154 4.154 0 0 1 6.416 0 4.15 4.15 0 0 0 3.146 1.515 4.154 4.154 0 0 1 4 5.017 4.15 4.15 0 0 0 .777 3.404 4.154 4.154 0 0 1-1.427 6.255 4.15 4.15 0 0 0-2.177 2.73 4.154 4.154 0 0 1-5.781 2.784 4.15 4.15 0 0 0-3.492 0 4.154 4.154 0 0 1-5.78-2.784 4.15 4.15 0 0 0-2.178-2.73A4.154 4.154 0 0 1 .87 11.551a4.15 4.15 0 0 0 .776-3.404 4.154 4.154 0 0 1 4-5.017 4.15 4.15 0 0 0 3.146-1.515Z"),
    // VerifiedBadge's "verifier" check (`fill=mark`).
    ("cards", "verifier-check", "M17.861 8.26a1.44 1.44 0 0 1 0 2.033l-6.571 6.571a1.437 1.437 0 0 1-2.033 0L5.97 13.58a1.438 1.438 0 0 1 2.033-2.033l2.27 2.269 5.554-5.555a1.437 1.437 0 0 1 2.033 0Z"),
];

/// The verified ground: main's circle, and the SVG's path for it.
const CIRCLE: (f64, f64, f64) = (12.0, 12.0, 11.5);
const VERIFIED_GROUND: &str = "M12 .5a11.5 11.5 0 1 1 0 23 11.5 11.5 0 1 1 0-23Z";

/// What the move deleted, by package: none is left in any `.contract`.
const DELETED: [(&str, &[&str]); 2] = [
    ("cards", &["attieIcon("]),
    (
        "actions",
        &["iconGallery(", "iconCamera(", "iconGif(", "ToolGlyph"],
    ),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn svg(package: &str, name: &str) -> String {
    let file = repo().join(format!("packages/{package}/assets/{name}.svg"));
    std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
}

/// Named under its namespace in its package's design, with no `sf` (Apple
/// draws the path, not an SF Symbol).
fn assert_declared(package: &str, name: &str) {
    let design =
        std::fs::read_to_string(repo().join(format!("packages/{package}/{package}.tamber")))
            .unwrap();
    let line = format!("  {package}.{name} = icon(svg \"{name}.svg\")\n");
    assert!(
        design.contains(&line),
        "{package}.tamber has no `{}`",
        line.trim()
    );
}

#[test]
fn each_moved_svgs_d_is_the_path_it_replaced_byte_for_byte_written_nonzero() {
    for (package, name, d) in MOVED {
        let want = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{d}"/></svg>"#
        );
        assert_eq!(svg(package, name), want, "{package}/assets/{name}.svg");
        assert_declared(package, name);
    }
}

#[test]
fn the_verified_ground_is_mains_circle_drawn_as_two_arcs() {
    let want = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{VERIFIED_GROUND}"/></svg>"#
    );
    assert_eq!(svg("cards", "verified"), want);
    assert_declared("cards", "verified");
    // M x y, then two relative arcs `a rx ry rotation large sweep dx dy`,
    // then Z: the same radius as main's circle, starting at its top, the
    // first ending at its bottom (a diameter, so the arc is that circle's
    // half whatever its flags) and the second back at the top.
    let numbers: Vec<f64> = VERIFIED_GROUND
        .trim_start_matches('M')
        .trim_end_matches('Z')
        .replace('a', " ")
        .replace('-', " -")
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    let (cx, cy, r) = CIRCLE;
    assert_eq!(numbers.len(), 2 + 7 * 2, "{numbers:?}");
    let (x0, y0) = (numbers[0], numbers[1]);
    assert_eq!((x0, y0), (cx, cy - r), "it starts at the circle's top");
    let mut at = (x0, y0);
    for arc in numbers[2..].chunks(7) {
        assert_eq!(
            (arc[0], arc[1], arc[2]),
            (r, r, 0.0),
            "{arc:?}: the circle's radius, unrotated"
        );
        let to = (at.0 + arc[5], at.1 + arc[6]);
        // Each end is on the circle, and the two ends are opposite.
        for p in [at, to] {
            assert!(
                ((p.0 - cx).hypot(p.1 - cy) - r).abs() < 1e-9,
                "{p:?} is off the circle"
            );
        }
        assert_eq!(
            ((at.0 + to.0) / 2.0, (at.1 + to.1) / 2.0),
            (cx, cy),
            "{arc:?}: not a diameter"
        );
        at = to;
    }
    assert_eq!(at, (x0, y0), "it closes where it began");
    // Both arcs sweep the same way, so together they go round once.
    assert_eq!(numbers[2 + 4], numbers[2 + 7 + 4]);
}

#[test]
fn the_moved_functions_and_toolglyph_are_gone_and_nothing_draws_both_ways() {
    for (package, gone) in DELETED {
        for entry in std::fs::read_dir(repo().join("packages").join(package)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "contract") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for name in gone {
                assert!(!text.contains(name), "{} still has {name}", path.display());
            }
        }
    }
    // No inline `svg` is left in AtCard, ComposeFrame or VerifiedBadge's
    // file but the two that are no icon: ComposeFrame's counter ring (two
    // stroked circles) and the avatar's default figure (two colours).
    for (file, svgs) in [
        ("cards/at-card.contract", 0),
        ("actions/compose-frame.contract", 1),
        ("cards/common.contract", 1),
    ] {
        let text = std::fs::read_to_string(repo().join("packages").join(file)).unwrap();
        let n = text
            .lines()
            .filter(|l| l.trim_start().starts_with("svg "))
            .count();
        assert_eq!(n, svgs, "{file}'s inline svgs");
    }
}

#[test]
fn library_gate_keeps_every_moved_icon_nonzero_its_path_the_same_d() {
    let plan = contract::compile_path(&repo().join("apps/library-gate/app.contract"))
        .unwrap_or_else(|e| panic!("{e}"));
    let rows = MOVED
        .iter()
        .copied()
        .chain([("cards", "verified", VERIFIED_GROUND)]);
    for (package, name, d) in rows {
        let full = format!("{package}.{name}");
        let row = plan
            .icons
            .iter()
            .find(|i| plan.str(i.name) == full)
            .unwrap_or_else(|| panic!("no {full} in the plan's icons"));
        let icon =
            exact_runner::icon_of(&plan, row).unwrap_or_else(|| panic!("{full} reads as no icon"));
        assert_eq!((icon.path, icon.even_odd), (d, false), "{full}");
        assert_eq!(plan.str(row.sf), "", "{full}");
        assert_eq!(plan.str(row.view_box), "0 0 24 24", "{full}");
    }
}
