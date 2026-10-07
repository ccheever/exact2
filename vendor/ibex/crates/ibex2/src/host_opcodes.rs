//! One declarative catalog for every externally reachable host operation.
//!
//! Dispatch enums and platform adapters name these constants instead of
//! repeating numbers. The catalog at the bottom is also the unbounded source
//! for the uniqueness test; the grouped JSI adapter's C++ entry-path literals
//! are parsed and compared with `JSI_SYNC_BINDINGS`.

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Owner {
    Inline,
    Async,
    IntlNumber,
    IntlCase,
    IntlDateTime,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Assignment {
    pub(crate) name: &'static str,
    pub(crate) op: u32,
    pub(crate) owner: Owner,
}

pub(crate) mod inline {
    pub(crate) const CONSOLE_LOG: u32 = 1;
    pub(crate) const CONSOLE_INFO: u32 = 2;
    pub(crate) const CONSOLE_DEBUG: u32 = 3;
    pub(crate) const CONSOLE_WARN: u32 = 4;
    pub(crate) const CONSOLE_ERROR: u32 = 5;
    pub(crate) const BTOA: u32 = 10;
    pub(crate) const ATOB: u32 = 11;
    pub(crate) const TEXT_ENCODE: u32 = 20;
    pub(crate) const TEXT_DECODE: u32 = 21;
    pub(crate) const TEXT_ENCODE_INTO: u32 = 22;
    pub(crate) const URL_SEARCH_PARAMS_NORMALIZE: u32 = 29;
    pub(crate) const URL_PARSE: u32 = 30;
    pub(crate) const URL_SEARCH_PARAMS_GET: u32 = 31;
    pub(crate) const URL_SET: u32 = 32;
    pub(crate) const URL_SEARCH_PARAMS_GET_ALL: u32 = 33;
    pub(crate) const URL_SEARCH_PARAMS_HAS: u32 = 34;
    pub(crate) const URL_SEARCH_PARAMS_SET: u32 = 35;
    pub(crate) const URL_SEARCH_PARAMS_APPEND: u32 = 36;
    pub(crate) const URL_SEARCH_PARAMS_DELETE: u32 = 37;
    pub(crate) const URL_SEARCH_PARAMS_SORT: u32 = 38;
    pub(crate) const URL_SEARCH_PARAMS_ENTRIES: u32 = 39;
    pub(crate) const HEADERS_NEW: u32 = 40;
    pub(crate) const HEADERS_APPEND: u32 = 41;
    pub(crate) const HEADERS_SET: u32 = 42;
    pub(crate) const HEADERS_GET: u32 = 43;
    pub(crate) const HEADERS_HAS: u32 = 44;
    pub(crate) const HEADERS_DELETE: u32 = 45;
    pub(crate) const HEADERS_COUNT: u32 = 46;
    pub(crate) const HEADERS_NAME_AT: u32 = 47;
    pub(crate) const HEADERS_VALUE_AT: u32 = 48;
    pub(crate) const HEADERS_VALID_NAME: u32 = 49;
    pub(crate) const HEADERS_VALID_VALUE: u32 = 50;
    pub(crate) const HEADERS_FREE: u32 = 51;
    pub(crate) const TIMER_SET: u32 = 60;
    pub(crate) const TIMER_SET_REPEATING: u32 = 61;
    pub(crate) const TIMER_CLEAR: u32 = 62;
    pub(crate) const PERFORMANCE_NOW: u32 = 63;
    pub(crate) const CRYPTO_RANDOM_UUID: u32 = 70;
    pub(crate) const CRYPTO_GET_RANDOM_VALUES: u32 = 71;
    pub(crate) const FETCH_CONTROL: u32 = 72;
    pub(crate) const MULTIPART_BOUNDARY: u32 = 73;
    pub(crate) const MULTIPART_ENCODE: u32 = 74;
    pub(crate) const SQLITE_RESULT: u32 = 80;
}

#[cfg_attr(
    not(all(feature = "intl", any(target_os = "linux", windows))),
    allow(dead_code)
)]
pub(crate) mod intl_number {
    pub(crate) const CREATE: u32 = 90;
    pub(crate) const FORMAT: u32 = 91;
    pub(crate) const FORMAT_PARTS: u32 = 92;
    pub(crate) const PART_TYPE: u32 = 93;
    pub(crate) const PART_VALUE: u32 = 94;
    pub(crate) const RESOLVED: u32 = 95;
    pub(crate) const SUPPORTED_LOCALES: u32 = 96;
    pub(crate) const CURRENCY_DIGITS: u32 = 98;
    pub(crate) const CANONICAL_LOCALE: u32 = 99;
}

