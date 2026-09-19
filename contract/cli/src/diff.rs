//! A unified diff between two texts, for `contract fmt --check`: the
//! longest common subsequence over lines, hunks with three lines of context.

/// The unified diff of `before` and `after`, both named `path`; empty when
/// they are the same.
pub fn unified(path: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let ops = edits(&a, &b);
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    let context = 3usize;
    let mut i = 0;
    while i < ops.len() {
        if ops[i].0 == Op::Keep {
            i += 1;
            continue;
        }
        // A hunk: from `context` lines before the first change to `context`
        // lines after the last change that is within `2 * context` of it.
        let start = i.saturating_sub(context);
        let mut end = i;
        let mut last_change = i;
        while end < ops.len() {
            if ops[end].0 != Op::Keep {
                last_change = end;
            } else if end - last_change > 2 * context {
                break;
            }
            end += 1;
        }
        let end = (last_change + context + 1).min(ops.len());
        let (mut a_start, mut b_start) = (0usize, 0usize);
        for op in &ops[..start] {
            match op.0 {
                Op::Keep => {
                    a_start += 1;
                    b_start += 1;
                }
                Op::Remove => a_start += 1,
                Op::Add => b_start += 1,
            }
        }
        let a_len = ops[start..end].iter().filter(|op| op.0 != Op::Add).count();
        let b_len = ops[start..end]
            .iter()
            .filter(|op| op.0 != Op::Remove)
            .count();
        out.push_str(&format!(
            "@@ -{},{a_len} +{},{b_len} @@\n",
            a_start + 1,
            b_start + 1
        ));
        for (op, line) in &ops[start..end] {
            let sign = match op {
                Op::Keep => ' ',
                Op::Remove => '-',
                Op::Add => '+',
            };
            out.push(sign);
            out.push_str(line);
            out.push('\n');
        }
        i = end;
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Remove,
    Add,
}

/// The edit script from `a` to `b`, in order.
fn edits<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<(Op, &'a str)> {
    let (n, m) = (a.len(), b.len());
    // lcs[i][j]: the length of the longest common subsequence of a[i..] and b[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((Op::Keep, a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push((Op::Remove, a[i]));
            i += 1;
        } else {
            out.push((Op::Add, b[j]));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| (Op::Remove, *l)));
    out.extend(b[j..].iter().map(|l| (Op::Add, *l)));
    out
}

#[cfg(test)]
mod tests {
    use super::unified;

    #[test]
    fn a_changed_line_is_one_hunk_with_context() {
        let before = "a\nb\nc\nd\ne\nf\ng\n";
        let after = "a\nb\nc\nD\ne\nf\ng\n";
        assert_eq!(
            unified("x", before, after),
            "--- a/x\n+++ b/x\n@@ -1,7 +1,7 @@\n a\n b\n c\n-d\n+D\n e\n f\n g\n"
        );
        assert_eq!(unified("x", before, before), "");
    }
}
