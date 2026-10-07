// Rows few nodes set (`"rare": true` in schema.json) live apart from the
// rest of StyleProps, behind one shared pointer: a style that sets none of
// them (most) holds no allocation for them, and copying or dropping a style
// copies a pointer instead of ~750 bytes and a dozen vectors. Reading a rare
// row reads the shared initial values while none is set; writing one makes
// the rows the style's own (copy on write). Nothing a reader sees changes.

/// Where `row` lives in StyleProps, as a path from the struct.
fn at(row: &StyleRow) -> String {
    if row.rare {
        format!("rare.{}", row.field)
    } else {
        row.field.clone()
    }
}

/// `RareRows`, its initial values, and `Rare`, the pointer StyleProps holds.
fn emit_rare(w: &mut String, schema: &Schema) {
    let rare: Vec<&StyleRow> = schema.styles.iter().filter(|r| r.rare).collect();
    for row in &rare {
        assert!(!row.inherited, "schema.json: rare row {} is inherited", row.field);
    }
    writeln!(w, "/// The rows few nodes set ([`StyleProps::rare`]).").unwrap();
    writeln!(w, "#[derive(Debug, Clone, PartialEq)]").unwrap();
    writeln!(w, "pub struct RareRows {{").unwrap();
    for row in &rare {
        writeln!(w, "    pub {}: {},", row.field, parse_codec(&row.codec).rust_type()).unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl Default for RareRows {{").unwrap();
    writeln!(w, "    fn default() -> Self {{").unwrap();
    writeln!(w, "        RareRows {{").unwrap();
    for row in &rare {
        let codec = parse_codec(&row.codec);
        let default = default_of(&codec, &row.default, &row.field, &schema.colors);
        writeln!(w, "            {}: {},", row.field, default).unwrap();
    }
    writeln!(w, "        }}\n    }}\n}}").unwrap();
    let mut mask = String::new();
    for row in &rare {
        write!(mask, "m.set(StyleId::{}); ", pascal(&row.field)).unwrap();
    }
    w.push_str(concat!(
        "static INITIAL_RARE: std::sync::LazyLock<RareRows> = std::sync::LazyLock::new(RareRows::default);\n",
        "/// The rare rows of a style: shared, and absent while every one holds its\n",
        "/// initial value. Reads see the initial values then; a write makes them\n",
        "/// the style's own.\n",
        "#[derive(Debug, Clone, Default)]\n",
        "pub struct Rare(Option<std::sync::Arc<RareRows>>);\n",
        "impl Rare {\n",
        "    /// Whether no rare row was ever written here.\n",
        "    pub fn is_initial(&self) -> bool { self.0.is_none() }\n",
        "}\n",
        "impl std::ops::Deref for Rare {\n",
        "    type Target = RareRows;\n",
        "    fn deref(&self) -> &RareRows { self.0.as_deref().unwrap_or(&INITIAL_RARE) }\n",
        "}\n",
        "impl std::ops::DerefMut for Rare {\n",
        "    fn deref_mut(&mut self) -> &mut RareRows {\n",
        "        std::sync::Arc::make_mut(self.0.get_or_insert_with(|| std::sync::Arc::new(RareRows::default())))\n",
        "    }\n",
        "}\n",
        "impl PartialEq for Rare {\n",
        "    fn eq(&self, other: &Rare) -> bool {\n",
        "        match (&self.0, &other.0) {\n",
        "            (Some(a), Some(b)) if std::sync::Arc::ptr_eq(a, b) => true,\n",
        "            (None, None) => true,\n",
        "            _ => **self == **other,\n",
        "        }\n",
        "    }\n",
        "}\n",
    ));
    writeln!(w, "impl StyleMask {{").unwrap();
    writeln!(w, "    /// The rows [`RareRows`] holds.").unwrap();
    writeln!(w, "    pub fn rare() -> StyleMask {{ let mut m = StyleMask::EMPTY; {mask}m }}").unwrap();
    writeln!(w, "}}").unwrap();
}

/// `copy_rows`' rare rows: nothing to do, and no allocation, when neither
/// style holds any (both read the initial values).
fn emit_rare_copy(w: &mut String, schema: &Schema) {
    writeln!(
        w,
        "        if mask.intersects(StyleMask::rare()) && !(self.rare.is_initial() && from.rare.is_initial()) {{"
    )
    .unwrap();
    writeln!(w, "            let (to, from) = (&mut *self.rare, &*from.rare);").unwrap();
    for row in schema.styles.iter().filter(|r| r.rare) {
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
