fn validate(schema: &Schema) {
    let mut roles = BTreeSet::new();
    for [role, apple, path] in &schema.symbols {
        assert!(
            !role.is_empty() && role != "sf" && role.bytes().all(|c| c.is_ascii_lowercase() || c == b'-'),
            "schema: invalid symbol role"
        );
        assert!(roles.insert(role), "schema: duplicate symbol role {role}");
        assert!(
            !apple.is_empty() && !path.is_empty(),
            "schema: incomplete symbol {role}"
        );
        assert!(
            path.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b" .,-".contains(&c)),
            "schema: invalid symbol path {role}"
        );
        // A filled state is its own role, named as Apple names it, beside its outline.
        let base = role.strip_suffix("-fill");
        assert!(
            base.is_some() == apple.ends_with(".fill")
                && base.is_none_or(|base| schema.symbols.iter().any(|row| row[0] == base)),
            "schema: symbol {role}: `-fill` is exactly Apple's `.fill`, beside its role"
        );
    }
    let mut materials = BTreeSet::new();
    for [name, ios, macos, blur, saturate, ..] in &schema.materials {
        assert!(
            !name.is_empty() && name.bytes().all(|c| c.is_ascii_lowercase() || c == b'-'),
            "schema: invalid material name {name}"
        );
        assert!(materials.insert(name), "schema: duplicate material {name}");
        for platform in [ios, macos] {
            let apple = platform.strip_prefix('~').unwrap_or(platform);
            assert!(
                !apple.is_empty() && apple.bytes().all(|c| c.is_ascii_alphanumeric()),
                "schema: material {name}: an Apple name, or `~` and the one drawn instead"
            );
        }
        assert!(
            blur.parse::<u16>().is_ok() && saturate.parse::<u16>().is_ok(),
            "schema: material {name}: whole blur and saturate"
        );
    }
    assert_eq!(
        schema.schema_version, 1,
        "schema: unsupported schemaVersion"
    );
    let mut ids = BTreeSet::new();
    for (i, row) in schema.node_types.iter().enumerate() {
        assert_eq!(
            row.id as usize, i,
            "schema: node type ids must be contiguous from 0"
        );
        assert!(
            ids.insert(row.name.clone()),
            "schema: duplicate node type `{}`",
            row.name
        );
    }
    let mut prop_ids = BTreeSet::new();
    let mut prop_names = BTreeSet::new();
    for row in &schema.props {
        assert!(
            prop_ids.insert(row.id),
            "schema: duplicate prop id {}",
            row.id
        );
        assert!(
            prop_names.insert(row.name.clone()),
            "schema: duplicate prop `{}`",
            row.name
        );
        assert!(
            matches!(row.kind.as_str(), "str" | "bool" | "int" | "float"),
            "schema: prop `{}` has unknown kind `{}`",
            row.name,
            row.kind
        );
    }
    for (name, def) in &schema.enums {
        assert!(
            !def.values.is_empty(),
            "schema: enum `{name}` has no values"
        );
        assert!(def.values.len() <= 255, "schema: enum `{name}` exceeds u8");
        let set: BTreeSet<_> = def.values.iter().collect();
        assert_eq!(
            set.len(),
            def.values.len(),
            "schema: enum `{name}` has duplicate values"
        );
        assert!(
            def.values.contains(&def.default),
            "schema: enum `{name}` default is not a member"
        );
    }
    let mut fields = BTreeSet::new();
    for (i, row) in schema.styles.iter().enumerate() {
        assert_eq!(
            row.bit as usize, i,
            "schema: style bits must be contiguous from 0 (`{}`)",
            row.field
        );
        assert!(
            fields.insert(row.field.clone()),
            "schema: duplicate style field `{}`",
            row.field
        );
        let codec = parse_codec(&row.codec);
        if let Codec::Enum(name) = &codec {
            assert!(
                schema.enums.contains_key(name),
                "schema: style `{}` references unknown enum `{name}`",
                row.field
            );
        }
        if let Some(name) = &row.keywords {
            assert!(
                matches!(codec, Codec::U8) && row.default.as_f64() == Some(0.0),
                "schema: keywords apply to u8 rows defaulting to 0 (`{}`)",
                row.field
            );
            assert!(
                schema.enums.get(name).is_some_and(|e| e.values.len() <= 9),
                "schema: style `{}` keywords `{name}` must name an enum of at most 9 values",
                row.field
            );
        }
        let ends_ok = !row.ends || matches!(codec, Codec::Animations);
        assert!(
            ends_ok,
            "schema: ends applies to animations rows (`{}`)",
            row.field
        );
        if row.admits_auto {
            assert!(
                matches!(codec, Codec::Dimension),
                "schema: admitsAuto only applies to dimension rows (`{}`)",
                row.field
            );
        }
    }
    assert!(
        schema.styles.len() <= 64 * 4,
        "schema: more style rows than the mask can carry"
    );
    for row in &schema.styles {
        match parse_codec(&row.codec) {
            Codec::Enum(name) => match &row.default {
                serde_json::Value::Null => {}
                serde_json::Value::String(s) => assert!(
                    schema.enums[name.as_str()].values.contains(s),
                    "schema: style `{}` default `{s}` is not a member of enum `{name}`",
                    row.field
                ),
                _ => panic!(
                    "schema: style `{}` enum default must be a string",
                    row.field
                ),
            },
            Codec::Tracks | Codec::Placement => {
                assert!(
                    row.default.is_null(),
                    "schema: style `{}` cannot declare a default",
                    row.field
                )
            }
            _ => {}
        }
    }
    let mut op_ids = BTreeSet::new();
    let mut op_names = BTreeSet::new();
    for row in &schema.opcodes {
        assert!(row.id != 0, "schema: opcode 0 is reserved");
        assert!(op_ids.insert(row.id), "schema: duplicate opcode {}", row.id);
        assert!(
            op_names.insert(row.name.clone()),
            "schema: duplicate opcode `{}`",
            row.name
        );
    }
}
