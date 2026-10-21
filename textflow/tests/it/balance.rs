use crate::support::prepare;
use exact_textflow::{balanced_width, Options, BALANCE_MAX_LINES};

// Eight units a character: "aaaa bb cc dd" greedy at 88 is "aaaa bb cc " /
// "dd"; balanced it is two lines of seven and five characters.
#[test]
fn balance_finds_the_narrowest_width_that_keeps_the_line_count() {
    let p = prepare("aaaa bb cc dd", Options::default());
    assert_eq!(p.count_lines(88.0), 2);
    let width = balanced_width(88.0, |w| p.count_lines(w)).unwrap();
    assert_eq!(width, 56.0, "\"aaaa bb\" is the widest line that keeps two");
    assert_eq!(p.count_lines(width), 2);
    assert_eq!(p.count_lines(width - 1.0 / 64.0), 3);
}

#[test]
fn one_line_and_more_than_six_stay_greedy() {
    let short = prepare("aa bb", Options::default());
    assert_eq!(balanced_width(88.0, |w| short.count_lines(w)), None);
    let words = ["aa"; 2 * (BALANCE_MAX_LINES + 1)].join(" ");
    let long = prepare(&words, Options::default());
    assert_eq!(long.count_lines(40.0), BALANCE_MAX_LINES + 1);
    assert_eq!(balanced_width(40.0, |w| long.count_lines(w)), None);
    let six = ["aa"; 2 * BALANCE_MAX_LINES].join(" ");
    let six = prepare(&six, Options::default());
    assert_eq!(six.count_lines(40.0), BALANCE_MAX_LINES);
    assert!(balanced_width(40.0, |w| six.count_lines(w)).is_some());
    assert_eq!(balanced_width(f32::INFINITY, |_| 2), None);
}
