// Round-4 integration, included by markdown.rs: GFM column alignment for chat tables.
// The portable parser ignores a table's `| :-- | --: | :-: |` rule, but T3's
// ChatMarkdown keeps it (remark-gfm's `align` on every th and td). The rules are
// read from the source in order, so the n-th table block takes the n-th rule;
// lines inside a fenced code block are skipped like the parser skips them.

/// Each table's column alignments ("left", "right" or "center"), in source order.
fn table_aligns(text: &str) -> Vec<Vec<&'static str>> {
    let lines: Vec<&str> = text.split('\n').collect();
    let rule = |line: &str| {
        let line = line.trim();
        line.contains('|') && line.contains('-') && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
    };
    let mut tables = Vec::new();
    let mut fence: Option<&str> = None;
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let marker = if trimmed.starts_with("```") { Some("```") } else if trimmed.starts_with("~~~") { Some("~~~") } else { None };
        if let Some(marker) = marker {
            fence = match fence { Some(open) if open == marker => None, None => Some(marker), other => other };
            index += 1;
            continue;
        }
        if fence.is_none() && lines[index].contains('|') && lines.get(index + 1).is_some_and(|next| rule(next)) {
            let cells: Vec<&str> = lines[index + 1].trim().trim_start_matches('|').trim_end_matches('|').split('|').collect();
            tables.push(cells.iter().map(|cell| {
                let cell = cell.trim();
                match (cell.starts_with(':'), cell.ends_with(':')) {
                    (true, true) => "center",
                    (false, true) => "right",
                    _ => "left",
                }
            }).collect());
            index += 2;
            while index < lines.len() && lines[index].contains('|') && !lines[index].trim().is_empty() { index += 1; }
            continue;
        }
        index += 1;
    }
    tables
}

#[cfg(test)]
mod r4_integrate_align_tests {
    use super::*;

    #[test]
    fn rules_give_each_table_its_column_alignment() {
        let text = "| a | b | c |\n|:-|-:|:-:|\n| 1 | 2 | 3 |\n\n```\n| x | y |\n|--:|---|\n```\n\n| d | e |\n|---|--:|\n| 4 | 5 |";
        assert_eq!(table_aligns(text), vec![vec!["left", "right", "center"], vec!["left", "right"]]);
    }
}
