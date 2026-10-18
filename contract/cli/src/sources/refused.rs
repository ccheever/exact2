//! What the sources' syntax refused while the rest of them was still
//! checked (`contract_syntax::Partial`): one run reports a refused line and
//! what the later passes find in the declarations around it.

use crate::CompileError;

/// The syntax refusals of every source the loader read, and the names the
/// refused declarations and members declared.
#[derive(Debug, Default)]
pub(crate) struct Refused {
    pub(crate) errors: Vec<CompileError>,
    pub(crate) hidden: Vec<String>,
    /// A source whose declarations could not all be told apart: nothing after
    /// the syntax is checked.
    pub(crate) whole: bool,
}

impl Refused {
    /// `result`, after the syntax refusals when there are any: then a later
    /// pass's refusal is kept only when it names nothing refused, since one
    /// that does may say no more than that the thing is missing.
    pub(crate) fn join<T>(
        &self,
        result: Result<T, Vec<CompileError>>,
    ) -> Result<T, Vec<CompileError>> {
        if self.errors.is_empty() {
            return result;
        }
        let mut all = self.errors.clone();
        if !self.whole {
            let later = result.err().unwrap_or_default();
            all.extend(later.into_iter().filter(|e| !self.hides(e)));
        }
        Err(all)
    }

    /// Whether `e` names something refused.
    pub(crate) fn hides(&self, e: &CompileError) -> bool {
        self.hidden.iter().any(|name| mentions(&e.message, name))
    }
}

/// Whether `text` holds `name` as a whole word.
fn mentions(text: &str, name: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(name).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(word)
            && !text[i + name.len()..].chars().next().is_some_and(word)
    })
}

#[cfg(test)]
mod tests {
    use super::mentions;

    #[test]
    fn a_name_is_mentioned_as_a_whole_word() {
        assert!(mentions("unknown name `count`", "count"));
        assert!(mentions("count + 1", "count"));
        assert!(!mentions("`counter` is a number", "count"));
        assert!(!mentions("recount", "count"));
    }
}