#[cfg_attr(
    not(all(feature = "intl", any(target_os = "linux", windows))),
    allow(dead_code)
)]
pub(crate) mod intl_case {
    pub(crate) const MAP: u32 = 97;
}

/// Every Intl host operation, by name. `intl_ops_are_the_catalogs_intl_assignments`
/// keeps this equal to the catalog's `Owner::Intl*` entries, so a new Intl
/// operation cannot bypass the Windows ICU gate in `ibex2_host_call`.
#[cfg(any(test, all(feature = "intl", windows)))]
pub(crate) const INTL_OPS: &[u32] = &[
    intl_number::CREATE,
    intl_number::FORMAT,
    intl_number::FORMAT_PARTS,
    intl_number::PART_TYPE,
    intl_number::PART_VALUE,
    intl_number::RESOLVED,
    intl_number::SUPPORTED_LOCALES,
    intl_number::CURRENCY_DIGITS,
    intl_number::CANONICAL_LOCALE,
    intl_case::MAP,
    intl_datetime::CREATE,
    intl_datetime::FORMAT,
    intl_datetime::FORMAT_PARTS,
    intl_datetime::PART_TYPE,
    intl_datetime::PART_VALUE,
    intl_datetime::RESOLVED,
    intl_datetime::SUPPORTED_LOCALES,
    intl_datetime::CANONICAL_TIME_ZONE,
];

/// Whether `op` is one of the Intl families' operations.
#[cfg(all(feature = "intl", windows))]
pub(crate) fn is_intl(op: u32) -> bool {
    INTL_OPS.contains(&op)
}

#[cfg_attr(
    not(all(feature = "intl", any(target_os = "linux", windows))),
    allow(dead_code)
)]
pub(crate) mod intl_datetime {
    pub(crate) const CREATE: u32 = 130;
    pub(crate) const FORMAT: u32 = 131;
    pub(crate) const FORMAT_PARTS: u32 = 132;
    pub(crate) const PART_TYPE: u32 = 133;
    pub(crate) const PART_VALUE: u32 = 134;
    pub(crate) const RESOLVED: u32 = 135;
    pub(crate) const SUPPORTED_LOCALES: u32 = 136;
    pub(crate) const CANONICAL_TIME_ZONE: u32 = 137;
}

pub(crate) mod async_ops {
    pub(crate) const ECHO: u32 = 100;
    pub(crate) const FETCH: u32 = 101;
    pub(crate) const READ_BODY: u32 = 102;
    pub(crate) const FS_READ_FILE: u32 = 110;
    pub(crate) const FS_WRITE_FILE: u32 = 111;
    pub(crate) const FS_APPEND_FILE: u32 = 112;
    pub(crate) const FS_READ_DIR: u32 = 113;
    pub(crate) const FS_MKDIR: u32 = 114;
    pub(crate) const FS_REMOVE: u32 = 115;
    pub(crate) const FS_STAT: u32 = 116;
    pub(crate) const FS_RENAME: u32 = 117;
    pub(crate) const FS_COPY_FILE: u32 = 118;
    pub(crate) const FS_REALPATH: u32 = 119;
    pub(crate) const FS_ATOMIC_WRITE_FILE: u32 = 120;
}

