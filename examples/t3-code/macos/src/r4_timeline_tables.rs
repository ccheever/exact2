// Chat Markdown tables and image chips (lane r4-timeline), included by markdown.rs.
// T3's MarkdownTable (ChatMarkdown.tsx, MIT, see LICENSE-T3) draws one table:
// cells of 12pt text padded 7.2/12 (8.8 in the header), a column as wide as its
// widest cell up to 24rem, and a footer with the expand toggle and the Copy as
// Markdown / CSV menu (markdown-clipboard.ts serializeTable / ToCsv). The parser
// emits one block per row; here a table's rows become one `table` block whose
// `columns` name, per column, the cell that sizes it (drawn invisibly in every
// row so the host's own text metrics set the width) and its share of any room.

/// Advance widths of the system UI font at 12pt (printable ASCII), as measured
/// in the reference's canvas (pages-text-width.ts R12); others count 6.9.
const R12: [f64; 95] = [
    3.38, 3.73, 5.73, 7.56, 7.56, 11.1, 8.54, 3.56, 4.58, 4.58, 5.66, 7.56, 3.56, 5.66, 3.56, 3.66,
    7.56, 5.57, 7.24, 7.52, 7.72, 7.42, 7.64, 6.83, 7.66, 7.64, 3.56, 3.56, 7.56, 7.56, 7.56, 6.15,
    11.02, 8.09, 7.89, 8.59, 8.72, 7.15, 6.87, 8.96, 8.91, 3.21, 6.46, 7.9, 6.81, 10.49, 8.91,
    9.26, 7.62, 9.26, 7.84, 7.65, 7.61, 8.85, 8.09, 11.61, 8.14, 7.86, 7.94, 4.58, 3.66, 4.58,
    7.56, 7.0, 6.0, 6.62, 7.37, 6.71, 7.37, 6.86, 4.34, 7.31, 7.06, 2.96, 2.96, 6.52, 3.04, 10.44,
    7.0, 7.09, 7.32, 7.31, 4.57, 6.28, 4.36, 7.0, 6.5, 9.29, 6.29, 6.52, 6.47, 4.58, 3.11, 4.58,
    7.56,
];
/// `.chat-markdown td` max-width: 24rem.
const CELL_CAP: f64 = 384.0;

/// A cell's max-content width estimate: 12pt text (semibold in the header), a
/// code span 12pt SF Mono in its 1pt-bordered 5.6pt-padded box, plus 24 padding.
fn cell_width(runs: &[markdown_parse::Run], header: bool) -> f64 {
    let mut width = 0.0;
    for run in runs {
        if run.code {
            width += run.text.chars().count() as f64 * 7.22 + 13.2;
            continue;
        }
        let bold = header || run.bold;
        let text: f64 = run
            .text
            .chars()
            .map(|c| {
                let code = c as u32;
                if (32..127).contains(&code) {
                    R12[(code - 32) as usize]
                } else {
                    6.9
                }
            })
            .sum();
        width += text * 0.99 * if bold { 1.045 } else { 1.0 };
    }
    width + 24.0
}

