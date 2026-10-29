//! A component's members files (LLP 1091.002): each is loaded as a `use`d
//! file is, scoped through its own `use` lines with every member of the
//! component bound, and spliced into the component at its line.

use super::{name_error, Loader};
use crate::resolve::resolve;
use crate::{CompileError, RelatedLocation};
use contract_syntax::members::{declared, splice};
use contract_syntax::{File, Span};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// A `members` line and the file it loaded.
#[derive(Clone, Copy)]
pub(super) struct Part {
    /// The listing component, by its index in its file.
    component: usize,
    /// The line, by its index among the component's.
    line: usize,
    /// The members file's unit.
    unit: usize,
}

impl super::Unit {
    /// The units this one loads: its `use` lines' and its `members` lines'.
    pub(super) fn loads(&self) -> impl Iterator<Item = usize> + '_ {
        let parts = self.parts.iter().map(|p| p.unit);
        self.targets.iter().copied().chain(parts)
    }
}

impl Loader<'_> {
    /// Load the file each `members` line of unit `index` names (D1).
    pub(super) fn link_members(
        &mut self,
        path: &Path,
        index: usize,
    ) -> Result<(), Vec<CompileError>> {
        let components = &self.units[index].file.components;
        let lines: Vec<(usize, usize, String, Span)> = components
            .iter()
            .enumerate()
            .flat_map(|(c, component)| {
                let lines = component.members.lines.iter().enumerate();
                lines.map(move |(l, line)| (c, l, line.path.clone(), line.span))
            })
            .collect();
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let from = self.sources.origins[index].clone();
        for (component, line, written, span) in lines {
            let described = format!("`members \"{written}\"`");
            let relative = written.starts_with("./") || written.starts_with("../");
            if !relative || !written.ends_with(".contract") {
                let message = format!(
                    "{described}: a members file is a `.contract` file named by a relative path, \
                     `./` or `../`"
                );
                return Err(vec![name_error("contract-use-path", message, span)]);
            }
            let consulted = &mut self.sources.consulted;
            let resolved = resolve(&written, &dir, &from, self.app_root, consulted).map_err(
                |(id, message)| vec![name_error(id, format!("{described}: {message}"), span)],
            )?;
            let unit = self.load_resolved(resolved, &described, span)?;
            let part = Part {
                component,
                line,
                unit,
            };
            self.units[index].parts.push(part);
        }
        Ok(())
    }

    /// Every component's members files, by the listing unit and component.
    fn hosts(&self) -> BTreeMap<(usize, usize), Vec<Part>> {
        let mut hosts: BTreeMap<(usize, usize), Vec<Part>> = BTreeMap::new();
        for (index, unit) in self.units.iter().enumerate() {
            for part in &unit.parts {
                hosts
                    .entry((index, part.component))
                    .or_default()
                    .push(*part);
            }
        }
        hosts
    }

    /// Refuse a members file its line's component does not own (D2), and a
    /// member declared in two of a component's files (D5); then give each
    /// file of a component the members its other files declare, so scoping
    /// binds them.
    pub(super) fn prepare_members(&mut self) -> Result<(), Vec<CompileError>> {
        let mut errors = Vec::new();
        let mut listed: HashMap<usize, Span> = HashMap::new();
        for ((host, component), parts) in self.hosts() {
            let owner = &self.units[host].file.components[component];
            for part in parts {
                let span = owner.members.lines[part.line].span;
                let shown = self.shown(part.unit);
                if let Some(first) = listed.insert(part.unit, span) {
                    let message = format!(
                        "{shown} is already listed by the `members` line on line {} of {}: a \
                         members file belongs to one component, listed once",
                        first.line,
                        self.shown(first.source_id as usize)
                    );
                    errors.push(name_error("contract-members-twice", message, span));
                    continue;
                }
                let name = &owner.name;
                match &self.units[part.unit].file.members {
                    Some(block) if block.name == *name => {}
                    Some(block) => {
                        let message = format!(
                            "{shown} holds members of `{}`, but `{name}` lists it: its block is \
                             `members {name}`",
                            block.name
                        );
                        errors.push(name_error("contract-members-owner", message, block.span));
                    }
                    None => {
                        let message = format!(
                            "{shown} has no `members {name}` block: a members file declares \
                             the component's members under one"
                        );
                        errors.push(name_error("contract-members-owner", message, span));
                    }
                }
            }
        }
        for (index, unit) in self.units.iter().enumerate() {
            if let Some(block) = &unit.file.members {
                if !listed.contains_key(&index) {
                    let message = format!(
                        "no `members \"…\"` line in `{}` lists this file, so its members \
                         belong to no component: list it in the component's body",
                        block.name
                    );
                    errors.push(name_error("contract-members-unlisted", message, block.span));
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        for ((host, component), parts) in self.hosts() {
            self.share_members(host, component, &parts)
                .map_err(|e| vec![e])?;
        }
        Ok(())
    }

    /// Give a component and each of its members blocks the names the others
    /// declare, refusing one declared in two of its files.
    fn share_members(
        &mut self,
        host: usize,
        component: usize,
        parts: &[Part],
    ) -> Result<(), CompileError> {
        // Each file's names: the component's own first, then each block's.
        let mut files: Vec<Vec<(String, bool, Span)>> = Vec::new();
        let own = &self.units[host].file.components[component];
        let owned = |(name, callable, span): (&str, bool, Span)| (name.to_owned(), callable, span);
        files.push(declared(own).map(owned).collect());
        for part in parts {
            let block = self.units[part.unit].file.members.as_ref();
            let block = block.expect("prepare_members checked every block");
            files.push(declared(block).map(owned).collect());
        }
        let mut first: HashMap<&str, Span> = HashMap::new();
        for (name, _, span) in files.iter().flatten() {
            match first.get(name.as_str()) {
                // One file's two declarations are the type checker's to refuse.
                Some(prior) if prior.source_id != span.source_id => {
                    let message = format!(
                        "`{name}` is declared in {} and in {}: a component's members are one \
                         set of names across its files; rename one",
                        self.shown(prior.source_id as usize),
                        self.shown(span.source_id as usize)
                    );
                    let mut error = name_error("contract-members-duplicate", message, *span);
                    error.related = Box::new([RelatedLocation {
                        span: *prior,
                        file: None,
                        note: format!("`{name}` is first declared here"),
                    }]);
                    return Err(error);
                }
                Some(_) => {}
                None => {
                    first.insert(name, *span);
                }
            }
        }
        // A block's props are its component's; the rest of every other
        // file's names are members it shares.
        let own = &self.units[host].file.components[component];
        let props: Vec<String> = own
            .props
            .iter()
            .chain(&own.injects)
            .map(|p| p.name.clone())
            .collect();
        let names = |skip: usize| -> Vec<(String, bool)> {
            let others = files.iter().enumerate().filter(|(i, _)| *i != skip);
            let names = others.flat_map(|(_, names)| names);
            let members = names.filter(|(name, ..)| !props.contains(name));
            members
                .map(|(name, callable, _)| (name.clone(), *callable))
                .collect()
        };
        let outer = names(0);
        let blocks: Vec<_> = (1..=parts.len()).map(names).collect();
        self.units[host].file.components[component].members.outer = outer;
        for (part, outer) in parts.iter().zip(blocks) {
            let block = self.units[part.unit].file.members.as_mut();
            let members = &mut block.expect("checked").members;
            members.props = props.clone();
            members.outer = outer;
        }
        Ok(())
    }

    /// For each unit that is a members file, the unit of the file that
    /// lists it. A declaration in that listing file and a binding in its
    /// members file are in one file once the members are pasted in (D4).
    pub(super) fn listed_by(&self) -> Vec<Option<usize>> {
        let mut listed = vec![None; self.units.len()];
        for (host, unit) in self.units.iter().enumerate() {
            for part in &unit.parts {
                listed[part.unit] = Some(host);
            }
        }
        listed
    }

    /// Each file's local bindings outside its members block, and those in
    /// its block, which join the listing file's once pasted in. Neither counts
    /// the names `prepare_members` copied across from the component's other
    /// files: each is bound where it is declared.
    pub(super) fn bindings_by_file(&self) -> (Vec<HashSet<String>>, Vec<HashSet<String>>) {
        let unshared = |c: &mut contract_syntax::Component| {
            c.members.props.clear();
            c.members.outer.clear();
        };
        let mut own = Vec::new();
        let mut blocks = Vec::new();
        for unit in &self.units {
            let mut file = unit.file.clone();
            let mut block = File {
                members: file.members.take(),
                ..File::default()
            };
            file.components.iter_mut().for_each(unshared);
            block.members.iter_mut().for_each(unshared);
            own.push(contract_syntax::scope::bindings(&mut file));
            blocks.push(contract_syntax::scope::bindings(&mut block));
        }
        (own, blocks)
    }

    /// Splice every members block, scoped, into its component at its line
    /// (D4): last line first, so each line's place among the component's own
    /// members holds. Then decide the refusals the parser left to the whole
    /// component, kept with the syntax refusals so the rest is still checked.
    pub(super) fn splice_members(&mut self) {
        let hosts = self.hosts();
        for (&(host, component), parts) in &hosts {
            let mut parts = parts.clone();
            parts.sort_by_key(|p| std::cmp::Reverse(p.line));
            for part in parts {
                let block = self.units[part.unit].file.members.take();
                let block = block.expect("prepare_members checked every block");
                let c = &mut self.units[host].file.components[component];
                let at = c.members.lines[part.line].at;
                splice(c, at, block);
            }
        }
        // Decide each deferred refusal with the actions from every file of a
        // component: a call in a block, its component's; any other call in a
        // file that lists members files, those of that file's components,
        // each after the splice. The parser has already counted the
        // declarations of the file the call is written in.
        let names = |c: &contract_syntax::Component| -> HashSet<String> {
            c.actions.iter().map(|a| a.name.clone()).collect()
        };
        let mut component_of = HashMap::new();
        for (&(host, component), parts) in &hosts {
            let actions = names(&self.units[host].file.components[component]);
            for part in parts {
                component_of.insert(part.unit, actions.clone());
            }
        }
        for index in 0..self.units.len() {
            let deferred = std::mem::take(&mut self.units[index].file.deferred);
            if deferred.is_empty() {
                continue;
            }
            let file = &self.units[index].file;
            let own: HashSet<String> = file.components.iter().flat_map(names).collect();
            let none = HashSet::new();
            for d in deferred {
                // A block's call counts its component's actions and, as any
                // call in its file, those of the file's own components.
                let component = match d.in_block {
                    true => component_of.get(&index).unwrap_or(&none),
                    false => &none,
                };
                let action = own.contains(&d.name) || component.contains(&d.name);
                if action == d.if_action {
                    let error = self.sources.resolve(CompileError::from(d.error));
                    self.refused.errors.push(error);
                }
            }
        }
    }
}

/// Refuse a members line or block in a source text compiled with no path
/// (`compile`): it has no file to read members from, and its members would
/// otherwise be dropped unread.
pub(crate) fn standalone_members(file: &File) -> Result<(), CompileError> {
    let lines = file.components.iter().flat_map(|c| &c.members.lines);
    if let Some(line) = lines.into_iter().next() {
        let message = format!(
            "`members \"{}\"` needs this file's own path to resolve: compile it with \
             `contract build <file>` (`compile_path`)",
            line.path
        );
        return Err(name_error("contract-use-unresolved", message, line.span));
    }
    if let Some(block) = &file.members {
        let message = format!(
            "no `members \"…\"` line in `{}` lists this text, so its members belong to no \
             component: compile the component's file by its path",
            block.name
        );
        return Err(name_error("contract-members-unlisted", message, block.span));
    }
    Ok(())
}