pub(crate) mod sqlite_async {
    pub(crate) const OPEN: u32 = 150;
    pub(crate) const PREPARE: u32 = 151;
    pub(crate) const EXECUTE: u32 = 152;
    pub(crate) const QUERY: u32 = 153;
    pub(crate) const STATEMENT_EXECUTE: u32 = 154;
    pub(crate) const STATEMENT_QUERY: u32 = 155;
    pub(crate) const TRANSACTION: u32 = 156;
    pub(crate) const CLOSE: u32 = 157;
    pub(crate) const STATEMENT_CLOSE: u32 = 158;
}

pub(crate) mod subtle {
    pub(crate) const DIGEST: u32 = 160;
    pub(crate) const IMPORT_KEY: u32 = 161;
    pub(crate) const EXPORT_KEY: u32 = 162;
    pub(crate) const GENERATE_KEY: u32 = 163;
    pub(crate) const SIGN: u32 = 164;
    pub(crate) const VERIFY: u32 = 165;
    pub(crate) const ENCRYPT: u32 = 166;
    pub(crate) const DECRYPT: u32 = 167;
    pub(crate) const DERIVE_BITS: u32 = 168;
    pub(crate) const DERIVE_KEY: u32 = 169;
}

#[cfg(test)]
macro_rules! assignment {
    ($name:literal, $op:path, $owner:ident) => {
        Assignment {
            name: $name,
            op: $op,
            owner: Owner::$owner,
        }
    };
}

