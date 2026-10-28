//! The reference's code colours: @pierre/highlights 0.2.0's lexers (`highlights.wasm`, Apache-2.0; generated with
//! its licence beside it by tools/generate/gen-highlights.mjs), which the reference T3 Code highlights all code with.
//! Hermes runs no WebAssembly, so this crate runs them with wasmi, and the TypeScript module asks for their tokens
//! through the root (`highlightCode`; src/highlight/highlights.ts says how).
//!
//! The module's protocol is its own JavaScript's (dist/highlighter.js): the input's UTF-8 bytes at 64 KiB with a NUL
//! after them, the lexer id at byte 0, the output mode at byte 1 and the input's length at bytes 2-5; `highlight()`
//! then leaves the output's offset and length at bytes 6-13. Mode 3 is UTF-16 line records: pairs of a token's end
//! offset and its type (dist/token-types.js), and at each line break the next line's start offset and `u32::MAX`.

use exact_plan::Value;
use std::cell::RefCell;
use wasmi::{Caller, Engine, Linker, Memory, Module, Store, TypedFunc};

const LEXERS: &[u8] = include_bytes!("../highlights.wasm");
const PAGE: usize = 65_536;
/// The output mode that answers UTF-16 line records.
const LINE_RECORDS: u8 = 3;
/// The text one `highlightCode` answer tokenizes at most, in UTF-8 bytes: about 0.2 s of lexing at wasmi's
/// 2.4 MB/s (measured on a long TypeScript file); a request past it waits for the next answer.
pub const BATCH_BYTES: usize = 512 * 1024;

/// One instance of the lexers, made on this thread's first request.
struct Lexers {
    store: Store<()>,
    memory: Memory,
    highlight: TypedFunc<(), ()>,
}

thread_local! {
    static LEXERS_HERE: RefCell<Option<Lexers>> = const { RefCell::new(None) };
}

/// The module's two imports: whether the code point whose UTF-8 bytes are at `ptr` is Unicode ID_Start (`start`) or
/// ID_Continue, as its JavaScript answers with `\p{ID_Start}` and `[‌‍\p{ID_Continue}]`. The lexers ask
/// only for code points outside ASCII.
fn id_class(caller: &Caller<'_, ()>, ptr: i32, len: i32, start: bool) -> i32 {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        return 0;
    };
    let (ptr, len) = (ptr as usize, len as usize);
    let character = memory
        .data(caller)
        .get(ptr..ptr + len)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| text.chars().next());
    character.is_some_and(|c| {
        if start {
            is_id_start(c)
        } else {
            is_id_continue(c)
        }
    }) as i32
}

/// Unicode's ID_Start: letters, letter numbers and Other_ID_Start, less Pattern_Syntax.
fn is_id_start(c: char) -> bool {
    use unicode_general_category::GeneralCategory::*;
    matches!(
        c,
        '\u{1885}' | '\u{1886}' | '\u{2118}' | '\u{212E}' | '\u{309B}' | '\u{309C}'
    ) || c != '\u{2E2F}'
        && matches!(
            unicode_general_category::get_general_category(c),
            UppercaseLetter
                | LowercaseLetter
                | TitlecaseLetter
                | ModifierLetter
                | OtherLetter
                | LetterNumber
        )
}

/// Unicode's ID_Continue (with ZWNJ and ZWJ): ID_Start, marks, digits, connector punctuation and Other_ID_Continue.
fn is_id_continue(c: char) -> bool {
    use unicode_general_category::GeneralCategory::*;
    is_id_start(c)
        || matches!(
            c,
            '\u{00B7}' | '\u{0387}' | '\u{1369}'
                ..='\u{1371}' | '\u{19DA}' | '\u{200C}' | '\u{200D}' | '\u{30FB}' | '\u{FF65}'
        )
        || matches!(
            unicode_general_category::get_general_category(c),
            NonspacingMark | SpacingMark | DecimalNumber | ConnectorPunctuation
        )
}

impl Lexers {
    fn new() -> Result<Lexers, String> {
        // The lexers use the SIMD proposal, which wasmi's `simd` feature turns on (Cargo.toml).
        let engine = Engine::default();
        let module = Module::new(&engine, LEXERS).map_err(|error| error.to_string())?;
        let mut store = Store::new(&engine, ());
        let mut linker = Linker::<()>::new(&engine);
        linker
            .func_wrap(
                "env",
                "is_id_start",
                |caller: Caller<'_, ()>, ptr: i32, len: i32| id_class(&caller, ptr, len, true),
            )
            .and_then(|linker| {
                linker.func_wrap(
                    "env",
                    "is_id_continue",
                    |caller: Caller<'_, ()>, ptr: i32, len: i32| id_class(&caller, ptr, len, false),
                )
            })
            .map_err(|error| error.to_string())?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|error| error.to_string())?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or("the lexers export no memory")?;
        let highlight = instance
            .get_typed_func::<(), ()>(&store, "highlight")
            .map_err(|error| error.to_string())?;
        Ok(Lexers {
            store,
            memory,
            highlight,
        })
    }

    /// The UTF-16 line records of `text` under lexer `language`.
    fn records(&mut self, language: u8, text: &str) -> Result<Vec<u32>, String> {
        let bytes = text.as_bytes();
        // dist/highlighter.js #growMemoryIfNeeded: one page for the header, the input and 96 bytes of slack after it.
        let pages = 1 + (bytes.len() + 96).div_ceil(PAGE);
        let have = self.memory.data(&self.store).len() / PAGE;
        if pages > have {
            self.memory
                .grow(&mut self.store, (pages - have) as u64)
                .map_err(|error| error.to_string())?;
        }
        let data = self.memory.data_mut(&mut self.store);
        data[PAGE..PAGE + bytes.len()].copy_from_slice(bytes);
        data[PAGE + bytes.len()] = 0;
        data[0] = language;
        data[1] = LINE_RECORDS;
        data[2..6].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.highlight
            .call(&mut self.store, ())
            .map_err(|error| error.to_string())?;
        let data = self.memory.data(&self.store);
        let word =
            |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        let (start, length) = (word(6) as usize, word(10) as usize);
        let output = data
            .get(start..start + length)
            .ok_or("the lexers' output is outside their memory")?;
        Ok(output
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect())
    }
}

