//! Infer action interfaces once, then propagate requirements over bindings.
//! @ref LLP 1035.005 D2: invocation, declaration and binding in one diagnostic.

use super::{handler_arity, AnalyzeError, Related, HANDLERS};
use contract_syntax::{Component, Expr, File, Node, Span};
use contract_types::{Ref, Scope, Ty, Types};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone)]
struct Origin {
    invocation: Span,
    declaration: Span,
}

#[derive(Clone)]
struct Need {
    min: usize,
    max: usize,
    low: Origin,
    high: Origin,
}
impl Need {
    fn shifted(&self, bound: usize) -> Self {
        Self {
            min: self.min + bound,
            max: self.max + bound,
            ..self.clone()
        }
    }
    fn description(&self) -> String {
        if self.min == self.max {
            self.min.to_string()
        } else {
            format!("{} or {}", self.min, self.max)
        }
    }
}
struct Interface {
    name: String,
    declaration: Option<Span>,
    need: Option<Need>,
}
#[derive(Clone)]
enum Target {
    Interface(usize),
    Action { name: String, arity: usize },
}
#[derive(Clone)]
struct Binding {
    target: Target,
    bound: usize,
    span: Span,
}
#[derive(Clone)]
struct Edge {
    to: usize,
    bound: usize,
    span: Span,
}
struct Graph<'a> {
    file: &'a File,
    types: &'a Types,
    component_ids: BTreeMap<&'a str, usize>,
    // Explicit props followed by injects, matching Ref::Prop's indices.
    params: Vec<Vec<Option<usize>>>,
    // Ambient provider channels also cross components that do not inject them.
    contexts: Vec<BTreeMap<String, usize>>,
    interfaces: Vec<Interface>,
    edges: Vec<Vec<Edge>>,
    concrete: Vec<(usize, Binding)>,
    queue: VecDeque<usize>,
}

pub(super) fn check(file: &File, types: &Types, expanded: &Component) -> Result<(), AnalyzeError> {
    let context_names: BTreeSet<_> = file
        .components
        .iter()
        .enumerate()
        .flat_map(|(ci, c)| {
            c.injects.iter().enumerate().filter_map(move |(i, p)| {
                matches!(types.components[ci].props[c.props.len() + i], Ty::Action(_))
                    .then_some(p.name.clone())
            })
        })
        .collect();
    let mut graph = Graph {
        file,
        types,
        component_ids: file
            .components
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.as_str(), i))
            .collect(),
        params: Vec::new(),
        contexts: Vec::new(),
        interfaces: Vec::new(),
        edges: Vec::new(),
        concrete: Vec::new(),
        queue: VecDeque::new(),
    };
    for (ci, c) in file.components.iter().enumerate() {
        let mut contexts = BTreeMap::new();
        for name in &context_names {
            let declared = c.injects.iter().find(|p| &p.name == name);
            let id = graph.interface(format!("{}.{}", c.name, name), declared.map(|p| p.span));
            contexts.insert(name.clone(), id);
        }
        let mut params = Vec::new();
        for (i, p) in c.props.iter().enumerate() {
            params.push(
                matches!(types.components[ci].props[i], Ty::Action(_))
                    .then(|| graph.interface(format!("{}.{}", c.name, p.name), Some(p.span))),
            );
        }
        for (i, p) in c.injects.iter().enumerate() {
            params.push(
                matches!(types.components[ci].props[c.props.len() + i], Ty::Action(_))
                    .then(|| contexts[&p.name]),
            );
        }
        graph.params.push(params);
        graph.contexts.push(contexts);
    }
    if graph.interfaces.is_empty() {
        return Ok(());
    }
    for (ci, c) in file.components.iter().enumerate() {
        let scoped = if ci == 0 { expanded } else { c };
        let mut scope = types.component_scope(scoped, &types.components[ci]);
        graph.nodes(ci, &c.view, &mut scope, &mut Vec::new())?;
    }
    while let Some(id) = graph.queue.pop_front() {
        let need = graph.interfaces[id]
            .need
            .clone()
            .expect("queued requirement");
        for edge in graph.edges[id].clone() {
            graph.require(edge.to, need.shifted(edge.bound), Some(edge.span))?;
        }
    }
    for (id, binding) in &graph.concrete {
        let Some(need) = &graph.interfaces[*id].need else {
            continue;
        };
        let Target::Action { name, arity } = &binding.target else {
            unreachable!()
        };
        let remaining = arity.checked_sub(binding.bound);
        if remaining.is_some_and(|n| (need.min..=need.max).contains(&n)) {
            continue;
        }
        let origin = if remaining.is_some_and(|n| n < need.min) {
            &need.low
        } else {
            &need.high
        };
        let mut error = refusal(
            format!("`{}` requires {} action parameter(s), but this binding to `{name}` supplies {} of its {arity}",
                graph.interfaces[*id].name, need.description(), binding.bound),
            origin.invocation,
            origin.declaration,
            binding.span,
            remaining.map_or_else(
                || format!("binding to `{name}` supplies more arguments than it accepts"),
                |n| format!("bound to `{name}` here; {n} parameter(s) remain")),
        );
        if let Some(span) = graph.interfaces[*id].declaration {
            if span != origin.declaration {
                error.related.push(Related {
                    span,
                    note: "forwarded action interface declared here".into(),
                });
            }
        }
        return Err(error);
    }
    Ok(())
}

