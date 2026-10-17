//! `set-param` and `unset-param` (LLP 1121 D15, stage 7 B): the app's value
//! of a component's param is a line of the design's `params` block, written
//! as a token's, `when` lines under it. The inverse is the edit's preimage,
//! restored, as every edit's is.

use super::{line, refuse, refuse_at, same_conds, when_code, Doc};
use crate::{Design, Diagnostic, Op};

impl Doc {
    /// The `params` block's line, if the design has one.
    fn params_header(&self) -> Option<usize> {
        self.lines
            .iter()
            .position(|l| l.trim_end_matches('\r') == "params")
    }

    pub(super) fn set_param(
        &mut self,
        design: &Design,
        name: &str,
        when: &[(String, String)],
        value: &str,
    ) -> Result<Vec<Op>, Vec<Diagnostic>> {
        let entry = design.params.iter().find(|p| p.name.value == name);
        let Some(p) = entry else {
            if !when.is_empty() {
                return Err(refuse(format!(
                    "`{name}` has no line in the design's `params` block: set its value before one of its `when` lines"
                )));
            }
            let new = self.ended(line(2, &format!("{name} = {}", value.trim()), None));
            match self.params_header() {
                // After the block's last line.
                Some(header) => {
                    let last = design
                        .params
                        .iter()
                        .map(|p| p.whens.last().map_or(p.name.line, |w| w.line))
                        .max()
                        .map_or(header + 1, |l| l as usize);
                    self.lines.insert(last, new);
                }
                // A new block at the end, a blank line before it.
                None => {
                    while self.lines.last().is_some_and(|l| l.trim().is_empty()) {
                        self.lines.pop();
                    }
                    let blank = self.ended(String::new());
                    let header = self.ended("params".into());
                    self.lines.extend([blank, header, new]);
                    self.newline = true;
                }
            }
            return Ok(Vec::new());
        };
        if when.is_empty() {
            let (indent, code, trailing) = self.parts(p.name.line);
            let l = crate::lex::Line {
                indent,
                text: code,
                line: p.name.line,
                col: 1,
            };
            let new = crate::fmt::token_with(&l, value.trim());
            self.set(p.name.line, indent, &new, trailing.as_deref());
            return Ok(Vec::new());
        }
        if let Some(w) = p.whens.iter().find(|w| same_conds(&w.conds, when)) {
            let code = when_code(&w.conds, value.trim());
            let (indent, _, trailing) = self.parts(w.line);
            self.set(w.line, indent, &code, trailing.as_deref());
            return Ok(Vec::new());
        }
        let indent = match p.whens.first() {
            Some(w) => self.parts(w.line).0,
            None => self.parts(p.name.line).0 + 2,
        };
        let after = p.whens.last().map_or(p.name.line, |w| w.line);
        let new = self.ended(line(indent, &when_code(when, value.trim()), None));
        self.lines.insert(after as usize, new);
        Ok(Vec::new())
    }

    pub(super) fn unset_param(
        &mut self,
        design: &Design,
        name: &str,
        when: &[(String, String)],
    ) -> Result<Vec<Op>, Vec<Diagnostic>> {
        let p = design
            .params
            .iter()
            .find(|p| p.name.value == name)
            .ok_or_else(|| {
                refuse(format!(
                    "`{name}` has no line in the design's `params` block"
                ))
            })?;
        if !when.is_empty() {
            let w = p
                .whens
                .iter()
                .find(|w| same_conds(&w.conds, when))
                .ok_or_else(|| {
                    refuse_at(p.name.line, format!("`{name}` has no such `when` line"))
                })?;
            self.lines.remove(w.line as usize - 1);
            return Ok(Vec::new());
        }
        // The line and its `when` lines, the last first.
        let mut lines: Vec<u32> = std::iter::once(p.name.line)
            .chain(p.whens.iter().map(|w| w.line))
            .collect();
        lines.sort_unstable();
        for l in lines.into_iter().rev() {
            self.lines.remove(l as usize - 1);
        }
        // The block emptied: its header and the blank lines before it go.
        if design.params.len() == 1 {
            if let Some(header) = self.params_header() {
                let empty = self.lines[header + 1..]
                    .iter()
                    .take_while(|l| !l.trim().is_empty() && l.starts_with(' '))
                    .count()
                    == 0;
                if empty {
                    self.lines.remove(header);
                    // One blank line separates blocks: the one after it, or
                    // at the end, those before it.
                    if self.lines.get(header).is_some_and(|l| l.trim().is_empty()) {
                        self.lines.remove(header);
                    } else {
                        let mut at = header;
                        while at > 0 && self.lines[at - 1].trim().is_empty() {
                            self.lines.remove(at - 1);
                            at -= 1;
                        }
                    }
                }
            }
        }
        Ok(Vec::new())
    }
}
