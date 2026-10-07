use std::path::{Path, PathBuf};

/// Resolve repository package tools without relying on Bun's platform-specific
/// `.bin` shims. Windows runs the upstream JavaScript entries through Bun.
pub(crate) fn package_tool(root: &Path, name: &str) -> PathBuf {
    package_tool_for(root, name, cfg!(windows))
}

fn package_tool_for(root: &Path, name: &str, windows: bool) -> PathBuf {
    if windows {
        root.join(match name {
            "tsc" => "node_modules/typescript/bin/tsc",
            "rolldown" => "node_modules/rolldown/bin/cli.mjs",
            _ => unreachable!("the TypeScript toolchain has only tsc and rolldown package tools"),
        })
    } else {
        root.join("node_modules/.bin").join(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_tools_use_scripts_on_windows_and_bin_entries_elsewhere() {
        let root = Path::new("sdk");
        assert_eq!(
            package_tool_for(root, "rolldown", true),
            root.join("node_modules/rolldown/bin/cli.mjs")
        );
        assert_eq!(
            package_tool_for(root, "tsc", true),
            root.join("node_modules/typescript/bin/tsc")
        );
        assert_eq!(
            package_tool_for(root, "rolldown", false),
            root.join("node_modules/.bin/rolldown")
        );
    }
}
