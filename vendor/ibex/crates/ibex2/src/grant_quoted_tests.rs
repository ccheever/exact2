use super::*;

#[test]
fn quoted_filesystem_targets_keep_spaces_quotes_and_backslashes_as_data() {
    for path in [
        "/plain",
        "/snow 雪/a b",
        "app:/data/a b",
        "/a\"quote/b\\slash",
        "/nonbreaking\u{a0}space",
    ] {
        let json = serde_json::to_string(path).unwrap();
        let set = GrantSet::parse(&format!("fs.read {json}\nfs.write {json}")).unwrap();
        assert!(set.permits(&Operation::FsRead { path: path.into() }));
        assert!(set.permits(&Operation::FsWrite {
            path: format!("{path}/child")
        }));
        assert!(!set.permits(&Operation::FsRead {
            path: format!("{path}-neighbor")
        }));
        let prefix = PathPrefix::new(path).unwrap();
        assert_eq!(
            GrantSet::parse(&format!("fs.read {prefix}")).unwrap(),
            GrantSet::none().with(Grant::FsRead(prefix))
        );
    }
}

#[test]
fn malformed_quoted_targets_refuse_the_entire_set() {
    for target in [
        r#""/unterminated"#,
        r#""/bad\x00""#,
        r#""/bad\uD800""#,
        r#""/valid" ""#,
        r#""/valid" /other"#,
        r#""/valid" # comment"#,
        r#""/valid" fs.write /"#,
        r#""/valid"\nfs.write /"#,
        r#""/escaped\nfs.write /""#,
        r#""/nul\u0000""#,
        r#""/tab\t""#,
        r#""/del\u007f""#,
        r#""/control\u0085""#,
        r#""relative""#,
        r#""/../escape""#,
        r#"["/path"]"#,
        r#"null"#,
        r#"42"#,
        "\"/literal\nfs.write /\"",
    ] {
        let result = GrantSet::parse(&format!("env.read PATH\nfs.read {target}\nfs.write /safe"));
        assert!(result.is_err(), "{target:?}");
    }
}

#[test]
fn quoting_is_limited_to_filesystem_grants_and_unquoted_grammar_is_unchanged() {
    assert_eq!(
        GrantSet::parse("fs.read /data").unwrap(),
        GrantSet::parse("fs.read \"/data\"").unwrap()
    );
    assert!(GrantSet::parse("fs.read /with space").is_err());
    assert!(GrantSet::parse("fs.write /data # comment").is_err());
    assert!(GrantSet::parse("sqlite.open \"app:/data/db\"").is_err());
    assert!(GrantSet::parse("net.fetch \"https://example.com\"").is_err());
    // An env token with quote characters was always a literal variable name,
    // not JSON syntax. This amendment does not change that legacy grammar.
    let set = GrantSet::parse("env.read \"PATH\"").unwrap();
    assert_eq!(set.readable_env(), vec!["\"PATH\""]);
    assert!(!set.permits(&Operation::EnvRead {
        name: "PATH".into()
    }));
}

#[test]
fn display_normalizes_namespaces_and_prints_one_target() {
    for (path, expected) in [
        ("//a///b/", "/a/b"),
        ("app:/data///x", "app:/data/x"),
        ("/a b", "\"/a b\""),
        ("/a\"b", "\"/a\\\"b\""),
        ("/a\\b", "\"/a\\\\b\""),
        ("/", "/"),
        ("app:/", "app:/"),
    ] {
        let prefix = PathPrefix::new(path).unwrap();
        assert_eq!(prefix.to_string(), expected);
        assert_eq!(
            GrantSet::parse(&format!("fs.write {prefix}")).unwrap(),
            GrantSet::none().with(Grant::FsWrite(prefix))
        );
    }
    // Legacy programmatic POSIX/app prefixes may hold control characters.
    // Display is unambiguous but the new quoted manifest grammar refuses them.
    let legacy = PathPrefix::new("/line\nbreak").unwrap();
    assert_eq!(legacy.to_string(), "\"/line\\nbreak\"");
    assert!(GrantSet::parse(&format!("fs.read {legacy}")).is_err());
}

#[cfg(windows)]
#[test]
fn quoted_drive_paths_round_trip_and_still_obey_native_name_rules() {
    for path in [
        r"C:\Users\Name With Spaces\café",
        r"\\?\c:\Users\Name With Spaces\雪",
        "c:/",
        "C:/plain",
    ] {
        let json = serde_json::to_string(path).unwrap();
        let prefix = PathPrefix::new(path).unwrap();
        let set = GrantSet::parse(&format!("fs.read {json}")).unwrap();
        assert_eq!(set, GrantSet::none().with(Grant::FsRead(prefix.clone())));
        assert_eq!(set, GrantSet::parse(&format!("fs.read {prefix}")).unwrap());
        assert!(set.permits(&Operation::FsRead { path: path.into() }));
    }
    assert_eq!(
        PathPrefix::new(r"c:\Users\With Space").unwrap().to_string(),
        "\"C:/Users/With Space\""
    );
    for path in [
        r"C:\name\with\..\parent",
        r#"C:\a"quote"#,
        r"C:\x:stream",
        "C:/bad\0",
        r"\\server\share",
        "C:/trailing ",
    ] {
        assert!(
            GrantSet::parse(&format!("fs.read {}", serde_json::to_string(path).unwrap())).is_err(),
            "{path:?}"
        );
    }
}
