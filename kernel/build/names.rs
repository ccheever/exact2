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
