// The Windows include for Ibex's ICU shims: the SDK's single <icu.h>, which
// declares the same unversioned ICU C API backed by the operating system's
// ICU (no bundled ICU code or data), plus the pointer table through which the
// shims call the entry points only icu.dll exports.
//
// @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — the stated floor is
// Windows 10 2004, so the shims see only the API that release exports.
//
// <icu.h> gates each declaration on the first Windows release that exported
// it (NTDDI_VERSION blocks). Pinning NTDDI_VERSION here, before any Windows
// header is read, makes a call to a newer entry point (for example the
// Windows 11 `unumf_resultAsValue`/`ufmtval_*`/`ucfpos_*`) a compile error in
// these files instead of a load-time import that would stop the process from
// starting on an older Windows. This header must be the first include of
// every shim translation unit; it overrides a global NTDDI_VERSION on purpose.
#pragma once

#if !defined(_WIN32)
#error "intl_icu_windows.h is the Windows-only ICU include"
#endif

#if defined(_SDKDDKVER_)
#error "include intl_icu_windows.h before any Windows SDK header"
#endif

#undef NTDDI_VERSION
#undef _WIN32_WINNT
#undef WINVER
// NTDDI_WIN10_VB (Windows 10 2004, build 19041): the first release whose
// icu.dll exports the `unumf_*` number formatter the shims use.
#define NTDDI_VERSION 0x0A000008
#define _WIN32_WINNT 0x0A00
#define WINVER 0x0A00

#include <sdkddkver.h>

static_assert(NTDDI_VERSION == NTDDI_WIN10_VB,
              "Ibex's Windows ICU shims are pinned to the Windows 10 2004 API");

#include <icu.h>

#include <cstdlib>

// @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — the probe is the binding.
//
// Everything else the shims call links against the frozen icuuc/icuin import
// libraries that the Hermes VM already imports at load time. The entry points
// below exist only in icu.dll, and nothing links against icu.dll: the shims
// call them through this table, which `ibex2_intl_os_icu_bind`
// (intl_icu_windows.cc) fills from System32's icu.dll, loaded by full path
// and verified by Rust's `intl_os` probe. A shim that calls another
// icu.dll-only function without adding it here fails to link.
#define IBEX2_OS_ICU_ENTRY_POINTS(X)      \
  X(unumf_openForSkeletonAndLocale)        \
  X(unumf_close)                           \
  X(unumf_openResult)                      \
  X(unumf_closeResult)                     \
  X(unumf_formatDouble)                    \
  X(unumf_formatDecimal)                   \
  X(unumf_resultToString)                  \
  X(unumf_resultGetAllFieldPositions)

// Pointer types come from the SDK declarations, so a signature cannot drift.
struct Ibex2OsIcu {
#define IBEX2_OS_ICU_POINTER(name) decltype(&::name) name;
  IBEX2_OS_ICU_ENTRY_POINTS(IBEX2_OS_ICU_POINTER)
#undef IBEX2_OS_ICU_POINTER
};

// Rust (`intl_os`): runs the one-time probe, which binds the table, and
// reports whether it succeeded.
extern "C" int ibex2_intl_os_icu_available();

// Written once by the binder, before the probe reports success.
extern Ibex2OsIcu ibex2_os_icu_table;

// The bound table. Every Intl host operation and every INTL install is
// already refused unless the probe passed; reaching a shim without it is a
// bug in those gates, so stop rather than call an unbound pointer.
inline const Ibex2OsIcu &ibex2_os_icu() {
  if (ibex2_intl_os_icu_available() == 0) std::abort();
  return ibex2_os_icu_table;
}

#if !defined(IBEX2_OS_ICU_BINDER)
// The shim sources keep calling ICU by name; on Windows these names call
// through the bound table.
#define unumf_openForSkeletonAndLocale \
  (ibex2_os_icu().unumf_openForSkeletonAndLocale)
#define unumf_close (ibex2_os_icu().unumf_close)
#define unumf_openResult (ibex2_os_icu().unumf_openResult)
#define unumf_closeResult (ibex2_os_icu().unumf_closeResult)
#define unumf_formatDouble (ibex2_os_icu().unumf_formatDouble)
#define unumf_formatDecimal (ibex2_os_icu().unumf_formatDecimal)
#define unumf_resultToString (ibex2_os_icu().unumf_resultToString)
#define unumf_resultGetAllFieldPositions \
  (ibex2_os_icu().unumf_resultGetAllFieldPositions)
#endif