fn refusal(
    message: String,
    span: Span,
    declaration: Span,
    other: Span,
    note: String,
) -> AnalyzeError {
    AnalyzeError {
        id: "analyze-action-arity",
        message,
        span,
        related: vec![
            Related {
                span: declaration,
                note: "action interface declared here".into(),
            },
            Related { span: other, note },
        ],
    }
}

impl Graph<'_> {
    fn interface(&mut self, name: String, declaration: Option<Span>) -> usize {
        let id = self.interfaces.len();
        self.interfaces.push(Interface {
            name,
            declaration,
            need: None,
        });
        self.edges.push(Vec::new());
        id
    }

    fn require(
        &mut self,
        id: usize,
        need: Need,
        binding: Option<Span>,
    ) -> Result<(), AnalyzeError> {
        let entry = &mut self.interfaces[id];
        if let Some(old) = &mut entry.need {
            if need.min > old.max || old.min > need.max {
                let (incoming, previous) = if need.min > old.max {
                    (&need.low, &old.high)
                } else {
                    (&need.high, &old.low)
                };
                let mut error = refusal(
                    format!(
                        "`{}` has incompatible action requirements: {} and {} parameter(s)",
                        entry.name,
                        old.description(),
                        need.description()
                    ),
                    incoming.invocation,
                    entry.declaration.unwrap_or(incoming.declaration),
                    previous.invocation,
                    "incompatible invocation required here".into(),
                );
                if let Some(span) = binding {
                    error.related.push(Related {
                        span,
                        note: "forwarded through this binding".into(),
                    });
                }
                return Err(error);
            }
            if need.min <= old.min && need.max >= old.max {
                return Ok(());
            }
            if need.min > old.min {
                old.min = need.min;
                old.low = need.low;
            }
            if need.max < old.max {
                old.max = need.max;
                old.high = need.high;
            }
        } else {
            entry.need = Some(need);
        }
        self.queue.push_back(id);
        Ok(())
    }

    fn binding(&self, ci: usize, expr: &Expr, scope: &Scope, span: Span) -> Option<Binding> {
        let (name, bound) = match expr {
            Expr::Ident(name, _) => (name, 0),
            Expr::Call(name, args, _) => {
                // Expression calls use the function namespace before actions.
                if self.types.shapes.fns.contains_key(name) {
                    return None;
                }
                (name, args.len())
            }
            _ => return None,
        };
        let (reference, ty) = scope.lookup(name)?;
        let Ty::Action(params) = ty else { return None };
        let target = match reference {
            Ref::Action(_) => Target::Action {
                name: name.clone(),
                arity: params.len(),
            },
            Ref::Prop(pi) => Target::Interface(self.params[ci][pi as usize]?),
            _ => return None,
        };
        Some(Binding {
            target,
            bound,
            span,
        })
    }

    fn connect(&mut self, id: usize, binding: Binding) {
        match binding.target {
            Target::Interface(to) => self.edges[id].push(Edge {
                to,
                bound: binding.bound,
                span: binding.span,
            }),
            Target::Action { .. } => self.concrete.push((id, binding)),
        }
    }

    fn nodes(
        &mut self,
        ci: usize,
        nodes: &[Node],
        scope: &mut Scope,
        providers: &mut Vec<(String, Option<Binding>)>,
    ) -> Result<(), AnalyzeError> {
        for node in nodes {
            match node {
                Node::Element {
                    attrs, children, ..
                } => {
                    for attr in attrs {
                        if !HANDLERS.contains(&attr.name.as_str()) {
                            continue;
                        }
                        let (name, given) = match &attr.value {
                            Expr::Ident(name, _) => (name, 0),
                            Expr::Call(name, args, _) => (name, args.len()),
                            _ => continue,
                        };
                        let Some((Ref::Prop(pi), Ty::Action(_))) = scope.lookup(name) else {
                            continue;
                        };
                        let Some(id) = self.params[ci][pi as usize] else {
                            continue;
                        };
                        let declaration = self.interfaces[id]
                            .declaration
                            .expect("authored action parameter");
                        let Some(range) = handler_arity(&attr.name, given) else {
                            return Err(refusal(
                                format!(
                                    "`{}=` does not accept explicitly bound arguments",
                                    attr.name
                                ),
                                attr.span,
                                declaration,
                                attr.value.span(),
                                "remove the explicit arguments here".into(),
                            ));
                        };
                        let origin = Origin {
                            invocation: attr.value.span(),
                            declaration,
                        };
                        self.require(
                            id,
                            Need {
                                min: *range.start(),
                                max: *range.end(),
                                low: origin.clone(),
                                high: origin,
                            },
                            None,
                        )?;
                    }
                    self.nodes(ci, children, scope, providers)?;
                }
                Node::Use {
                    name,
                    args,
                    children,
                    ..
                } => {
                    // Slot fills are expanded in the caller's lexical/provider scope.
                    self.nodes(ci, children, scope, providers)?;
                    let target = self.component_ids[name.as_str()];
                    for (i, p) in self.file.components[target].props.iter().enumerate() {
                        let Some(id) = self.params[target][i] else {
                            continue;
                        };
                        let Some(arg) = args.iter().find(|arg| arg.name == p.name) else {
                            continue;
                        };
                        if let Some(binding) = self.binding(ci, &arg.value, scope, arg.span) {
                            self.connect(id, binding);
                        }
                    }
                    for (name, id) in self.contexts[target].clone() {
                        match providers
                            .iter()
                            .rev()
                            .find(|(provided, _)| provided == &name)
                        {
                            Some((_, Some(binding))) => self.connect(id, binding.clone()),
                            Some((_, None)) => {} // A non-action provider still shadows an outer one.
                            None => self.edges[id].push(Edge {
                                to: self.contexts[ci][&name],
                                bound: 0,
                                span: node.span(),
                            }),
                        }
                    }
                }
                Node::Provide {
                    name,
                    expr,
                    body,
                    span,
                } => {
                    providers.push((
                        name.clone(),
                        self.binding(ci, expr, scope, self.file.names.name(*span)),
                    ));
                    self.nodes(ci, body, scope, providers)?;
                    providers.pop();
                }
                Node::Children { .. } => {}
                Node::When {
                    then, otherwise, ..
                } => {
                    self.nodes(ci, then, scope, providers)?;
                    self.nodes(ci, otherwise, scope, providers)?;
                }
                Node::Each {
                    var, index, body, ..
                } => {
                    scope.push_each(var, index.as_deref(), Ty::Unknown);
                    self.nodes(ci, body, scope, providers)?;
                    scope.pop();
                }
                Node::Match { some, none, .. } => {
                    scope.push_region(Some((some.0.clone(), Ref::Bound(0), Ty::Unknown)));
                    self.nodes(ci, &some.1, scope, providers)?;
                    scope.pop();
                    self.nodes(ci, none, scope, providers)?;
                }
            }
        }
        Ok(())
    }
}