#[cfg(test)]
pub(crate) const ALL: &[Assignment] = &[
    assignment!("console.log", inline::CONSOLE_LOG, Inline),
    assignment!("console.info", inline::CONSOLE_INFO, Inline),
    assignment!("console.debug", inline::CONSOLE_DEBUG, Inline),
    assignment!("console.warn", inline::CONSOLE_WARN, Inline),
    assignment!("console.error", inline::CONSOLE_ERROR, Inline),
    assignment!("btoa", inline::BTOA, Inline),
    assignment!("atob", inline::ATOB, Inline),
    assignment!("text.encode", inline::TEXT_ENCODE, Inline),
    assignment!("text.decode", inline::TEXT_DECODE, Inline),
    assignment!("text.encodeInto", inline::TEXT_ENCODE_INTO, Inline),
    assignment!(
        "url.searchParams.normalize",
        inline::URL_SEARCH_PARAMS_NORMALIZE,
        Inline
    ),
    assignment!("url.parse", inline::URL_PARSE, Inline),
    assignment!(
        "url.searchParams.get",
        inline::URL_SEARCH_PARAMS_GET,
        Inline
    ),
    assignment!("url.set", inline::URL_SET, Inline),
    assignment!(
        "url.searchParams.getAll",
        inline::URL_SEARCH_PARAMS_GET_ALL,
        Inline
    ),
    assignment!(
        "url.searchParams.has",
        inline::URL_SEARCH_PARAMS_HAS,
        Inline
    ),
    assignment!(
        "url.searchParams.set",
        inline::URL_SEARCH_PARAMS_SET,
        Inline
    ),
    assignment!(
        "url.searchParams.append",
        inline::URL_SEARCH_PARAMS_APPEND,
        Inline
    ),
    assignment!(
        "url.searchParams.delete",
        inline::URL_SEARCH_PARAMS_DELETE,
        Inline
    ),
    assignment!(
        "url.searchParams.sort",
        inline::URL_SEARCH_PARAMS_SORT,
        Inline
    ),
    assignment!(
        "url.searchParams.entries",
        inline::URL_SEARCH_PARAMS_ENTRIES,
        Inline
    ),
    assignment!("headers.new", inline::HEADERS_NEW, Inline),
    assignment!("headers.append", inline::HEADERS_APPEND, Inline),
    assignment!("headers.set", inline::HEADERS_SET, Inline),
    assignment!("headers.get", inline::HEADERS_GET, Inline),
    assignment!("headers.has", inline::HEADERS_HAS, Inline),
    assignment!("headers.delete", inline::HEADERS_DELETE, Inline),
    assignment!("headers.count", inline::HEADERS_COUNT, Inline),
    assignment!("headers.nameAt", inline::HEADERS_NAME_AT, Inline),
    assignment!("headers.valueAt", inline::HEADERS_VALUE_AT, Inline),
    assignment!("headers.validName", inline::HEADERS_VALID_NAME, Inline),
    assignment!("headers.validValue", inline::HEADERS_VALID_VALUE, Inline),
    assignment!("headers.free", inline::HEADERS_FREE, Inline),
    assignment!("timer.set", inline::TIMER_SET, Inline),
    assignment!("timer.setRepeating", inline::TIMER_SET_REPEATING, Inline),
    assignment!("timer.clear", inline::TIMER_CLEAR, Inline),
    assignment!("performance.now", inline::PERFORMANCE_NOW, Inline),
    assignment!("crypto.randomUUID", inline::CRYPTO_RANDOM_UUID, Inline),
    assignment!(
        "crypto.getRandomValues",
        inline::CRYPTO_GET_RANDOM_VALUES,
        Inline
    ),
    assignment!("fetch.control", inline::FETCH_CONTROL, Inline),
    assignment!("multipart.boundary", inline::MULTIPART_BOUNDARY, Inline),
    assignment!("multipart.encode", inline::MULTIPART_ENCODE, Inline),
    assignment!("sqlite.result", inline::SQLITE_RESULT, Inline),
    assignment!("intl.number.create", intl_number::CREATE, IntlNumber),
    assignment!("intl.number.format", intl_number::FORMAT, IntlNumber),
    assignment!(
        "intl.number.formatParts",
        intl_number::FORMAT_PARTS,
        IntlNumber
    ),
    assignment!("intl.number.partType", intl_number::PART_TYPE, IntlNumber),
    assignment!("intl.number.partValue", intl_number::PART_VALUE, IntlNumber),
    assignment!("intl.number.resolved", intl_number::RESOLVED, IntlNumber),
    assignment!(
        "intl.number.supportedLocales",
        intl_number::SUPPORTED_LOCALES,
        IntlNumber
    ),
    assignment!("intl.case.map", intl_case::MAP, IntlCase),
    assignment!(
        "intl.number.currencyDigits",
        intl_number::CURRENCY_DIGITS,
        IntlNumber
    ),
    assignment!(
        "intl.number.canonicalLocale",
        intl_number::CANONICAL_LOCALE,
        IntlNumber
    ),
    assignment!("async.echo", async_ops::ECHO, Async),
    assignment!("fetch", async_ops::FETCH, Async),
    assignment!("response.read", async_ops::READ_BODY, Async),
    assignment!("fs.readFile", async_ops::FS_READ_FILE, Async),
    assignment!("fs.writeFile", async_ops::FS_WRITE_FILE, Async),
    assignment!("fs.appendFile", async_ops::FS_APPEND_FILE, Async),
    assignment!("fs.readdir", async_ops::FS_READ_DIR, Async),
    assignment!("fs.mkdir", async_ops::FS_MKDIR, Async),
    assignment!("fs.rm", async_ops::FS_REMOVE, Async),
    assignment!("fs.stat", async_ops::FS_STAT, Async),
    assignment!("fs.rename", async_ops::FS_RENAME, Async),
    assignment!("fs.copyFile", async_ops::FS_COPY_FILE, Async),
    assignment!("fs.realpath", async_ops::FS_REALPATH, Async),
    assignment!("fs.atomicWriteFile", async_ops::FS_ATOMIC_WRITE_FILE, Async),
    assignment!("intl.datetime.create", intl_datetime::CREATE, IntlDateTime),
    assignment!("intl.datetime.format", intl_datetime::FORMAT, IntlDateTime),
    assignment!(
        "intl.datetime.formatParts",
        intl_datetime::FORMAT_PARTS,
        IntlDateTime
    ),
    assignment!(
        "intl.datetime.partType",
        intl_datetime::PART_TYPE,
        IntlDateTime
    ),
    assignment!(
        "intl.datetime.partValue",
        intl_datetime::PART_VALUE,
        IntlDateTime
    ),
    assignment!(
        "intl.datetime.resolved",
        intl_datetime::RESOLVED,
        IntlDateTime
    ),
    assignment!(
        "intl.datetime.supportedLocales",
        intl_datetime::SUPPORTED_LOCALES,
        IntlDateTime
    ),
    assignment!(
        "intl.datetime.canonicalTimeZone",
        intl_datetime::CANONICAL_TIME_ZONE,
        IntlDateTime
    ),
    assignment!("sqlite.open", sqlite_async::OPEN, Async),
    assignment!("sqlite.prepare", sqlite_async::PREPARE, Async),
    assignment!("sqlite.execute", sqlite_async::EXECUTE, Async),
    assignment!("sqlite.query", sqlite_async::QUERY, Async),
    assignment!(
        "sqlite.statementExecute",
        sqlite_async::STATEMENT_EXECUTE,
        Async
    ),
    assignment!(
        "sqlite.statementQuery",
        sqlite_async::STATEMENT_QUERY,
        Async
    ),
    assignment!("sqlite.transaction", sqlite_async::TRANSACTION, Async),
    assignment!("sqlite.close", sqlite_async::CLOSE, Async),
    assignment!(
        "sqlite.statementClose",
        sqlite_async::STATEMENT_CLOSE,
        Async
    ),
    assignment!("subtle.digest", subtle::DIGEST, Inline),
    assignment!("subtle.importKey", subtle::IMPORT_KEY, Inline),
    assignment!("subtle.exportKey", subtle::EXPORT_KEY, Inline),
    assignment!("subtle.generateKey", subtle::GENERATE_KEY, Inline),
    assignment!("subtle.sign", subtle::SIGN, Inline),
    assignment!("subtle.verify", subtle::VERIFY, Inline),
    assignment!("subtle.encrypt", subtle::ENCRYPT, Inline),
    assignment!("subtle.decrypt", subtle::DECRYPT, Inline),
    assignment!("subtle.deriveBits", subtle::DERIVE_BITS, Inline),
    assignment!("subtle.deriveKey", subtle::DERIVE_KEY, Inline),
];

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct JsiBinding {
    pub(crate) target: &'static str,
    pub(crate) name: &'static str,
    pub(crate) op: u32,
}

