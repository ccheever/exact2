//! Optional Apple delivery composition (LLP 1030 D4).
//! Apps selecting L=A instantiate this adapter; L=0 instantiates `exact_apple::host!`.
#![deny(missing_docs)]
pub mod store;
/// Reexport for the app-entry macro.
pub use exact_apple;
/// The process store's operations, injected before the first boot.
pub static HOOKS: exact_apple::delivery::Hooks = exact_apple::delivery::Hooks {
    selected_plan: store::selected_plan,
    boot_started: store::boot_started,
    entry_refused: store::entry_refused,
    status_into: store::status_into,
    take_note: store::take_note,
    last_line: store::last_line,
};

/// Instantiate an app with the update adapter linked into its archive.
#[macro_export]
macro_rules! host {
    ($data:ty, $plan:expr, $compat:expr) => {
        mod exact_delivery_adapter {
            use super::*;
            extern "C" fn input(n: usize) -> *mut u8 {
                $crate::store::input(n)
            }
            extern "C" fn output() -> *const u8 {
                $crate::store::output_ptr()
            }
            extern "C" fn open(n: usize) -> u32 {
                $crate::store::open(n, $compat, $plan)
            }
            extern "C" fn select() -> u32 {
                $crate::store::select()
            }
            extern "C" fn boot_succeeded() {
                $crate::store::boot_succeeded()
            }
            extern "C" fn check(
                done: Option<$crate::exact_apple::delivery::DoneFn>,
                ctx: *mut ::std::ffi::c_void,
            ) -> u32 {
                $crate::store::check(done, ctx)
            }
            extern "C" fn activate() -> u32 {
                $crate::store::activate()
            }
            pub static API: $crate::exact_apple::delivery::Api =
                $crate::exact_apple::delivery::Api {
                    input,
                    output,
                    open,
                    select,
                    boot_succeeded,
                    check,
                    activate,
                };
        }
        $crate::exact_apple::host!(
            $data,
            $plan,
            $compat,
            Some(&$crate::HOOKS),
            &exact_delivery_adapter::API
        );
    };
}
