//! The walker's break opportunities against Chrome's, Safari's and Firefox's,
//! over excerpts of Pretext's corpora (MIT, © Pretext contributors).
//! `tests/corpus/record.mjs` recorded where each engine starts each line in a
//! box of width 0, where every opportunity is taken; the walker at width 0
//! takes each of its own. Against each engine it is given the Thai, Lao, Khmer
//! and Myanmar word boundaries that engine's `Intl.Segmenter` found, as the web
//! host would be, so each column is the web host's in that browser.
//! @ref LLP 1043 §4 C — agreement with the browser is the parity obligation.
use exact_textflow::{Cursor, Options, Prepared};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

const BREAKS: &str = include_str!("../corpus/breaks.txt");
const ENGINES: [&str; 3] = ["chrome", "webkit", "firefox"];

/// Per corpus and engine (Chrome, WebKit, Firefox): the engine's starts the
/// walker lacks, and the walker's the engine lacks. The walker follows Blink,
/// so Chrome's column is the one kept at zero; the others record where the
/// browsers disagree with it. The scorecard is exact, so any change updates it.
/// Chrome's five extras are compounds `Intl.Segmenter` splits and Chrome's
/// line breaker keeps whole (`วสันต|ฤดู`). WebKit (Safari 27) also breaks
/// after a closing `”` before Hangul or Myanmar (`못해!”|하고`). Firefox 156
/// breaks before a zero-width space rather than after it (most of Khmer's),
/// before Myanmar `။`, inside URLs after `/`, and never before small kana.
type Score = (usize, usize);
const SCORECARD: &[(&str, [Score; 3])] = &[
    ("mixed-app-text", [(0, 0), (0, 0), (5, 0)]),
    ("en-gatsby-opening", [(0, 0), (0, 0), (0, 0)]),
    ("ja-rashomon", [(0, 0), (0, 0), (0, 0)]),
    ("ja-kumo-no-ito", [(0, 0), (0, 0), (0, 53)]),
    ("ko-unsu-joh-eun-nal", [(0, 0), (8, 0), (1, 0)]),
    ("ko-sonagi", [(0, 0), (0, 0), (0, 0)]),
    ("zh-zhufu", [(0, 0), (0, 0), (0, 0)]),
    ("zh-guxiang", [(0, 0), (0, 0), (0, 0)]),
    ("th-nithan-vetal-story-1", [(0, 0), (0, 0), (2, 0)]),
    ("th-nithan-vetal-story-7", [(0, 2), (0, 0), (5, 0)]),
    ("my-cunning-heron-teacher", [(0, 2), (8, 0), (61, 0)]),
    (
        "my-bad-deeds-return-to-you-teacher",
        [(0, 1), (4, 0), (30, 0)],
    ),
    (
        "km-prachum-reuang-preng-khmer-volume-7-stories-1-10",
        [(0, 0), (0, 0), (430, 0)],
    ),
    ("ar-risalat-al-ghufran-part-1", [(0, 0), (0, 0), (0, 0)]),
    ("ar-al-bukhala", [(0, 0), (0, 0), (0, 0)]),
    ("hi-eidgah", [(0, 0), (0, 0), (0, 0)]),
    ("he-masaot-binyamin-metudela", [(0, 0), (0, 0), (0, 0)]),
    ("ur-chughd", [(0, 0), (0, 0), (0, 0)]),
];

struct Corpus<'a> {
    id: &'a str,
    paragraphs: Vec<Paragraph<'a>>,
}

struct Paragraph<'a> {
    text: &'a str,
    /// Offsets per key: an engine's starts, or `<engine>-words`.
    data: BTreeMap<&'a str, Vec<usize>>,
}

fn corpora() -> Vec<Corpus<'static>> {
    let mut result: Vec<Corpus> = Vec::new();
    for line in BREAKS
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        if let Some(header) = line.strip_prefix("@ ") {
            let id = header.split(' ').next().unwrap();
            result.push(Corpus {
                id,
                paragraphs: Vec::new(),
            });
        } else if let Some(entry) = line.strip_prefix("= ") {
            let mut fields = entry.split(' ');
            let key = fields.next().unwrap();
            let data = &mut result
                .last_mut()
                .unwrap()
                .paragraphs
                .last_mut()
                .unwrap()
                .data;
            let offsets = if entry.ends_with(" =") {
                // The same as Chrome's line of this kind.
                let chrome = key.replacen(key.split('-').next().unwrap(), "chrome", 1);
                data[chrome.as_str()].clone()
            } else {
                fields.map(|s| s.parse().unwrap()).collect()
            };
            data.insert(key, offsets);
        } else {
            let corpus = result.last_mut().unwrap();
            corpus.paragraphs.push(Paragraph {
                text: line,
                data: BTreeMap::new(),
            });
        }
    }
    result
}

/// The walker's line starts at width 0, as the recorder reads a browser's: the
/// first painted byte of every line with ink, after the first such line.
fn walker_starts(text: &str, words: &[usize]) -> BTreeSet<usize> {
    let mut measure = |r: std::ops::Range<usize>| r.len() as f32;
    let prepared = Prepared::with_words(text, Options::default(), words, &mut measure);
    let mut starts = Vec::new();
    let mut cursor = Cursor::default();
    while let Some(line) = prepared.next_line(cursor, 0.0) {
        let paint = prepared.paint_range(text, line.start.byte..line.end.byte);
        if !paint.is_empty() {
            starts.push(paint.start);
        }
        cursor = line.end;
    }
    starts.into_iter().skip(1).collect()
}

fn context(text: &str, at: usize) -> String {
    let before: String = text[..at]
        .chars()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let after: String = text[at..].chars().take(12).collect();
    format!("{before}|{after}").replace('\n', "⏎")
}

#[test]
fn walker_breaks_where_browsers_break() {
    let mut report = String::new();
    let mut scores = Vec::new();
    for corpus in corpora() {
        let mut row = [(0, 0); 3];
        for (engine, score) in ENGINES.iter().zip(&mut row) {
            let mut examples = Vec::new();
            let mut total = 0;
            for p in &corpus.paragraphs {
                let words = &p.data[format!("{engine}-words").as_str()];
                let theirs: BTreeSet<usize> = p.data[engine].iter().copied().collect();
                let ours = walker_starts(p.text, words);
                total += theirs.len();
                for &at in theirs.difference(&ours) {
                    score.0 += 1;
                    examples.push(format!("  missing {}", context(p.text, at)));
                }
                for &at in ours.difference(&theirs) {
                    score.1 += 1;
                    examples.push(format!("  extra   {}", context(p.text, at)));
                }
            }
            let (missing, extra) = *score;
            writeln!(
                report,
                "{} {engine}: {missing} missing, {extra} extra of {total}",
                corpus.id
            )
            .unwrap();
            for example in examples.iter().take(6) {
                writeln!(report, "{example}").unwrap();
            }
        }
        scores.push((corpus.id, row));
    }
    println!("{report}");
    assert_eq!(
        scores,
        SCORECARD.to_vec(),
        "the walker's agreement with the browsers moved; update SCORECARD\n{report}"
    );
}
