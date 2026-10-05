//! `contract lean --components`: the file *before* component expansion, as
//! a `Contract.Components.CProgram` (`semantics/Contract/Components.lean`):
//! every component with its props, injects, `provide` section, `slot`,
//! states, derives and actions, and a view whose uses are still uses. Its
//! meaning is `semantics/Contract/CompSem.lean`; `Contract.Expand` mirrors
//! the expander that turns it into what [`super::lean`] emits.
//!
//! The root's own declarations carry the expanded root's checked types
//! (its first entries); a child's carry the types the checker gave the
//! child standalone (`Types::components`, file order).

use super::{list, load, read, string, strings_beside, ty, Emitter};
use crate::CompileError;
use contract_syntax::{Component, File, Node, TypeExpr};
use contract_types::{Checked, ComponentTypes};
use std::fmt::Write as _;
use std::path::Path;

/// Compile one source text and emit `def <name> : Contract.Components.CProgram`.
pub fn lean_components(src: &str, name: &str) -> Result<String, CompileError> {
    crate::compile(src)?;
    let file = contract_syntax::parse(src)?;
    emit_file(&file, name, None)
}

/// [`lean_components`] for a file, resolving its `use`s.
pub fn lean_components_path(path: &Path, name: &str) -> Result<String, CompileError> {
    let src = read(path)?;
    crate::compile_path_source(path, &src)?;
    emit_file(&load(path, &src)?, name, strings_beside(path)?)
}

fn emit_file(
    file: &File,
    name: &str,
    strings: Option<std::sync::Arc<contract_types::strings::Strings>>,
) -> Result<String, CompileError> {
    let checked = contract_types::check_all(file, false, contract_lower::tags::style, strings)
        .map_err(|mut all| CompileError::from(all.swap_remove(0)))?;
    let e = Emitter {
        out: String::new(),
        checked: &checked,
        root: checked.expanded.root.clone(),
    };
    e.components(name)
}

/// The next node number.
fn next(ids: &mut usize) -> usize {
    let i = *ids;
    *ids += 1;
    i
}

/// Whether a prop is an `action` prop.
fn is_action(t: &Option<TypeExpr>) -> bool {
    matches!(t, Some(TypeExpr::Named(n, _)) if n == "action")
}

