//! theme2's path-icons ruling (2026-10-23, terms 2 and 3): `@exact/cards`'
//! and `@exact/actions`' nonzero one-colour path icons are icon tokens. Each
//! SVG in the package's `assets/` holds the path function's `d` it replaced,
//! byte for byte (the strings below are main's `common.contract` before the
//! move, at 0277d04737), so a later move starts from the same source; the
//! moved functions are gone, so nothing draws both ways; and library-gate's
//! plan keeps every one in its `icons` table, its path the same `d`.

use std::path::{Path, PathBuf};

/// (package, icon, the path function's string on main).
const MOVED: [(&str, &str, &str); 15] = [
    // cards's `iconRepost`.
    ("cards", "repost", "M16.793 2.293a1 1 0 0 1 1.414 0L20.5 4.586a2 2 0 0 1 0 2.828l-2.293 2.293a1 1 0 0 1-1.414-1.414L18.086 7H7a2 2 0 0 0-2 2v2a1 1 0 1 1-2 0V9a4 4 0 0 1 4-4h11.086l-1.293-1.293a1 1 0 0 1 0-1.414ZM20 12a1 1 0 0 1 1 1v2a4 4 0 0 1-4 4H5.914l1.293 1.293a1 1 0 1 1-1.414 1.414L3.5 19.414a2 2 0 0 1 0-2.828l2.293-2.293a1 1 0 0 1 1.414 1.414L5.914 17H17a2 2 0 0 0 2-2v-2a1 1 0 0 1 1-1Z"),
    // cards's `iconPin`.
    ("cards", "pin", "M6.5 3a1 1 0 0 1 1-1h9a1 1 0 0 1 1 1v3.997a6.25 6.25 0 0 0 1.83 4.42l.377.376A1 1 0 0 1 20 12.5V15a1 1 0 0 1-1 1h-6v5a1 1 0 1 1-2 0v-5H5a1 1 0 0 1-1-1v-2.5a1 1 0 0 1 .293-.707l.376-.377A6.25 6.25 0 0 0 6.5 6.996V3.001Zm2 1v2.997a8.25 8.25 0 0 1-2.416 5.834L6 12.914V14h12v-1.086l-.084-.083A8.25 8.25 0 0 1 15.5 6.997V4h-7Z"),
    // cards's `iconCornerDownRight`.
    ("cards", "corner-down-right", "M15.793 10.293a1 1 0 0 1 1.338-.068l.076.068 3.293 3.293a2 2 0 0 1 .138 2.677l-.138.151-3.293 3.293a1 1 0 1 1-1.414-1.414L18.086 16H8a5 5 0 0 1-5-5V5a1 1 0 0 1 2 0v6a3 3 0 0 0 3 3h10.086l-2.293-2.293-.068-.076a1 1 0 0 1 .068-1.338Z"),
    // cards's `iconBan`.
    ("cards", "ban", "M12 4a8 8 0 0 0-6.32 12.906L16.906 5.68A7.96 7.96 0 0 0 12 4Zm6.32 3.094L7.094 18.32A8 8 0 0 0 18.32 7.094ZM2 12C2 6.477 6.477 2 12 2a9.97 9.97 0 0 1 7.071 2.929A9.97 9.97 0 0 1 22 12c0 5.523-4.477 10-10 10a9.97 9.97 0 0 1-7.071-2.929A9.97 9.97 0 0 1 2 12Z"),
    // cards's `iconEyeSlash`.
    ("cards", "eye-slash", "M2.293 2.293a1 1 0 0 1 1.414 0L7.335 5.92l.03.03 3.22 3.222 4.243 4.242 3.22 3.22.03.03 3.63 3.629a1 1 0 0 1-1.415 1.414l-3.09-3.09c-2.65 1.478-5.625 1.778-8.421.869-3.039-.987-5.779-3.37-7.67-7.027a1 1 0 0 1 0-.918c1.086-2.1 2.452-3.78 3.996-5.019L2.293 3.707a1 1 0 0 1 0-1.414Zm4.24 5.654 2.021 2.021a4 4 0 0 0 5.478 5.478l1.688 1.688c-2.042.982-4.246 1.124-6.32.45-2.34-.76-4.594-2.586-6.265-5.584.97-1.739 2.135-3.083 3.398-4.053Zm3.535 3.535 2.45 2.45a2 2 0 0 1-2.45-2.45Zm.81-5.405c3.573-.49 7.45 1.369 9.987 5.923a15 15 0 0 1-1.347 2.02 1 1 0 1 0 1.564 1.247 17 17 0 0 0 1.806-2.808 1 1 0 0 0 0-.918c-2.833-5.479-7.584-8.088-12.281-7.446a1 1 0 0 0 .271 1.982Z"),
    // cards's `iconWarning`.
    ("cards", "warning", "M11.14 4.494a.995.995 0 0 1 1.72 0l7.001 12.008a.996.996 0 0 1-.86 1.498H4.999a.996.996 0 0 1-.86-1.498L11.14 4.494Zm3.447-1.007c-1.155-1.983-4.019-1.983-5.174 0L2.41 15.494C1.247 17.491 2.686 20 4.998 20h14.004c2.312 0 3.751-2.509 2.587-4.506L14.587 3.487ZM13 9.019a1 1 0 1 0-2 0v2.994a1 1 0 1 0 2 0V9.02Zm-1 4.731a1.25 1.25 0 1 0 0 2.5 1.25 1.25 0 0 0 0-2.5Z"),
    // cards's `iconCircleInfo`.
    ("cards", "circle-info", "M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16ZM2 12C2 6.477 6.477 2 12 2s10 4.477 10 10-4.477 10-10 10S2 17.523 2 12Zm8-1a1 1 0 0 1 1-1h1a1 1 0 0 1 1 1v5a1 1 0 1 1-2 0v-4a1 1 0 0 1-1-1Zm1-3a1 1 0 1 0 2 0 1 1 0 0 0-2 0Z"),
    // actions's `iconReply`.
    ("actions", "reply", "M20.002 7a2 2 0 0 0-2-2h-12a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2a1 1 0 0 1 1 1v1.918l3.375-2.7a1 1 0 0 1 .625-.218h5a2 2 0 0 0 2-2V7Zm2 8a4 4 0 0 1-4 4h-4.648l-4.727 3.781A1.001 1.001 0 0 1 7.002 22v-3h-1a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h12a4 4 0 0 1 4 4v8Z"),
    // actions's `iconRepost`.
    ("actions", "repost", "M16.793 2.293a1 1 0 0 1 1.414 0L20.5 4.586a2 2 0 0 1 0 2.828l-2.293 2.293a1 1 0 0 1-1.414-1.414L18.086 7H7a2 2 0 0 0-2 2v2a1 1 0 1 1-2 0V9a4 4 0 0 1 4-4h11.086l-1.293-1.293a1 1 0 0 1 0-1.414ZM20 12a1 1 0 0 1 1 1v2a4 4 0 0 1-4 4H5.914l1.293 1.293a1 1 0 1 1-1.414 1.414L3.5 19.414a2 2 0 0 1 0-2.828l2.293-2.293a1 1 0 0 1 1.414 1.414L5.914 17H17a2 2 0 0 0 2-2v-2a1 1 0 0 1 1-1Z"),
    // actions's `iconHeart`.
    ("actions", "heart", "M16.734 5.091c-1.238-.276-2.708.047-4.022 1.38a1 1 0 0 1-1.424 0C9.974 5.137 8.504 4.814 7.266 5.09c-1.263.282-2.379 1.206-2.92 2.556C3.33 10.18 4.252 14.84 12 19.348c7.747-4.508 8.67-9.168 7.654-11.7-.541-1.351-1.657-2.275-2.92-2.557Zm4.777 1.812c1.604 4-.494 9.69-9.022 14.47a1 1 0 0 1-.978 0C2.983 16.592.885 10.902 2.49 6.902c.779-1.942 2.414-3.334 4.342-3.764 1.697-.378 3.552.003 5.169 1.286 1.617-1.283 3.472-1.664 5.17-1.286 1.927.43 3.562 1.822 4.34 3.764Z"),
    // actions's `iconHeartFilled`.
    ("actions", "heart-filled", "M12.489 21.372c8.528-4.78 10.626-10.47 9.022-14.47-.779-1.941-2.414-3.333-4.342-3.763-1.697-.378-3.552.003-5.169 1.287-1.617-1.284-3.472-1.665-5.17-1.287-1.927.43-3.562 1.822-4.34 3.764-1.605 4 .493 9.69 9.021 14.47a1 1 0 0 0 .978 0Z"),
    // actions's `iconBookmark`.
    ("actions", "bookmark", "M9.7 16.895a4 4 0 0 1 4.6 0l3.7 2.6V6.5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v12.995l3.7-2.6Zm10.3 2.6c0 1.62-1.825 2.567-3.15 1.636l-3.7-2.6a2 2 0 0 0-2.3 0l-3.7 2.6C5.825 22.062 4 21.115 4 19.495V6.5a4 4 0 0 1 4-4h8a4 4 0 0 1 4 4v12.995Z"),
    // actions's `iconBookmarkFilled`.
    ("actions", "bookmark-filled", "M16 2.5a4 4 0 0 1 4 4v12.995c0 1.62-1.825 2.567-3.15 1.636l-3.7-2.6a2 2 0 0 0-2.3 0l-3.7 2.6C5.825 22.062 4 21.115 4 19.495V6.5a4 4 0 0 1 4-4h8Z"),
    // actions's `iconShare`.
    ("actions", "share", "M11.839 4.744c0-1.488 1.724-2.277 2.846-1.364l.107.094 7.66 7.256.128.134c.558.652.558 1.62 0 2.272l-.128.135-7.66 7.255c-1.115 1.057-2.953.267-2.953-1.27v-2.748c-3.503.055-5.417.41-6.592.97-.997.474-1.525 1.122-2.084 2.14l-.243.46c-.558 1.088-2.09.583-2.08-.515l.015-.748c.111-3.68.777-6.5 2.546-8.415 1.83-1.98 4.63-2.771 8.438-2.884V4.744Zm2 3.256c0 .79-.604 1.41-1.341 1.494l-.149.01c-3.9.057-6.147.813-7.48 2.254-.963 1.043-1.562 2.566-1.842 4.79.38-.327.826-.622 1.361-.877 1.656-.788 4.08-1.14 7.938-1.169l.153.007A1.5 1.5 0 0 1 13.839 16v2.675L20.884 12l-7.045-6.676V8Z"),
    // actions's `iconDots`.
    ("actions", "dots", "M2 12a2 2 0 1 1 4 0 2 2 0 0 1-4 0Zm16 0a2 2 0 1 1 4 0 2 2 0 0 1-4 0Zm-6-2a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z"),
];

