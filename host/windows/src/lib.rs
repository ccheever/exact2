//! Native Windows carrier over the shared native Contract/game presenter.
//! A native executable and lazily loaded GPU DLL; no browser or dev server.
#![deny(unsafe_code)]
#![deny(missing_docs)]

#[cfg(windows)]
mod window;

/// Run a Windows app with its display name, baked plan and product identity.
pub fn run<D: exact_runner::DataSource + Default + 'static>(
    name: &str,
    plan: &[u8],
    compat: &str,
) -> i32 {
    #[cfg(windows)]
    {
        window::run::<D>(name, plan, compat, None)
    }
    #[cfg(not(windows))]
    {
        let _ = (name, plan, compat);
        refuse()
    }
}

/// [`run`] with the app's hatches (LLP 1075.003.000.001 §5): `H` is the one
/// type the app's `modules/linux` names (the trait is the shared
/// presenter's), `words` the hatch words `app.json` gives this platform.
/// The window's loop tells the window moments and its frame loop the ticks;
/// overlays and observed input go through the presenter. Nothing of `H` is
/// made before the first frame is presented.
pub fn run_with_hatches<
    D: exact_runner::DataSource + Default + 'static,
    H: exact_linux::Hatches,
>(
    name: &str,
    plan: &[u8],
    compat: &str,
    words: &'static [&'static str],
) -> i32 {
    #[cfg(windows)]
    {
        let hatches = exact_linux::hatches::install::<H>(words);
        window::run::<D>(name, plan, compat, Some(hatches))
    }
    #[cfg(not(windows))]
    {
        let _ = (name, plan, compat, words);
        refuse()
    }
}

#[cfg(not(windows))]
fn refuse() -> i32 {
    eprintln!("exact-windows requires a Windows target");
    1
}
