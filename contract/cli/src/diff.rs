//! Unified formatter diffs with three context lines around each change.
//! Three-line anchors keep separated edits local without a quadratic edit table.

use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Clone, Copy)]
struct Change {
    a_start: usize,
    a_end: usize,
    b_start: usize,
    b_end: usize,
}

pub fn unified(path: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    // Terminators participate in equality; a final newline is a real change.
    let a: Vec<_> = before.split_inclusive('\n').collect();
    let b: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (a_end, b_end) = (a.len() - suffix, b.len() - suffix);
    let mut anchors: BTreeMap<[&str; 3], Vec<usize>> = BTreeMap::new();
    for (offset, lines) in b[prefix..b_end].windows(3).enumerate() {
        anchors
            .entry([lines[0], lines[1], lines[2]])
            .or_default()
            .push(prefix + offset);
    }
    let mut changes = Vec::new();
    let (mut x, mut y) = (prefix, prefix);
    while x < a_end || y < b_end {
        if x < a_end && y < b_end && a[x] == b[y] {
            x += 1;
            y += 1;
            continue;
        }
        let mut change = Change {
            a_start: x,
            b_start: y,
            a_end,
            b_end,
        };
        // Both cursors only advance. Repeated anchors are indexed once and
        // searched by position, rather than rescanning a repeated paragraph.
        for (offset, lines) in a[x..a_end].windows(3).enumerate() {
            if let Some(positions) = anchors.get(&[lines[0], lines[1], lines[2]]) {
                let at = positions.partition_point(|position| *position < y);
                if let Some(&next) = positions.get(at) {
                    change.a_end = x + offset;
                    change.b_end = next;
                    break;
                }
            }
        }
        x = change.a_end;
        y = change.b_end;
        changes.push(change);
    }
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    let mut first = 0;
    while first < changes.len() {
        let mut last = first;
        while last + 1 < changes.len() && changes[last + 1].a_start - changes[last].a_end <= 6 {
            last += 1;
        }
        let start = changes[first];
        let end = changes[last];
        let leading = start.a_start.min(3);
        let trailing = (a.len() - end.a_end).min(3);
        let a_start = start.a_start - leading;
        let b_start = start.b_start - leading;
        let a_count = end.a_end + trailing - a_start;
        let b_count = end.b_end + trailing - b_start;
        writeln!(
            out,
            "@@ -{},{a_count} +{},{b_count} @@",
            a_start + usize::from(a_count != 0),
            b_start + usize::from(b_count != 0)
        )
        .unwrap();
        let mut cursor = a_start;
        for change in &changes[first..=last] {
            for line in &a[cursor..change.a_start] {
                emit(&mut out, ' ', line);
            }
            for line in &a[change.a_start..change.a_end] {
                emit(&mut out, '-', line);
            }
            for line in &b[change.b_start..change.b_end] {
                emit(&mut out, '+', line);
            }
            cursor = change.a_end;
        }
        for line in &a[cursor..end.a_end + trailing] {
            emit(&mut out, ' ', line);
        }
        first = last + 1;
    }
    out
}

fn emit(out: &mut String, sign: char, line: &str) {
    out.push(sign);
    out.push_str(line);
    if !line.ends_with('\n') {
        out.push_str("\n\\ No newline at end of file\n");
    }
}

#[cfg(test)]
mod tests {
    use super::unified;

    #[test]
    fn separated_changes_keep_local_context_after_insertions() {
        let before = (0..20).map(|n| format!("row {n}\n")).collect::<String>();
        let after = before
            .replace("row 1\n", "new 1\nextra\n")
            .replace("row 18\n", "new 18\n");
        assert_eq!(
            unified("x", &before, &after),
            "--- a/x\n+++ b/x\n@@ -1,5 +1,6 @@\n row 0\n-row 1\n+new 1\n+extra\n row 2\n row 3\n row 4\n@@ -16,5 +17,5 @@\n row 15\n row 16\n row 17\n-row 18\n+new 18\n row 19\n"
        );
    }

    #[test]
    fn nearby_changes_share_context_and_repeated_lines_still_match() {
        let before = "a\nx\nx\nx\nx\nx\nx\nb\n";
        let after = "A\nx\nx\nx\nx\nx\nx\nB\n";
        assert_eq!(
            unified("x", before, after),
            "--- a/x\n+++ b/x\n@@ -1,8 +1,8 @@\n-a\n+A\n x\n x\n x\n x\n x\n x\n-b\n+B\n"
        );
    }

    #[test]
    fn eof_only_and_empty_file_changes_are_visible() {
        assert_eq!(
            unified("x", "a", "a\n"),
            "--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@\n-a\n\\ No newline at end of file\n+a\n"
        );
        assert_eq!(
            unified("x", "", "a\n"),
            "--- a/x\n+++ b/x\n@@ -0,0 +1,1 @@\n+a\n"
        );
        assert_eq!(
            unified("x", "a\n", ""),
            "--- a/x\n+++ b/x\n@@ -1,1 +0,0 @@\n-a\n"
        );
        assert_eq!(unified("x", "a\n", "a\n"), "");
    }
}