impl Emitter<'_> {
    fn components(&self, name: &str) -> Result<String, CompileError> {
        let checked: &Checked = self.checked;
        let file = checked.file;
        let mut o = String::new();
        let _ = writeln!(o, "  fns := {},", self.fns_list()?);
        let records: Vec<String> = file
            .shapes
            .iter()
            .filter(|s| !file.fns.iter().any(|f| f.name == s.name))
            .map(|s| string(&s.name))
            .collect();
        let _ = writeln!(o, "  records := [{}],", records.join(", "));
        let _ = writeln!(o, "  root := {},", self.component(&file.components[0], 0)?);
        let mut children = Vec::new();
        for (k, c) in file.components.iter().enumerate().skip(1) {
            children.push(self.component(c, k)?);
        }
        let _ = writeln!(o, "  components := [{}],", children.join(",\n    "));
        let _ = writeln!(o, "  routes := {},", self.routes_list());
        let _ = writeln!(o, "  router := {},", self.router());
        let _ = writeln!(o, "  strings := {},", self.strings_list());
        let _ = writeln!(o, "  locale := {}", self.locale());
        o.push_str("}\n");
        let head = format!(
            "def {name} : Contract.Components.CProgram := {{\n  shapes := {},\n",
            self.shapes_list(&o)
        );
        Ok(head + &o)
    }

    /// Component `k` of the file (0 the root).
    fn component(&self, c: &Component, k: usize) -> Result<String, CompileError> {
        let types = &self.checked.types;
        let ct: &ComponentTypes = &types.components[k];
        // The root's slots in the expanded root follow the router's.
        let offset = if k == 0 && self.checked.file.routes.is_some() {
            1
        } else {
            0
        };
        let prop = |p: &contract_syntax::Param| {
            let t =
                p.ty.as_ref()
                    .and_then(|t| types.shapes.resolve(t).ok())
                    .map_or(".unknown".into(), |t| ty(&t));
            format!(
                "{{ name := {}, ty := {t}, declared := {}, action := {} }}",
                string(&p.name),
                p.ty.is_some(),
                is_action(&p.ty)
            )
        };
        let props: Vec<String> = c.props.iter().map(prop).collect();
        let injects: Vec<String> = c.injects.iter().map(prop).collect();
        let mut provides = Vec::new();
        for b in &c.provides {
            provides.push(format!("({}, {})", string(&b.name), self.expr(&b.expr)?));
        }
        let mut states = Vec::new();
        for (i, s) in c.states.iter().enumerate() {
            states.push(format!(
                "{{ name := {}, ty := {}, init := {} }}",
                string(&s.name),
                ct.slots.get(offset + i).map_or(".unknown".into(), ty),
                self.expr(&s.expr)?
            ));
        }
        let mut ids = 0usize;
        let view = self.cnodes(&c.view, &mut ids)?;
        let (resources, mutations, tasks) = if k == 0 {
            (
                self.resources_list(c, ct)?,
                self.mutations_list(c, ct),
                self.tasks_list(c)?,
            )
        } else {
            ("[]".into(), "[]".into(), "[]".into())
        };
        Ok(format!(
            "{{\n      name := {}, props := [{}], injects := [{}], provides := [{}], slot := {},\n      \
             states := [{}],\n      derives := {},\n      resources := {},\n      mutations := {},\n      \
             actions := {},\n      tasks := {},\n      view := {} }}",
            string(&c.name),
            props.join(", "),
            injects.join(", "),
            provides.join(", "),
            c.slot,
            states.join(",\n        "),
            self.derives_list(c, ct)?,
            resources,
            mutations,
            self.actions_list(&c.actions, ct)?,
            tasks,
            view
        ))
    }

    /// A view as written, each region, use and `children` node numbered in
    /// preorder within its component.
    fn cnodes(&self, nodes: &[Node], ids: &mut usize) -> Result<String, CompileError> {
        let mut parts = Vec::new();
        for n in nodes {
            parts.push(self.cnode(n, ids)?);
        }
        Ok(format!("[{}]", parts.join(", ")))
    }

    fn cnode(&self, n: &Node, ids: &mut usize) -> Result<String, CompileError> {
        Ok(match n {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                let (props, handlers) = self.attrs(attrs)?;
                format!(
                    "(.element {} {} [{}] [{}] {})",
                    string(tag),
                    list(positional, |x| self.expr(x))?,
                    props.join(", "),
                    handlers.join(", "),
                    self.cnodes(children, ids)?
                )
            }
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => {
                let i = next(ids);
                format!(
                    "(.when {i} {} {} {})",
                    self.expr(cond)?,
                    self.cnodes(then, ids)?,
                    self.cnodes(otherwise, ids)?
                )
            }
            Node::Each {
                var,
                index,
                list: items,
                key,
                body,
                ..
            } => {
                let i = next(ids);
                format!(
                    "(.each {i} {} {} {} {} {})",
                    string(var),
                    match index {
                        Some(x) => format!("(.some {})", string(x)),
                        None => ".none".into(),
                    },
                    self.expr(items)?,
                    self.expr(key)?,
                    self.cnodes(body, ids)?
                )
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let i = next(ids);
                format!(
                    "(.matchN {i} {} {} {} {})",
                    self.expr(subject)?,
                    string(&some.0),
                    self.cnodes(&some.1, ids)?,
                    self.cnodes(none, ids)?
                )
            }
            Node::Use {
                name,
                args,
                children,
                ..
            } => {
                let i = next(ids);
                let mut a = Vec::new();
                for arg in args {
                    a.push(format!(
                        "({}, {})",
                        string(&arg.name),
                        self.expr(&arg.value)?
                    ));
                }
                format!(
                    "(.use {i} {} [{}] {})",
                    string(name),
                    a.join(", "),
                    self.cnodes(children, ids)?
                )
            }
            Node::Children { .. } => format!("(.children {})", next(ids)),
        })
    }
}