/// One cell as Markdown source: emphasis, code and links restored, pipes escaped.
fn cell_markdown(runs: &[markdown_parse::Run]) -> String {
    let mut out = String::new();
    for run in runs {
        let skill_source = run.href.strip_prefix("t3-skill:");
        let mut text = if run.code {
            format!("`{}`", run.text)
        } else {
            skill_source.unwrap_or(&run.text).to_string()
        };
        if run.italic && !run.code {
            text = format!("*{text}*");
        }
        if run.bold && !run.code {
            text = format!("**{text}**");
        }
        if !run.href.is_empty() && skill_source.is_none() {
            let href = run.href.strip_prefix(FILE_LINK).unwrap_or(&run.href);
            text = format!("[{text}]({href})");
        }
        out.push_str(&text);
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}
fn cell_plain(runs: &[markdown_parse::Run]) -> String {
    runs.iter()
        .map(|run| run.href.strip_prefix("t3-skill:").unwrap_or(&run.text))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
/// csvCell: quoted when it holds a quote, comma or newline.
fn csv_cell(value: &str) -> String {
    if value.contains(['"', ',', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// serializeTableElementToMarkdown and serializeTableElementToCsv over the parsed rows.
fn table_text(rows: &[&markdown_parse::Block]) -> (String, String) {
    let mut markdown = Vec::new();
    let mut csv = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if row.cells.is_empty() {
            continue;
        }
        markdown.push(format!(
            "| {} |",
            row.cells
                .iter()
                .map(|cell| cell_markdown(cell))
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        if index == 0 {
            markdown.push(format!("| {} |", vec!["---"; row.cells.len()].join(" | ")));
        }
        csv.push(
            row.cells
                .iter()
                .map(|cell| csv_cell(&cell_plain(cell)))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    (markdown.join("\n"), csv.join("\n"))
}

/// The `table` block for rows `rows` (first index `first`): `rows` holds each
/// row's cells, `columns` the sizing cell, share and alignment (r4_integrate_align.rs) of every column.
fn table_block(first: usize, gap: f64, rows: &[&markdown_parse::Block], align: &[&str]) -> Value {
    let columns = rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);
    let mut sizers = Vec::new();
    for column in 0..columns {
        let mut best: Option<(f64, &[markdown_parse::Run], bool)> = None;
        for row in rows {
            let Some(cell) = row.cells.get(column) else {
                continue;
            };
            let width = cell_width(cell, row.header);
            if best.is_none_or(|(widest, _, _)| width > widest) {
                best = Some((width, cell.as_slice(), row.header));
            }
        }
        let (width, runs, header) = best.unwrap_or((24.0, &[], false));
        let capped = width > CELL_CAP;
        let sizer = chat_runs(&Value::list(
            runs.iter()
                .enumerate()
                .map(|(i, run)| markdown_parse::value::run(i, run))
                .collect(),
        ));
        sizers.push(Value::record(vec![
            Value::str(&column.to_string()),
            sizer,
            Value::Bool(capped),
            Value::Bool(header),
            Value::Number(width.min(CELL_CAP)),
            Value::str(align.get(column).copied().unwrap_or("left")),
        ]));
    }
    let row_values = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let cells = (0..columns)
                .map(|column| {
                    let runs = row.cells.get(column).map(Vec::as_slice).unwrap_or(&[]);
                    Value::record(vec![
                        Value::str(&column.to_string()),
                        chat_runs(&Value::list(
                            runs.iter()
                                .enumerate()
                                .map(|(i, run)| markdown_parse::value::run(i, run))
                                .collect(),
                        )),
                    ])
                })
                .collect();
            Value::record(vec![
                Value::str(&index.to_string()),
                Value::Bool(row.header),
                Value::list(cells),
            ])
        })
        .collect();
    let (markdown, csv) = table_text(rows);
    Value::record(vec![
        Value::str(&first.to_string()),
        Value::str("table"),
        Value::Number(0.0),
        Value::str(""),
        Value::str(""),
        Value::str(""),
        Value::Bool(false),
        Value::list(Vec::new()),
        Value::list(Vec::new()),
        Value::Number(gap),
        Value::Bool(false),
        Value::list(row_values),
        Value::list(sizers),
        Value::str(&markdown),
        Value::str(&csv),
    ])
}

/// The fields every non-table block carries after `flow`: no rows, columns or text.
fn no_table(fields: &mut Vec<Value>) {
    fields.extend([
        Value::list(Vec::new()),
        Value::list(Vec::new()),
        Value::str(""),
        Value::str(""),
    ]);
}

/// `![name](t3-context://v1/image/…)`: the parser keeps inline images as bare
/// text, so an image reference loses its `!` and parses as the chip link it is.
fn image_chip_links(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains("![") || !text.contains("](t3-context://") {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("![") {
        let after = &rest[at + 2..];
        let is_chip = after.find(']').is_some_and(|close| {
            !after[..close].contains('\n') && after[close..].starts_with("](t3-context://")
        });
        out.push_str(&rest[..at]);
        out.push_str(if is_chip { "[" } else { "![" });
        rest = after;
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

/// A line of pipes is a table row only beside the `| --- |` rule that makes
/// it a table; any other is prose and keeps its line break.
fn table_lines(lines: &[&str]) -> Vec<bool> {
    let rule = |line: &str| {
        let line = line.trim();
        line.contains('|')
            && line.contains('-')
            && line
                .chars()
                .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
    };
    let mut table = vec![false; lines.len()];
    let mut index = 0;
    while index < lines.len() {
        if lines[index].contains('|') && lines.get(index + 1).is_some_and(|next| rule(next)) {
            let mut end = index + 2;
            while end < lines.len() && lines[end].contains('|') && !lines[end].trim().is_empty() {
                end += 1;
            }
            for flag in &mut table[index..end] {
                *flag = true;
            }
            index = end;
        } else {
            index += 1;
        }
    }
    table
}
