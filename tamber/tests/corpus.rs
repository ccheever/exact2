//! Golden IR and refusals for the corpus under `tests/corpus/` (LLP 1121
//! §5's "golden IR and folded tables"). A `.tamber` that checks has a
//! `.json` beside it; one that is refused has a `.err`, one diagnostic a
//! line. `BLESS=1 cargo test -p exact-tamber` rewrites them.

use common::vocab;
use exact_tamber::check;

mod common;
use std::path::Path;

#[test]
fn corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let bless = std::env::var_os("BLESS").is_some();
    let vocab = vocab();
    let mut failures = Vec::new();
    let mut count = 0;
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for path in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "tamber"))
    {
        count += 1;
        let src = std::fs::read_to_string(path).unwrap();
        let (got, ext, other) = match check(&src, &vocab) {
            Ok(folding) => (folding.ir(), "json", "err"),
            Err(errors) => (
                errors.iter().map(|e| format!("{e}\n")).collect::<String>(),
                "err",
                "json",
            ),
        };
        let golden = path.with_extension(ext);
        if bless {
            std::fs::write(&golden, &got).unwrap();
            let _ = std::fs::remove_file(path.with_extension(other));
            continue;
        }
        match std::fs::read_to_string(&golden) {
            Ok(want) if want == got => {}
            Ok(want) => failures.push(format!(
                "{}:\n--- want\n{want}--- got\n{got}",
                golden.display()
            )),
            Err(_) => failures.push(format!("{} is missing; got\n{got}", golden.display())),
        }
    }
    assert!(count > 10, "the corpus is {count} files");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
