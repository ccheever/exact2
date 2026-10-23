// Pierre file-icon resolution for the Markdown module's file chips, as timeline-files.ts resolves
// (@pierre/trees createFileTreeIconResolver, Apache-2.0, with T3's T3_PIERRE_ICONS, MIT; see LICENSE-T3).
// Pierre's own tables are generated (data/pierre-icons.tsv, tools/generate/gen-icons.mjs), so they
// are parsed at run time from `crate::pierre_tables()`, which embeds the file (lib.rs).

/// T3_PIERRE_ICONS.byFileName: two remap a built-in icon to its shape without its colour.
const T3_PIERRE_BY_NAME: &[(&str, &str)] = &[
    ("package.json", "npm-plain"),
    ("tsconfig.json", "typescript-plain"),
    ("agents.md", "agents"),
    ("pnpm-lock.yaml", "pnpm"),
    ("pnpm-workspace.yaml", "pnpm"),
];

/// Pierre's file-name, complete-set extension and extension tables.
#[derive(Default)]
struct PierreTables {
    name: std::collections::HashMap<&'static str, &'static str>,
    complete: std::collections::HashMap<&'static str, &'static str>,
    extension: std::collections::HashMap<&'static str, &'static str>,
}

fn pierre_tables_parsed() -> &'static PierreTables {
    static TABLES: std::sync::OnceLock<PierreTables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let mut tables = PierreTables::default();
        // One row per entry: kind, key, token; `#` lines are the header.
        for row in crate::pierre_tables().lines().filter(|row| !row.starts_with('#')) {
            let mut fields = row.split('\t');
            let (Some(kind), Some(key), Some(token)) = (fields.next(), fields.next(), fields.next()) else {
                continue;
            };
            let table = match kind {
                "name" => &mut tables.name,
                "complete" => &mut tables.complete,
                "extension" => &mut tables.extension,
                _ => continue,
            };
            table.insert(key, token);
        }
        tables
    })
}

/// The Pierre icon token for a path ("default" when no specific icon exists).
fn pierre_icon_token(path: &str) -> &'static str {
    let base = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    if let Some((_, token)) = T3_PIERRE_BY_NAME.iter().find(|(name, _)| *name == base) {
        return token;
    }
    let tables = pierre_tables_parsed();
    if let Some(token) = tables.name.get(base.as_str()) {
        return token;
    }
    let parts: Vec<&str> = base.split('.').collect();
    for index in 1..parts.len() {
        let extension = parts[index..].join(".");
        if let Some(token) = tables
            .complete
            .get(extension.as_str())
            .or_else(|| tables.extension.get(extension.as_str()))
        {
            return token;
        }
    }
    "default"
}
