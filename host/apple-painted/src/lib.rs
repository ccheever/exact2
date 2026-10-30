//! Mac benchmark carrier. @ref LLP 1149 S1.1, D12.
#![deny(unsafe_code)]

#[cfg(target_os = "macos")]
mod agent;
#[cfg(target_os = "macos")]
mod window;

/// Run a benchmark window over the app's baked plan and data source.
pub fn run<D: exact_runner::DataSource + Default + 'static>(
    name: &str,
    plan: &[u8],
    compat: &str,
) -> i32 {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "{name} — benchmark carrier (LLP 1149 S1.1)\n\
            Ink in a macOS CAMetalLayer; not a shipping Apple presenter.\n\
            EXACT_INK_DEVICE=metal|wgpu (default: metal)\n\
            EXACT_WINDOW_WIDTH / EXACT_WINDOW_HEIGHT: content size in points\n\
            EXACT_AGENT=1: agent JSON lines on stdin/stdout\n\
            EXACT_AGENT_SOCKET=<path>: agent JSON lines on a Unix socket\n\
            No IME, menus, dialogs or accessibility in this benchmark carrier."
        );
        return 0;
    }
    #[cfg(target_os = "macos")]
    {
        window::run::<D>(name, plan, compat)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (name, plan, compat);
        eprintln!("the benchmark carrier requires macOS");
        1
    }
}
