//! What the tests share.

use exact_tamber::{Role, Vocab};

/// The colour roles, read from the kernel's schema as the compiler will.
pub fn vocab() -> Vocab {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../kernel/tables/schema.json")).unwrap();
    let mut roles = Vec::new();
    let mut system = true;
    for row in schema["colors"].as_array().unwrap() {
        let name = row[0].as_str().unwrap().to_string();
        // CSS's system colours come first, by their CSS names.
        system &= name.starts_with(|c: char| c.is_ascii_uppercase());
        let accent = matches!(name.as_str(), "AccentColor" | "LinkText")
            || (!system && row[2].as_str() == Some("@tint"));
        roles.push(Role {
            name,
            system,
            fallback: [
                row[4].as_str().unwrap().into(),
                row[5].as_str().unwrap().into(),
            ],
            // As the kernel's `roles::web_accent` reads the same row.
            accent,
        });
    }
    let named_lengths = Vocab::named_lengths(
        schema["textStyles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r[0].as_str().unwrap(), r[1].as_str().unwrap())),
        schema["lengths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap()),
    );
    let sizes = named_lengths
        .iter()
        .filter_map(|name| {
            use exact_kernel::style::relative::{parse, MEDIUM};
            let (unit, n) = parse(name)?;
            let px = f64::from(unit.basis(MEDIUM, MEDIUM, 1.0) * n);
            Some((name.clone(), px.to_string()))
        })
        .collect();
    Vocab {
        roles,
        named_lengths,
        sizes,
    }
}