/// The functions the icons replaced, by package: none is left.
const DELETED: [(&str, &[&str]); 2] = [
    (
        "cards",
        &[
            "iconRepost",
            "iconPin",
            "iconCornerDownRight",
            "iconBan",
            "iconEyeSlash",
            "iconWarning",
            "iconCircleInfo",
        ],
    ),
    (
        "actions",
        &[
            "iconReply",
            "iconRepost",
            "iconHeart",
            "iconHeartFilled",
            "iconBookmark",
            "iconBookmarkFilled",
            "iconShare",
            "iconDots",
        ],
    ),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn each_moved_svgs_d_is_its_path_functions_string_byte_for_byte() {
    for (package, name, d) in MOVED {
        let file = repo().join(format!("packages/{package}/assets/{name}.svg"));
        let svg = std::fs::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let want = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{d}"/></svg>"#
        );
        assert_eq!(
            String::from_utf8_lossy(&svg),
            want,
            "{} is not its function's path, byte for byte",
            file.display()
        );
        assert_eq!(svg, want.as_bytes(), "{}", file.display());
        // Named under its namespace in its package's design, with no `sf`
        // (Apple draws the path, not an SF Symbol).
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
}

#[test]
fn the_moved_functions_are_gone_and_nothing_draws_both_ways() {
    for (package, fns) in DELETED {
        let dir = repo().join("packages").join(package);
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "contract") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for f in fns {
                assert!(
                    !text.contains(&format!("{f}(")),
                    "{} still has {f}",
                    path.display()
                );
            }
            // Glyph went with its last user (ToolGlyph went with the
            // last path icons: library_last_path_icons.rs).
            assert!(
                !text.contains("component Glyph") && !text.contains(" Glyph("),
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn library_gate_keeps_every_moved_icon_its_path_the_same_d() {
    let plan = contract::compile_path(&repo().join("apps/library-gate/app.contract"))
        .unwrap_or_else(|e| panic!("{e}"));
    for (package, name, d) in MOVED {
        let full = format!("{package}.{name}");
        let row = plan
            .icons
            .iter()
            .find(|i| plan.str(i.name) == full)
            .unwrap_or_else(|| {
                panic!(
                    "no {full} in {:?}",
                    plan.icons
                        .iter()
                        .map(|i| plan.str(i.name))
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(icon_path(&plan, row), d, "{full}");
        assert_eq!(plan.str(row.sf), "", "{full}");
        assert_eq!(plan.str(row.view_box), "0 0 24 24", "{full}");
    }
}

/// A kept icon's path, read through `exact_runner::icon_of` (stage 14 C2:
/// the one reader of an `icons` row, which strips an even-odd icon's marker).
fn icon_path<'a>(plan: &'a exact_plan::Plan, row: &exact_plan::IconsRow) -> &'a str {
    exact_runner::icon_of(plan, row).map_or("", |icon| icon.path)
}
