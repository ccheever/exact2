//! @fixture/vault's library, behind a C ABI as a package's prebuilt library is (LLP 1154 D3 "The library"): it
//! carries its own Rust runtime and its own SQLite (rusqlite's bundled one), and exports these three functions alone.
//! One caller at a time; nothing unwinds across the boundary (the release profile aborts on panic).

use rusqlite::Connection;
use std::ffi::{c_char, CStr};
use std::sync::Mutex;

/// What only this library answers.
const ANSWER: i32 = 4217;

/// The database `vault_open` opened, held until `vault_close`.
static OPEN: Mutex<Option<Connection>> = Mutex::new(None);

/// The library's own number, which nothing else knows.
#[no_mangle]
pub extern "C" fn vault_answer() -> i32 {
    ANSWER
}

/// Open the SQLite file at `path` (a NUL-terminated UTF-8 path) with this library's SQLite, write one row and keep it
/// open: the number of rows it then holds, or -1 when it could not.
///
/// # Safety
///
/// `path` is a NUL-terminated string that lives for the call.
#[no_mangle]
pub unsafe extern "C" fn vault_open(path: *const c_char) -> i32 {
    let Ok(path) = unsafe { CStr::from_ptr(path) }.to_str() else {
        return -1;
    };
    let opened = Connection::open(path).and_then(|db| {
        db.execute_batch("CREATE TABLE IF NOT EXISTS opened (at INTEGER); INSERT INTO opened VALUES (unixepoch());")?;
        let rows: i32 = db.query_row("SELECT count(*) FROM opened", [], |row| row.get(0))?;
        Ok((db, rows))
    });
    match opened {
        Ok((db, rows)) => {
            *OPEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(db);
            rows
        }
        Err(_) => -1,
    }
}

/// Close what `vault_open` opened.
#[no_mangle]
pub extern "C" fn vault_close() {
    OPEN.lock().unwrap_or_else(|e| e.into_inner()).take();
}
