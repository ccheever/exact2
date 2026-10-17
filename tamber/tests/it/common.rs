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
        roles.push(Role {
            name,
            system,
            fallback: [
                row[4].as_str().unwrap().into(),
                row[5].as_str().unwrap().into(),
            ],
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
    Vocab {
        roles,
        named_lengths,
    }
}
