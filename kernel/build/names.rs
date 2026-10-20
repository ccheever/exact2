// Generated names: packed tables and `Debug` spelled from them.
/// Names by discriminant, packed: one string and each name's end offset.
fn packed_table(ty: &str, rows: &[(u64, &str)]) -> (String, Vec<usize>) {
    let max = rows.iter().map(|(id, _)| *id).max().unwrap_or(0) as usize;
    assert!(
        max < 2 * rows.len() + 8,
        "{ty}: discriminants too sparse to pack"
    );
    let mut names = vec![""; max + 1];
    for (id, name) in rows {
        assert!(
            name.is_ascii() && !name.contains(['"', '\\']),
            "{ty}: {name}"
        );
        assert!(
            names[*id as usize].is_empty(),
            "{ty}: discriminant {id} twice"
        );
        names[*id as usize] = name;
    }
    let (mut packed, mut ends) = (String::new(), Vec::new());
    for name in names {
        packed.push_str(name);
        ends.push(packed.len());
    }
    assert!(
        packed.len() <= usize::from(u16::MAX),
        "{ty}: names past u16 offsets"
    );
    (packed, ends)
}
/// `Debug` as `#[derive(Debug)]` writes a unit variant, its identifier,
/// spelled from `name()` so no enum carries a second table of names.
fn debug_from_name(w: &mut String, ty: &str, rows: &[(&str, &str)]) {
    let body = if rows.iter().all(|(ident, name)| ident == name) {
        "f.write_str(self.name())"
    } else {
        assert!(
            rows.iter().all(|(ident, name)| *ident == pascal(name)),
            "{ty}: a variant is spelled neither as its name nor as pascal(name)"
        );
        "debug_pascal(self.name(), f)"
    };
    writeln!(w, "impl ::core::fmt::Debug for {ty} {{").unwrap();
    writeln!(
        w,
        "    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{ {body} }}"
    )
    .unwrap();
    writeln!(w, "}}").unwrap();
}
/// @ref LLP 1116.007 D3.1 — every spelling a `symbol:` source answers to, with its role: a role,
/// Apple's name for it (dots read as dashes, as underscores do) and the table's aliases. A role and
/// its Apple name may coincide (`heart`); every other spelling names one role and is new, except
/// that two meanings may share one Apple glyph (`reorder` and `menu`, which Material draws apart):
/// its Apple spelling names the earlier row.
fn symbol_spellings(schema: &Schema) -> std::collections::BTreeMap<String, &str> {
    let mut spellings = std::collections::BTreeMap::new();
    for [role, _, _] in &schema.symbols {
        spellings.insert(role.clone(), role.as_str());
    }
    for [role, apple, _] in &schema.symbols {
        let key = apple.replace(['.', '_'], "-");
        match spellings.get(&key) {
            None => {
                spellings.insert(key, role.as_str());
            }
            Some(prior) if prior == role => {}
            Some(prior) => assert!(
                schema.symbols.iter().any(|row| row[0] == *prior && row[1] == *apple),
                "schema: symbol spelling `{key}` names both `{role}` and `{prior}`"
            ),
        }
    }
    for [alias, role] in &schema.symbol_aliases {
        assert!(
            alias.starts_with(|c: char| c.is_ascii_lowercase())
                && alias.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
            "schema: invalid symbol alias `{alias}`"
        );
        assert!(
            schema.symbols.iter().any(|row| row[0] == *role),
            "schema: symbol alias `{alias}`: `{role}` is no role"
        );
        assert!(
            spellings.insert(alias.clone(), role.as_str()).is_none(),
            "schema: symbol alias `{alias}` is already a spelling"
        );
    }
    spellings
}
/// @ref LLP 1116.007 D3.1 — `symbol_role`: a role by any of its spellings, as the lowering reads
/// a `symbol:` source.
fn write_symbol_role(w: &mut String, schema: &Schema) {
    writeln!(
        w,
        "/// A role by any of its spellings: its own name, Apple's name for it or a table alias (`.` and\n/// `_` read as `-`). `sf/<name>` is no role.\npub fn symbol_role(name: &str) -> Option<&'static str> {{ match name.replace(['.', '_'], \"-\").as_str() {{"
    )
    .unwrap();
    for (key, role) in symbol_spellings(schema) {
        writeln!(w, "{key:?} => Some({role:?}),").unwrap();
    }
    writeln!(w, "_ => None, }} }}").unwrap();
}
