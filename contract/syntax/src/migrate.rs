//! Source-preserving edits for retired Contract spellings. The parser identifies
//! each construct; comments, literals and user-declared functions stay unchanged.

use crate::{Span, SyntaxError};

pub(crate) struct Edit {
    start: Span,
    end: Span,
    text: String,
}

impl Edit {
    pub(crate) fn new(start: Span, end: Span, text: impl Into<String>) -> Self {
        Self {
            start,
            end,
            text: text.into(),
        }
    }
}

/// Rewrite retired list types, free method calls, view keywords and operators. Other syntax
/// errors refuse the migration without changing the source.
pub fn migrate(src: &str) -> Result<String, Vec<SyntaxError>> {
    let (parsed, edits) = crate::parser::parsed_with_edits(src, 0);
    if let Err(partial) = parsed {
        let errors: Vec<_> = partial
            .errors
            .into_iter()
            .filter(|e| !is_retired(e.id))
            .collect();
        if !errors.is_empty() {
            return Err(errors);
        }
    }
    let mut starts = vec![0];
    starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
    let offset = |span: Span, end: bool| {
        starts[span.line as usize - 1] + (if end { span.end_col } else { span.col }) as usize - 1
    };
    let mut edits: Vec<_> = edits
        .into_iter()
        .map(|e| (offset(e.start, false), offset(e.end, true), e.text))
        .collect();
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut after = src.to_string();
    for (start, end, text) in edits.into_iter().rev() {
        after.replace_range(start..end, &text);
    }
    // Validate the rewritten spelling before allowing any file write.
    crate::parse(&after).map_err(|e| vec![e])?;
    Ok(after)
}

/// Whether a diagnostic names a retired spelling repaired by this migrator.
pub fn is_retired(id: &str) -> bool {
    matches!(
        id,
        "syntax-list-type"
            | "syntax-method-form"
            | "syntax-view-keyword"
            | "syntax-else-keyword"
            | "syntax-operator-spelling"
    )
}
