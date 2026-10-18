fn find_documents(documents: Vec<Value>, messages: &[Value], args: &[Value]) -> Value {
    let query = args.get(2).and_then(Value::as_str).unwrap_or("");
    let active = args.get(3).and_then(Value::as_str).unwrap_or("");
    let occurrence = match args.get(4) {
        Some(Value::Number(n)) if *n >= 0.0 => Some(*n as usize),
        _ => None,
    };
    let documents = documents
        .into_iter()
        .map(|document| {
            let id = match &document {
                Value::Record(fields) => fields[0].as_str().unwrap_or(""),
                _ => "",
            };
            let selected = if id == active { occurrence } else { None };
            let title = messages
                .iter()
                .find_map(|message| {
                    let Value::Record(fields) = message else {
                        return None;
                    };
                    (fields.first().and_then(Value::as_str) == Some(id)
                        && fields.get(1).and_then(Value::as_str) == Some("plan"))
                    .then(|| fields.get(2).and_then(Value::as_str).unwrap_or(""))
                })
                .unwrap_or("");
            let user = messages.iter().any(|message| {
                matches!(message, Value::Record(fields)
            if fields.first().and_then(Value::as_str) == Some(id)
                && fields.get(1).and_then(Value::as_str) == Some("user"))
            });
            let labels = match args.get(5) {
                Some(Value::List(groups)) => groups
                    .iter()
                    .find_map(|group| {
                        let Value::Record(fields) = group else {
                            return None;
                        };
                        if fields.first().and_then(Value::as_str)
                            != Some(id.strip_suffix("#preview").unwrap_or(id))
                        {
                            return None;
                        }
                        match fields.get(1) {
                            Some(Value::List(chips)) => Some(&chips[..]),
                            _ => None,
                        }
                    })
                    .unwrap_or(&[]),
                _ => &[],
            };
            find_document(document, query, selected, title, user, labels)
        })
        .collect();
    Value::record(vec![Value::list(documents)])
}

