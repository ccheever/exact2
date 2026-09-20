//! A linear-space unified diff for an explicit formatter check. One hunk
//! covers the changed region, trimming unchanged edges to three context lines.

pub fn unified(path: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    // Include line terminators in equality: adding the final newline is a
    // real change, with the standard no-newline marker in the patch.
    let a: Vec<_> = before.split_inclusive('\n').collect();
    let b: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let start = prefix.saturating_sub(3);
    let context = suffix.min(3);
    let a_end = a.len() - suffix;
    let b_end = b.len() - suffix;
    let a_count = a_end + context - start;
    let b_count = b_end + context - start;
    let mut out = format!(
        "--- a/{path}\n+++ b/{path}\n@@ -{},{a_count} +{},{b_count} @@\n",
        start + usize::from(a_count != 0),
        start + usize::from(b_count != 0)
    );
    let mut emit = |sign, line: &str| {
        out.push(sign);
        out.push_str(line);
        if !line.ends_with('\n') {
            out.push_str("\n\\ No newline at end of file\n");
        }
    };
    for line in &a[start..prefix] {
        emit(' ', line);
    }
    for line in &a[prefix..a_end] {
        emit('-', line);
    }
    for line in &b[prefix..b_end] {
        emit('+', line);
    }
    for line in &a[a_end..a_end + context] {
        emit(' ', line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::unified;

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