/// The UTF-16 line records of `text` under lexer `language` (src/highlight/highlights-tables.ts LEXER_IDS), made
/// on this thread's lexers; an error when they could not be made or the lexer trapped.
pub fn records(language: u8, text: &str) -> Result<Vec<u32>, String> {
    LEXERS_HERE.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            *cell = Some(Lexers::new()?);
        }
        let result = cell.as_mut().expect("made above").records(language, text);
        // A trap leaves the instance's state unknown: the next request makes a new one.
        if result.is_err() {
            *cell = None;
        }
        result
    })
}

/// `highlightCode(requests)`: each request (`id`, `language`, `text`) as its id and line records, in order, until
/// [`BATCH_BYTES`] of text are done; the requests after that are left out, and the TypeScript module asks again. A
/// request whose lexer failed answers no records (plain text).
pub fn highlight_code(args: &[Value]) -> Value {
    let mut results = Vec::new();
    let mut done = 0;
    if let Some(Value::List(requests)) = args.first() {
        for request in requests.iter() {
            let Value::Record(fields) = request else {
                continue;
            };
            let id = fields.first().and_then(Value::as_number).unwrap_or(0.0);
            let language = fields.get(1).and_then(Value::as_number).unwrap_or(0.0);
            let text = fields.get(2).and_then(Value::as_str).unwrap_or("");
            if done > 0 && done + text.len() > BATCH_BYTES {
                break;
            }
            done += text.len();
            let records = if (1.0..=255.0).contains(&language) {
                records(language as u8, text).unwrap_or_default()
            } else {
                Vec::new()
            };
            results.push(Value::record(vec![
                Value::Number(id),
                Value::list(
                    records
                        .into_iter()
                        .map(|record| Value::Number(record as f64))
                        .collect(),
                ),
            ]));
        }
    }
    Value::record(vec![Value::list(results)])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TypeScript's lexer id (dist/languages.js `ts`).
    const TS: u8 = 72;

    #[test]
    fn a_typescript_line_has_the_records_the_package_s_javascript_reads() {
        // What dist/highlighter.js answers in mode 3 for this text (its own WebAssembly run, bun 1.4.2).
        let text = "const x: number = 1; // hi\nfunction f(a) { return `s${a}`; }";
        assert_eq!(
            records(TS, text).unwrap(),
            [
                6,
                19,
                7,
                58,
                9,
                40,
                16,
                56,
                18,
                25,
                19,
                24,
                21,
                35,
                26,
                4,
                27,
                u32::MAX,
                36,
                19,
                37,
                14,
                38,
                32,
                39,
                59,
                43,
                32,
                50,
                18,
                52,
                44,
                54,
                40,
                55,
                58,
                56,
                40,
                57,
                44,
                59,
                35,
                60,
                32
            ]
        );
    }

    #[test]
    fn offsets_count_utf16_units_and_identifiers_outside_ascii_ask_the_unicode_imports() {
        // `é` is one UTF-16 unit and two UTF-8 bytes; `𝑥` is two units and four bytes. A token keeps the spaces after
        // it, and both identifiers are variables (type 58), which the lexer knows from the ID_Start import.
        let text = "let café = 1; let 𝑥 = 2;";
        let records = records(TS, text).unwrap();
        assert_eq!(records[..4], [4, 19, 9, 58]);
        assert_eq!(records[10..14], [18, 19, 21, 58]);
        assert_eq!(
            records[records.len() - 2],
            text.encode_utf16().count() as u32
        );
    }

    #[test]
    fn a_batch_stops_at_its_byte_budget_and_an_unknown_lexer_answers_plain() {
        let request = |id: f64, language: f64, text: &str| {
            Value::record(vec![
                Value::Number(id),
                Value::Number(language),
                Value::str(text),
            ])
        };
        let long = "x".repeat(BATCH_BYTES);
        let answer = highlight_code(&[Value::list(vec![
            request(1.0, TS as f64, "let a = 1;"),
            request(2.0, 0.0, "plain"),
            request(3.0, TS as f64, &long),
            request(4.0, TS as f64, "let b = 2;"),
        ])]);
        let Value::Record(fields) = answer else {
            panic!("a record")
        };
        let Some(Value::List(results)) = fields.first() else {
            panic!("a list")
        };
        let ids: Vec<f64> = results
            .iter()
            .map(|result| match result {
                Value::Record(fields) => fields[0].as_number().unwrap(),
                _ => panic!("a record"),
            })
            .collect();
        assert_eq!(ids, [1.0, 2.0]);
        let Value::Record(plain) = &results[1] else {
            panic!("a record")
        };
        assert_eq!(plain[1], Value::list(Vec::new()));
    }
}