#[cfg(test)]
macro_rules! binding {
    ($target:literal, $name:literal, $op:path) => {
        JsiBinding {
            target: $target,
            name: $name,
            op: $op,
        }
    };
}

#[cfg(test)]
pub(crate) const JSI_SYNC_BINDINGS: &[JsiBinding] = &[
    binding!("global", "__ibex2_random_uuid", inline::CRYPTO_RANDOM_UUID),
    binding!(
        "global",
        "__ibex2_get_random_values",
        inline::CRYPTO_GET_RANDOM_VALUES
    ),
    binding!("subtle", "digest", subtle::DIGEST),
    binding!("subtle", "importKey", subtle::IMPORT_KEY),
    binding!("subtle", "exportKey", subtle::EXPORT_KEY),
    binding!("subtle", "generateKey", subtle::GENERATE_KEY),
    binding!("subtle", "sign", subtle::SIGN),
    binding!("subtle", "verify", subtle::VERIFY),
    binding!("subtle", "encrypt", subtle::ENCRYPT),
    binding!("subtle", "decrypt", subtle::DECRYPT),
    binding!("subtle", "deriveBits", subtle::DERIVE_BITS),
    binding!("subtle", "deriveKey", subtle::DERIVE_KEY),
    binding!("console", "log", inline::CONSOLE_LOG),
    binding!("console", "info", inline::CONSOLE_INFO),
    binding!("console", "debug", inline::CONSOLE_DEBUG),
    binding!("console", "warn", inline::CONSOLE_WARN),
    binding!("console", "error", inline::CONSOLE_ERROR),
    binding!("global", "__ibex2_fetch_control", inline::FETCH_CONTROL),
    binding!(
        "global",
        "__ibex2_multipart_boundary",
        inline::MULTIPART_BOUNDARY
    ),
    binding!(
        "global",
        "__ibex2_multipart_encode",
        inline::MULTIPART_ENCODE
    ),
    binding!("global", "__ibex2_text_encode", inline::TEXT_ENCODE),
    binding!("global", "__ibex2_text_decode", inline::TEXT_DECODE),
    binding!(
        "global",
        "__ibex2_text_encode_into",
        inline::TEXT_ENCODE_INTO
    ),
    binding!("global", "__ibex2_url_parse", inline::URL_PARSE),
    binding!("global", "__ibex2_url_set", inline::URL_SET),
    binding!(
        "global",
        "__ibex2_search_params_normalize",
        inline::URL_SEARCH_PARAMS_NORMALIZE
    ),
    binding!(
        "global",
        "__ibex2_search_params_get",
        inline::URL_SEARCH_PARAMS_GET
    ),
    binding!(
        "global",
        "__ibex2_search_params_get_all",
        inline::URL_SEARCH_PARAMS_GET_ALL
    ),
    binding!(
        "global",
        "__ibex2_search_params_has",
        inline::URL_SEARCH_PARAMS_HAS
    ),
    binding!(
        "global",
        "__ibex2_search_params_set",
        inline::URL_SEARCH_PARAMS_SET
    ),
    binding!(
        "global",
        "__ibex2_search_params_append",
        inline::URL_SEARCH_PARAMS_APPEND
    ),
    binding!(
        "global",
        "__ibex2_search_params_delete",
        inline::URL_SEARCH_PARAMS_DELETE
    ),
    binding!(
        "global",
        "__ibex2_search_params_sort",
        inline::URL_SEARCH_PARAMS_SORT
    ),
    binding!(
        "global",
        "__ibex2_search_params_entries",
        inline::URL_SEARCH_PARAMS_ENTRIES
    ),
    binding!("headers", "create", inline::HEADERS_NEW),
    binding!("headers", "append", inline::HEADERS_APPEND),
    binding!("headers", "set", inline::HEADERS_SET),
    binding!("headers", "get", inline::HEADERS_GET),
    binding!("headers", "has", inline::HEADERS_HAS),
    binding!("headers", "remove", inline::HEADERS_DELETE),
    binding!("headers", "count", inline::HEADERS_COUNT),
    binding!("headers", "nameAt", inline::HEADERS_NAME_AT),
    binding!("headers", "valueAt", inline::HEADERS_VALUE_AT),
    binding!("headers", "validName", inline::HEADERS_VALID_NAME),
    binding!("headers", "validValue", inline::HEADERS_VALID_VALUE),
    binding!("headers", "free", inline::HEADERS_FREE),
    binding!("global", "__ibex2_timer_set", inline::TIMER_SET),
    binding!(
        "global",
        "__ibex2_timer_set_repeating",
        inline::TIMER_SET_REPEATING
    ),
    binding!("global", "__ibex2_timer_clear", inline::TIMER_CLEAR),
    binding!("global", "__ibex2_performance_now", inline::PERFORMANCE_NOW),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn intl_ops_are_the_catalogs_intl_assignments() {
        let catalog: BTreeSet<u32> = ALL
            .iter()
            .filter(|assignment| {
                matches!(
                    assignment.owner,
                    Owner::IntlNumber | Owner::IntlCase | Owner::IntlDateTime
                )
            })
            .map(|assignment| assignment.op)
            .collect();
        let listed: BTreeSet<u32> = INTL_OPS.iter().copied().collect();
        assert_eq!(listed.len(), INTL_OPS.len(), "INTL_OPS repeats an op");
        assert_eq!(listed, catalog);
    }
}
