//! Native Windows carrier over the shared native Contract/game presenter.
//! A native executable and lazily loaded GPU DLL; no browser or dev server.
#![deny(unsafe_code)]
#![deny(missing_docs)]

#[cfg(windows)]
mod window;

/// Run a Windows app with its baked plan and product identity.
pub fn run<D: exact_runner::DataSource + Default + 'static>(plan: &[u8], compat: &str) -> i32 {
    #[cfg(windows)]
    {
        window::run::<D>(plan, compat)
    }
    #[cfg(not(windows))]
    {
        let _ = (plan, compat);
        eprintln!("exact-windows requires a Windows target");
        1
    }
}
