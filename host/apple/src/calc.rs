//! Native lengths use the kernel evaluator. Only authored expressions are
//! interned here; each Swift style owns and releases its environment snapshot.
use exact_kernel::style::Comparison;
use exact_kernel::Env;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct Lengths {
    trees: Vec<Comparison>,
    handles: HashMap<Comparison, u32>,
}
static LENGTHS: Mutex<Option<Lengths>> = Mutex::new(None);

/// A stable handle for an authored expression, independent of the viewport.
pub fn handle(c: Comparison) -> u32 {
    let mut lengths = LENGTHS.lock().unwrap_or_else(|e| e.into_inner());
    let lengths = lengths.get_or_insert_with(Lengths::default);
    if let Some(&n) = lengths.handles.get(&c) {
        return n;
    }
    let n = u32::try_from(lengths.trees.len() + 1).expect("calculation handle space");
    lengths.trees.push(c);
    lengths.handles.insert(c, n);
    n
}
fn tree(n: u32) -> Option<Comparison> {
    let lengths = LENGTHS.lock().unwrap_or_else(|e| e.into_inner());
    lengths
        .as_ref()?
        .trees
        .get(n.checked_sub(1)? as usize)
        .copied()
}

/// A calculation and the environment owned by its native style.
pub struct Context {
    calculation: Comparison,
    env: Env,
}
impl Context {
    /// Copy a serialized environment. See `exact_calc_create` in exact.h.
    pub fn new(handle: u32, values: &[f64]) -> Option<Box<Self>> {
        let calculation = tree(handle)?;
        if values.len() < 8
            || !(values.len() - 8).is_multiple_of(4)
            || !values
                .iter()
                .all(|v| v.is_finite() && (*v as f32).is_finite())
            || values[6..8]
                .iter()
                .any(|v| *v < 1.0 || *v > 255.0 || v.fract() != 0.0)
        {
            return None;
        }
        let mut env = Env::new(
            values[0] as f32,
            values[1] as f32,
            values[2] as f32,
            values[3] as f32,
        );
        env.viewport_width = values[4] as f32;
        env.viewport_height = values[5] as f32;
        env.cols = values[6] as u8;
        env.rows = values[7] as u8;
        env.segments = values[8..]
            .chunks_exact(4)
            .map(|r| {
                exact_kernel::style::env::Rect::new(
                    r[0] as f32,
                    r[1] as f32,
                    r[2] as f32,
                    r[3] as f32,
                )
            })
            .collect();
        env.segments_consistent()
            .then(|| Box::new(Self { calculation, env }))
    }
    /// Resolve without copying the environment or consulting the bridge's handle table.
    pub fn length(&self, basis: f64) -> f64 {
        f64::from(
            self.calculation
                .value(&self.env, Some(basis as f32))
                .unwrap_or(0.0),
        )
    }
}

/// Serialize the fields a calculation reads. Viewport units use the window's
/// dimensions even while the page's layout viewport is a sheet.
pub fn write(out: &mut String, c: Comparison, env: &Env) {
    use std::fmt::Write;
    let (width, height) = env
        .screen
        .unwrap_or((env.viewport_width, env.viewport_height));
    let _ = write!(
        out,
        "{{\"calc\":{},\"env\":[{},{},{},{},{},{},{},{}",
        handle(c),
        env.top,
        env.right,
        env.bottom,
        env.left,
        width,
        height,
        env.cols,
        env.rows
    );
    for r in &env.segments {
        let _ = write!(out, ",{},{},{},{}", r.x, r.y, r.width, r.height);
    }
    out.push_str("]}");
}

/// A calculation handle for CSS text; zero for invalid or compact lengths.
pub fn parse(text: &str) -> u32 {
    match exact_kernel::Dimension::parse_calc(text) {
        Some(exact_kernel::Dimension::Compare(c)) => handle(c),
        _ => 0,
    }
}

