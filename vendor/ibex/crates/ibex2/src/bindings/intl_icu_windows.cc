// Binds the shims' icu.dll pointer table (intl_icu_windows.h).
//
// @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — Rust's `intl_os` loads
// System32's icu.dll by full path, checks GetModuleFileNameW, and only then
// passes the module here. This file resolves the pointers from exactly that
// module; nothing else in the process decides which icu.dll the shims call.

#define IBEX2_OS_ICU_BINDER
#include "intl_icu_windows.h"

#include <windows.h>

Ibex2OsIcu ibex2_os_icu_table{};

// Returns null after publishing a fully bound table, or the name of the
// first entry point `module` does not export, publishing nothing.
extern "C" const char *ibex2_intl_os_icu_bind(void *module) {
  Ibex2OsIcu bound{};
#define IBEX2_OS_ICU_BIND(name)                                        \
  bound.name = reinterpret_cast<decltype(bound.name)>(                 \
      GetProcAddress(static_cast<HMODULE>(module), #name));            \
  if (bound.name == nullptr) return #name;
  IBEX2_OS_ICU_ENTRY_POINTS(IBEX2_OS_ICU_BIND)
#undef IBEX2_OS_ICU_BIND
  ibex2_os_icu_table = bound;
  return nullptr;
}
