// Effect rows marked cold in schema.json share their initial values.
// A style allocates them only when written; clones remain independent on writes.

/// `ColdRows`, its initial values, and `Cold`, the pointer StyleProps holds.
fn emit_cold(w: &mut String, schema: &Schema) {
    let cold: Vec<&StyleRow> = schema.styles.iter().filter(|r| r.cold).collect();
    for row in &cold {
        assert!(!row.inherited && !row.rare, "schema.json: cold row {} overlaps inherited or rare storage", row.field);
    }
    writeln!(w, "/// The rows few nodes set ([`StyleProps::cold`]).").unwrap();
    writeln!(w, "#[derive(Debug, Clone, PartialEq)]").unwrap();
    writeln!(w, "pub struct ColdRows {{").unwrap();
    for row in &cold {
        writeln!(w, "    pub {}: {},", row.field, parse_codec(&row.codec).rust_type()).unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl Default for ColdRows {{").unwrap();
    writeln!(w, "    fn default() -> Self {{").unwrap();
    writeln!(w, "        ColdRows {{").unwrap();
    for row in &cold {
        let codec = parse_codec(&row.codec);
        let default = default_of(&codec, &row.default, &row.field, &schema.colors);
        writeln!(w, "            {}: {},", row.field, default).unwrap();
    }
    writeln!(w, "        }}\n    }}\n}}").unwrap();
    let mut mask = String::new();
    for row in &cold {
        write!(mask, "m.set(StyleId::{}); ", pascal(&row.field)).unwrap();
    }
    w.push_str(concat!(
        "static INITIAL_COLD: std::sync::LazyLock<ColdRows> = std::sync::LazyLock::new(ColdRows::default);\n",
        "/// The cold rows of a style: shared, and absent while every one holds its\n",
        "/// initial value. Reads see the initial values then; a write makes them\n",
        "/// the style's own.\n",
        "#[derive(Debug, Clone, Default)]\n",
        "pub struct Cold(Option<std::sync::Arc<ColdRows>>);\n",
        "impl Cold {\n",
        "    /// Whether no cold row was ever written here.\n",
        "    pub fn is_initial(&self) -> bool { self.0.is_none() }\n",
        "}\n",
        "impl std::ops::Deref for Cold {\n",
        "    type Target = ColdRows;\n",
        "    fn deref(&self) -> &ColdRows { self.0.as_deref().unwrap_or_else(|| &INITIAL_COLD) }\n",
        "}\n",
        "impl std::ops::DerefMut for Cold {\n",
        "    fn deref_mut(&mut self) -> &mut ColdRows {\n",
        "        std::sync::Arc::make_mut(self.0.get_or_insert_with(|| std::sync::Arc::new(ColdRows::default())))\n",
        "    }\n",
        "}\n",
        "impl PartialEq for Cold {\n",
        "    fn eq(&self, other: &Cold) -> bool {\n",
        "        match (&self.0, &other.0) {\n",
        "            (Some(a), Some(b)) if std::sync::Arc::ptr_eq(a, b) => true,\n",
        "            (None, None) => true,\n",
        "            _ => **self == **other,\n",
        "        }\n",
        "    }\n",
        "}\n",
    ));
    w.push_str(concat!(
        "impl std::ops::Deref for StyleProps { type Target = ColdRows; fn deref(&self) -> &ColdRows { &self.cold } }\n",
        "impl std::ops::DerefMut for StyleProps { fn deref_mut(&mut self) -> &mut ColdRows { &mut self.cold } }\n",
    ));
    writeln!(w, "impl StyleMask {{").unwrap();
    writeln!(w, "    /// The rows [`ColdRows`] holds.").unwrap();
    writeln!(w, "    pub fn cold() -> StyleMask {{ let mut m = StyleMask::EMPTY; {mask}m }}").unwrap();
    writeln!(w, "}}").unwrap();
}

/// `copy_rows`' cold rows: nothing to do, and no allocation, when neither
/// style holds any (both read the initial values).
fn emit_cold_copy(w: &mut String, schema: &Schema) {
    writeln!(
        w,
        "        if mask.intersects(StyleMask::cold()) && !(self.cold.is_initial() && from.cold.is_initial()) {{"
    )
    .unwrap();
    writeln!(w, "            let (to, from) = (&mut *self.cold, &*from.cold);").unwrap();
    for row in schema.styles.iter().filter(|r| r.cold) {
        let clone = if parse_codec(&row.codec).is_copy() { "" } else { ".clone()" };
        writeln!(
            w,
            "            if mask.has(StyleId::{id}) {{ to.{f} = from.{f}{clone}; }}",
            id = pascal(&row.field),
            f = row.field
        )
        .unwrap();
    }
    writeln!(w, "        }}").unwrap();
}