/// Export the calculation seam from the application's static archive.
#[macro_export]
macro_rules! calc_exports {
    () => {
        /// Copy a style's calculation context; null for invalid data.
        /// # Safety
        /// `values` holds `count` aligned doubles.
        #[no_mangle]
        pub unsafe extern "C" fn exact_calc_create(
            handle: u32,
            values: *const f64,
            count: usize,
        ) -> *mut ::std::ffi::c_void {
            if values.is_null() || count > 8 + 4 * 255 * 255 {
                return ::std::ptr::null_mut();
            }
            // SAFETY: the caller supplies count readable, aligned doubles.
            let values = unsafe { ::std::slice::from_raw_parts(values, count) };
            $crate::calc::Context::new(handle, values)
                .map_or(::std::ptr::null_mut(), |c| Box::into_raw(c).cast())
        }
        /// Resolve a style's calculation context.
        /// # Safety
        /// `context` is null or a live pointer returned by exact_calc_create.
        #[no_mangle]
        pub unsafe extern "C" fn exact_calc_resolve(
            context: *const ::std::ffi::c_void,
            basis: f64,
        ) -> f64 {
            // SAFETY: a non-null context is live and remains borrowed during this call.
            unsafe { context.cast::<$crate::calc::Context>().as_ref() }
                .map_or(0.0, |c| c.length(basis))
        }
        /// Release a style's calculation context.
        /// # Safety
        /// `context` is null or a live pointer from exact_calc_create, consumed once.
        #[no_mangle]
        pub unsafe extern "C" fn exact_calc_destroy(context: *mut ::std::ffi::c_void) {
            if !context.is_null() {
                // SAFETY: create allocated this Box and the caller transfers ownership.
                drop(unsafe { Box::from_raw(context.cast::<$crate::calc::Context>()) });
            }
        }
        /// Parse an authored calculation.
        /// # Safety
        /// `text` holds `len` bytes.
        #[no_mangle]
        pub unsafe extern "C" fn exact_calc_parse(text: *const u8, len: usize) -> u32 {
            if text.is_null() {
                return 0;
            }
            // SAFETY: the caller guarantees text holds len bytes.
            let bytes = unsafe { ::std::slice::from_raw_parts(text, len) };
            ::std::str::from_utf8(bytes).map_or(0, $crate::calc::parse)
        }
    };
}
#[cfg(test)]
mod tests {
    #[test]
    fn contexts_keep_their_own_environment_and_validate_the_wire() {
        let n = super::parse("min(50%, 10vw + env(safe-area-inset-top))");
        let first = super::Context::new(n, &[3., 0., 0., 0., 300., 600., 1., 1.]).unwrap();
        for width in 400..1400 {
            let other =
                super::Context::new(n, &[7., 0., 0., 0., width as f64, 600., 1., 1.]).unwrap();
            assert!((other.length(400.) - (width as f64 / 10. + 7.).min(200.)).abs() < 0.0001);
            assert_eq!(first.length(400.), 33.);
        }
        assert!(super::Context::new(0, &[0.; 8]).is_none());
        assert!(super::Context::new(n, &[0.; 7]).is_none());
        for bad in [0., 1.5, 256., f64::INFINITY, f64::NAN] {
            assert!(super::Context::new(n, &[0., 0., 0., 0., 300., 600., bad, 1.]).is_none());
        }
        assert!(super::Context::new(n, &[0., 0., 0., 0., 300., 600., 2., 1.]).is_none());
    }

    #[test]
    fn resizing_does_not_intern_environment_snapshots() {
        let exact_kernel::Dimension::Compare(c) =
            exact_kernel::Dimension::parse_calc("min(23%, 17vw)").unwrap()
        else {
            panic!("expected comparison");
        };
        let mut env = exact_kernel::Env::default();
        let first = super::handle(c);
        for width in 1..1000 {
            env.viewport_width = width as f32;
            let mut json = String::new();
            super::write(&mut json, c, &env);
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value["calc"], first);
        }
    }

    #[test]
    fn a_handle_resolves_against_its_basis_as_the_kernel_does() {
        let n = super::parse("min(50%, 30px)");
        assert_ne!(n, 0);
        assert_eq!(
            super::parse("min(50%, 30px)"),
            n,
            "an equal pair shares one"
        );
        let context = super::Context::new(n, &[0., 0., 0., 0., 0., 0., 1., 1.]).unwrap();
        assert_eq!(context.length(40.0), 20.0);
        assert_eq!(context.length(200.0), 30.0);
        assert_eq!(super::parse("10px"), 0);
    }
}
