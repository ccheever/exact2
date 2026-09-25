//! The walker's break opportunities against Chrome's, over excerpts of
//! Pretext's corpora (MIT, © Pretext contributors). `tests/corpus/record.mjs`
//! recorded where Chrome starts each line in a box of width 0, where every
//! opportunity is taken; the walker at width 0 takes each of its own.
//! @ref LLP 1043 §4 C — agreement with the browser is the parity obligation.
use exact_textflow::{Cursor, Options, Prepared};
use std::collections::BTreeSet;
use std::fmt::Write;

const CHROME: &str = include_str!("../corpus/chrome.txt");

/// Per corpus: Chrome's starts the walker lacks, and the walker's Chrome lacks.
/// The scorecard is exact, so a change that moves either count updates it here.
const SCORECARD: &[(&str, usize, usize)] = &[
    ("mixed-app-text", 12, 4),
    ("en-gatsby-opening", 0, 0),
    ("ja-rashomon", 0, 0),
    ("ja-kumo-no-ito", 53, 0),
    ("ko-unsu-joh-eun-nal", 0, 1),
    ("ko-sonagi", 0, 0),
    ("zh-zhufu", 0, 0),
    ("zh-guxiang", 0, 0),
    ("th-nithan-vetal-story-1", 637, 0),
    ("th-nithan-vetal-story-7", 632, 0),
    ("my-cunning-heron-teacher", 490, 0),
    ("my-bad-deeds-return-to-you-teacher", 352, 0),
    (
        "km-prachum-reuang-preng-khmer-volume-7-stories-1-10",
        521,
        0,
    ),
    ("ar-risalat-al-ghufran-part-1", 0, 0),
    ("ar-al-bukhala", 0, 0),
    ("hi-eidgah", 0, 0),
    ("he-masaot-binyamin-metudela", 0, 0),
    ("ur-chughd", 0, 0),
];

struct Corpus<'a> {
    id: &'a str,
    paragraphs: Vec<(&'a str, BTreeSet<usize>)>,
}

fn corpora() -> Vec<Corpus<'static>> {
    let mut result: Vec<Corpus> = Vec::new();
    let mut lines = CHROME.lines().skip_while(|l| l.starts_with('#'));
    while let Some(line) = lines.next() {
        if let Some(header) = line.strip_prefix("@ ") {
            let id = header.split(' ').next().unwrap();
            result.push(Corpus {
                id,
                paragraphs: Vec::new(),
            });
            continue;
        }
        let starts = lines.next().expect("each paragraph has a starts line");
        let starts = starts
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect();
        result.last_mut().unwrap().paragraphs.push((line, starts));
    }
    result
}

/// The walker's line starts at width 0, as the recorder reads Chrome's: the
/// first painted byte of every line with ink, after the first such line.
fn walker_starts(text: &str) -> BTreeSet<usize> {
    let mut measure = |r: std::ops::Range<usize>| r.len() as f32;
    let prepared = Prepared::new(text, Options::default(), &mut measure);
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
fn walker_breaks_where_chrome_breaks() {
    let mut report = String::new();
    let mut scores = Vec::new();
    for corpus in corpora() {
        let (mut missing, mut extra) = (0, 0);
        let mut examples = Vec::new();
        for (text, chrome) in &corpus.paragraphs {
            let ours = walker_starts(text);
            for &at in chrome.difference(&ours) {
                missing += 1;
                examples.push(format!("  missing {}", context(text, at)));
            }
            for &at in ours.difference(chrome) {
                extra += 1;
                examples.push(format!("  extra   {}", context(text, at)));
            }
        }
        let total: usize = corpus.paragraphs.iter().map(|(_, s)| s.len()).sum();
        writeln!(
            report,
            "{}: {missing} missing, {extra} extra of {total}",
            corpus.id
        )
        .unwrap();
        for example in examples.iter().take(8) {
            writeln!(report, "{example}").unwrap();
        }
        scores.push((corpus.id, missing, extra));
    }
    println!("{report}");
    let expected: Vec<_> = SCORECARD.to_vec();
    assert_eq!(
        scores, expected,
        "the walker's agreement with Chrome moved; update SCORECARD\n{report}"
    );
}