// Search offsets follow shared/threadSearch.ts: lowercase keeps each scalar's UTF-16 width.
fn find_fold(text: &str) -> Vec<u16> {
    text.chars()
        .flat_map(|character| {
            let lowered: String = character
                .to_lowercase()
                .map(|c| if c == 'ς' { 'σ' } else { c })
                .collect();
            let value = if lowered.encode_utf16().count() == character.len_utf16() {
                lowered
            } else {
                character.to_string()
            };
            value.encode_utf16().collect::<Vec<_>>()
        })
        .collect()
}
fn find_ranges(text: &str, query: &str) -> Vec<(usize, usize)> {
    let needle = find_fold(query.trim());
    if needle.is_empty() {
        return Vec::new();
    }
    let haystack = find_fold(text);
    let mut result = Vec::new();
    let mut at = 0;
    while at + needle.len() <= haystack.len() {
        if haystack[at..at + needle.len()] == needle {
            result.push((at, at + needle.len()));
            at += needle.len();
        } else {
            at += 1;
        }
    }
    result
}
fn find_parts(text: &str, at: usize, ranges: &[(usize, usize)], active: Option<usize>) -> Value {
    let units: Vec<u16> = text.encode_utf16().collect();
    let end = at + units.len();
    let mut cuts = vec![at, end];
    for &(a, b) in ranges {
        cuts.push(a.clamp(at, end));
        cuts.push(b.clamp(at, end));
    }
    cuts.sort_unstable();
    cuts.dedup();
    Value::list(
        cuts.windows(2)
            .enumerate()
            .map(|(index, cut)| {
                let marked = ranges.iter().position(|&(a, b)| cut[0] >= a && cut[1] <= b);
                Value::record(vec![
                    Value::str(&index.to_string()),
                    Value::str(&String::from_utf16_lossy(&units[cut[0] - at..cut[1] - at])),
                    Value::Number(if marked.is_some() && marked == active {
                        if marked.is_some_and(|index| cut[0] == ranges[index].0) {
                            3.0
                        } else {
                            2.0
                        }
                    } else if marked.is_some() {
                        1.0
                    } else {
                        0.0
                    }),
                ])
            })
            .collect(),
    )
}
fn composer_context_href(href: &str) -> bool {
    let Some((kind, id)) = href
        .strip_prefix("t3-context://v1/")
        .and_then(|rest| rest.split_once('/'))
    else {
        return false;
    };
    !kind.is_empty()
        && kind.len() <= 40
        && kind.as_bytes()[0].is_ascii_lowercase()
        && kind
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn find_run_text<'a>(fields: &'a [Value], labels: &'a [Value]) -> &'a str {
    let href = fields.get(5).and_then(Value::as_str).unwrap_or("");
    if href.starts_with("t3-inline-code:") || href.starts_with("t3-citation://") {
        if let Some(label) = labels.iter().find_map(|chip| {
            let Value::Record(fields) = chip else {
                return None;
            };
            (fields.get(1).and_then(Value::as_str) == Some(href))
                .then(|| fields.get(3).and_then(Value::as_str))
                .flatten()
        }) {
            return label;
        }
    }
    fields.get(1).and_then(Value::as_str).unwrap_or("")
}
fn searchable_run(fields: &[Value], user: bool) -> bool {
    let kind = fields.get(6).and_then(Value::as_str).unwrap_or("");
    let href = fields.get(5).and_then(Value::as_str).unwrap_or("");
    kind != "media" && kind != "break" && !(user && composer_context_href(href))
}
fn find_runs(
    value: &Value,
    query: &str,
    selected: Option<usize>,
    occurrence: &mut usize,
    user: bool,
    labels: &[Value],
) -> Value {
    let Value::List(runs) = value else {
        return value.clone();
    };
    let searchable =
        |run: &Value| matches!(run, Value::Record(fields) if searchable_run(fields, user));
    let mut result = Vec::with_capacity(runs.len());
    // Ignored chips and hard breaks end a searchable segment, while inline styles do not.
    for segment in runs.chunk_by(|left, right| searchable(left) == searchable(right)) {
        let text: String = segment
            .iter()
            .filter_map(|run| match run {
                Value::Record(fields) if searchable_run(fields, user) => {
                    Some(find_run_text(fields, labels))
                }
                _ => None,
            })
            .collect();
        let ranges = find_ranges(&text, query);
        let active = selected
            .and_then(|index| index.checked_sub(*occurrence))
            .filter(|index| *index < ranges.len());
        *occurrence += ranges.len();
        let mut at = 0;
        result.extend(segment.iter().map(|run| {
            let Value::Record(fields) = run else {
                return run.clone();
            };
            let mut fields = fields.to_vec();
            let text = find_run_text(&fields, labels);
            let parts = if searchable_run(&fields, user) {
                let parts = find_parts(text, at, &ranges, active);
                at += text.encode_utf16().count();
                parts
            } else {
                find_parts(text, 0, &[], None)
            };
            fields.push(parts);
            Value::record(fields)
        }));
    }
    Value::list(result)
}
fn find_cells(
    value: &Value,
    query: &str,
    selected: Option<usize>,
    occurrence: &mut usize,
    user: bool,
    labels: &[Value],
) -> Value {
    let Value::List(cells) = value else {
        return value.clone();
    };
    Value::list(
        cells
            .iter()
            .map(|cell| {
                let Value::Record(fields) = cell else {
                    return cell.clone();
                };
                let mut fields = fields.to_vec();
                fields[1] = find_runs(&fields[1], query, selected, occurrence, user, labels);
                Value::record(fields)
            })
            .collect(),
    )
}
/// Adds presentation spans without replacing any run id, link, citation offset or Markdown source.
fn find_document(
    value: Value,
    query: &str,
    selected: Option<usize>,
    title: &str,
    user: bool,
    labels: &[Value],
) -> Value {
    let Value::Record(fields) = value else {
        return value;
    };
    let mut fields = fields.to_vec();
    let Value::List(blocks) = &fields[1] else {
        return Value::record(fields);
    };
    let mut occurrence = find_ranges(title, query).len();
    fields[1] = Value::list(
        blocks
            .iter()
            .map(|block| {
                let Value::Record(fields) = block else {
                    return block.clone();
                };
                let mut fields = fields.to_vec();
                fields[7] = find_runs(&fields[7], query, selected, &mut occurrence, user, labels);
                fields[8] = find_cells(&fields[8], query, selected, &mut occurrence, user, labels);
                if let Value::List(rows) = &fields[11] {
                    fields[11] = Value::list(
                        rows.iter()
                            .map(|row| {
                                let Value::Record(cells) = row else {
                                    return row.clone();
                                };
                                let mut cells = cells.to_vec();
                                cells[2] = find_cells(
                                    &cells[2],
                                    query,
                                    selected,
                                    &mut occurrence,
                                    user,
                                    labels,
                                );
                                Value::record(cells)
                            })
                            .collect(),
                    );
                }
                // Column sizers duplicate table text invisibly and must retain the same run shape.
                if let Value::List(columns) = &fields[12] {
                    fields[12] = Value::list(
                        columns
                            .iter()
                            .map(|column| {
                                let Value::Record(column) = column else {
                                    return column.clone();
                                };
                                let mut column = column.to_vec();
                                column[1] = find_runs(&column[1], "", None, &mut 0, user, labels);
                                Value::record(column)
                            })
                            .collect(),
                    );
                }
                let code = fields[1].as_str() == Some("code");
                let text = fields[4].as_str().unwrap_or("");
                let ranges = if code {
                    find_ranges(text, query)
                } else {
                    Vec::new()
                };
                let active = selected
                    .and_then(|index| index.checked_sub(occurrence))
                    .filter(|index| *index < ranges.len());
                occurrence += ranges.len();
                fields.push(find_parts(text, 0, &ranges, active));
                Value::record(fields)
            })
            .collect(),
    );
    fields.push(find_parts(
        title,
        0,
        &find_ranges(title, query),
        selected.filter(|index| *index < find_ranges(title, query).len()),
    ));
    Value::record(fields)
}

#[cfg(test)]
mod find_tests {
    use super::*;
    #[test]
    fn stable_utf16_and_nonoverlapping_matches() {
        assert_eq!(find_ranges("😀Σςİ aaa", "σ"), vec![(2, 3), (3, 4)]);
        assert_eq!(find_ranges("aaaa", "aa"), vec![(0, 2), (2, 4)]);
        assert_eq!(find_ranges("İ", "i"), Vec::new());
    }
    #[test]
    fn prose_matches_cross_styles_without_changing_run_identity() {
        let doc = find_document(
            document(Value::str("entry"), "he**llo** world"),
            "hello",
            Some(0),
            "",
            false,
            &[],
        );
        let Value::Record(doc) = doc else { panic!() };
        let Value::List(blocks) = &doc[1] else {
            panic!()
        };
        let Value::Record(block) = &blocks[0] else {
            panic!()
        };
        let Value::List(runs) = &block[7] else {
            panic!()
        };
        assert_eq!(runs.len(), 3);
        for run in &runs[..2] {
            let Value::Record(run) = run else { panic!() };
            let Value::List(parts) = &run[10] else {
                panic!()
            };
            let Value::Record(part) = &parts[0] else {
                panic!()
            };
            assert!(matches!(part[2], Value::Number(2.0 | 3.0)));
        }
    }
    #[test]
    fn code_and_table_keep_complete_text_and_a_single_active_anchor() {
        fn anchors(value: &Value) -> usize {
            match value {
                Value::Record(fields)
                    if fields.len() == 3 && matches!(fields[2], Value::Number(3.0)) =>
                {
                    1
                }
                Value::Record(fields) | Value::List(fields) => fields.iter().map(anchors).sum(),
                _ => 0,
            }
        }
        for source in [
            "```ts\nconst needle = 1;\n```",
            "| Name | Value |\n|---|---|\n| needle | yes |",
        ] {
            let doc = find_document(
                document(Value::str("entry"), source),
                "needle",
                Some(0),
                "",
                false,
                &[],
            );
            assert_eq!(anchors(&doc), 1, "{source}");
            let Value::Record(fields) = &doc else {
                panic!()
            };
            assert_eq!(fields.len(), 3);
            let Value::List(blocks) = &fields[1] else {
                panic!()
            };
            assert!(blocks
                .iter()
                .all(|block| matches!(block, Value::Record(fields)
                    if fields.len() == 20 && matches!(fields[18], Value::Number(_))
                        && matches!(fields[19], Value::List(_)))));
        }
    }
    #[test]
    fn question_markdown_keeps_thread_links_and_find_positional_shapes() {
        let href = "t3-thread://v1/thread/thread-1";
        let result = Markdown
            .query(
                "renderQuestionMarkdown",
                &[Value::list(vec![Value::record(vec![
                    Value::str("question-1"),
                    Value::str("request-1"),
                    Value::str("Choose"),
                    Value::str(&format!("Review [Thread]({href})?")),
                ])])],
            )
            .unwrap();
        let Value::Record(result) = result else {
            panic!("transcript record")
        };
        let Value::List(documents) = &result[0] else {
            panic!("document list")
        };
        assert_eq!(documents.len(), 1);
        let Value::Record(document) = &documents[0] else {
            panic!("document")
        };
        assert_eq!(document.len(), 3);
        assert_eq!(document[0].as_str(), Some("question-1"));
        assert_eq!(document[2], Value::list(vec![]));
        let Value::List(blocks) = &document[1] else {
            panic!("blocks")
        };
        let Value::Record(block) = &blocks[0] else {
            panic!("block")
        };
        assert_eq!(block.len(), 20);
        assert!(matches!(block[18], Value::Number(_)));
        assert!(matches!(block[19], Value::List(_)));
        let Value::List(runs) = &block[7] else {
            panic!("runs")
        };
        assert!(runs.iter().all(|run| matches!(run, Value::Record(fields)
            if fields.len() == 11 && matches!(fields[10], Value::List(_)))));
        assert!(runs.iter().any(|run| matches!(run, Value::Record(fields)
            if fields[5].as_str() == Some(href) && fields[1].as_str() == Some("Thread"))));
    }
    #[test]
    fn plan_title_precedes_body_occurrences() {
        let doc = find_document(
            document(Value::str("entry"), "needle body"),
            "needle",
            Some(0),
            "needle plan",
            false,
            &[],
        );
        let Value::Record(fields) = doc else { panic!() };
        let Value::List(title) = &fields[2] else {
            panic!()
        };
        assert!(matches!(&title[0], Value::Record(part) if part[2] == Value::Number(3.0)));
    }
    fn rendered_find(source: &str, role: &str, selected: usize) -> Value {
        rendered_find_labels(source, role, selected, Vec::new())
    }
    fn rendered_find_labels(
        source: &str,
        role: &str,
        selected: usize,
        labels: Vec<Value>,
    ) -> Value {
        Markdown
            .query(
                "renderMarkdown",
                &[
                    Value::list(vec![Value::record(vec![
                        Value::str("entry"),
                        Value::str(role),
                        Value::str(""),
                        Value::str(source),
                    ])]),
                    Value::list(vec![]),
                    Value::str("needle"),
                    Value::str("entry"),
                    Value::Number(selected as f64),
                    Value::list(vec![Value::record(vec![
                        Value::str("entry"),
                        Value::list(labels),
                    ])]),
                ],
            )
            .unwrap()
    }
    fn marked_parts(value: &Value) -> Vec<(String, u8)> {
        match value {
            Value::Record(fields)
                if fields.len() == 3
                    && fields[1].as_str().is_some()
                    && matches!(fields[2], Value::Number(_)) =>
            {
                match (fields[1].as_str(), &fields[2]) {
                    (Some(text), Value::Number(mark)) if *mark >= 1.0 => {
                        vec![(text.to_string(), *mark as u8)]
                    }
                    _ => Vec::new(),
                }
            }
            Value::Record(fields) | Value::List(fields) => {
                fields.iter().flat_map(marked_parts).collect()
            }
            _ => Vec::new(),
        }
    }
    fn run_identity(value: &Value) -> Vec<(String, String, String, String)> {
        match value {
            Value::Record(fields) if fields.len() == 10 || fields.len() == 11 => vec![(
                fields[0].as_str().unwrap_or("").into(),
                fields[1].as_str().unwrap_or("").into(),
                fields[5].as_str().unwrap_or("").into(),
                fields[6].as_str().unwrap_or("").into(),
            )],
            Value::Record(fields) | Value::List(fields) => {
                fields.iter().flat_map(run_identity).collect()
            }
            _ => Vec::new(),
        }
    }
    #[test]
    fn ignored_user_context_ends_searchable_segments() {
        for chip in [
            "[Thread](t3-context://v1/thread/thread_t1)",
            "[file.ts](t3-context://v1/mention/mention_m1)",
            "[Skill](t3-context://v1/skill/skill_s1)",
        ] {
            let source = format!("need{chip}le needle");
            let doc = rendered_find(&source, "user", 0);
            assert_eq!(marked_parts(&doc), vec![("needle".into(), 3)], "{chip}");
            assert_eq!(
                run_identity(&doc),
                run_identity(&document(Value::str("entry"), &source))
            );
        }
    }
    #[test]
    fn occurrence_ordinals_continue_across_ignored_user_context() {
        let source = "needle[Thread](t3-context://v1/thread/thread_t1)needle";
        for selected in 0..2 {
            let doc = rendered_find(source, "user", selected);
            let marks = if selected == 0 {
                vec![3, 1]
            } else {
                vec![1, 3]
            };
            assert_eq!(
                marked_parts(&doc),
                vec![("needle".into(), marks[0]), ("needle".into(), marks[1])]
            );
        }
    }
    #[test]
    fn invalid_context_links_remain_searchable_user_text() {
        let source = "need[le](t3-context://v1/thread/not/valid) needle";
        assert_eq!(
            marked_parts(&rendered_find(source, "user", 0)),
            vec![("need".into(), 3), ("le".into(), 2), ("needle".into(), 1)]
        );
    }
    #[test]
    fn assistant_context_link_labels_remain_searchable() {
        let source = "need[le](t3-context://v1/thread/thread_t1) needle";
        assert_eq!(
            marked_parts(&rendered_find(source, "assistant", 0)),
            vec![("need".into(), 3), ("le".into(), 2), ("needle".into(), 1)]
        );
        assert_eq!(
            marked_parts(&rendered_find("[needle](t3-skill:example)", "assistant", 0)),
            vec![("needle".into(), 3)]
        );
    }
    #[test]
    fn displayed_chip_labels_define_occurrences_without_rewriting_runs() {
        let href = "t3-citation://source?text=raw";
        let source = format!("`src/needle.ts:7` [Assistant quote]({href}) needle");
        let original = rendered_find(&source, "assistant", 0);
        for selected in 0..3 {
            let label = |href: &str, text: &str| {
                Value::record(vec![
                    Value::str("chip"),
                    Value::str(href),
                    Value::str("link"),
                    Value::str(text),
                ])
            };
            let doc = rendered_find_labels(
                &source,
                "assistant",
                selected,
                vec![
                    label("t3-inline-code:src/needle.ts:7", "needle.ts · L7"),
                    label(href, "needle comment"),
                ],
            );
            let expected: Vec<_> = (0..3)
                .map(|index| ("needle".to_string(), if index == selected { 3 } else { 1 }))
                .collect();
            assert_eq!(marked_parts(&doc), expected);
            assert_eq!(run_identity(&doc), run_identity(&original));
        }
    }
}
