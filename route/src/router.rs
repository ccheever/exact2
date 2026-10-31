//! The six verbs spend ids only when they make visits.
//!
//! @ref LLP 1038 §3 — Router, Reads, Verbs and Laws; LLP 1153 D5.
//!
//! Open builds the declaration's chain on its declared tab. Push and replace
//! stay on the selected tab, because a person opened from Search belongs to
//! the Search visit. Go compares canonical URLs and searches the selected
//! stack from the top, so repeated visits have an unambiguous pop target.
//! A location's fragment is never compared. Open's leaf, a push, a replace
//! and a `go` that stays at the top write the location's fragment on the
//! visit, so a location that differs from the top's only after `#` changes
//! that visit in place; a `go` that returns to a visit beneath or on
//! another tab writes a fragment only when the location has one, so a
//! location without one returns to the visit as it was, as Back does.

use crate::{canonical, fragment_of, Destination, Entry, Refusal, Router, Tab, Table};

impl Router {
    /// Seed every tab at its root, then open the launch location. An unmatched
    /// launch returns the empty input and one refusal, just like open(empty).
    pub fn launch(table: &Table, location: &str) -> (Self, Option<Refusal>) {
        open(table, Self::default(), location)
    }
}

/// The selected stack, root first. An empty/uninitialized value reads empty.
pub fn stack(r: &Router) -> &[Entry] {
    r.tabs
        .iter()
        .find(|t| t.name == r.tab)
        .map(|t| t.stack.as_slice())
        .unwrap_or(&[])
}

/// The selected visit, absent before launch.
pub fn top(r: &Router) -> Option<&Entry> {
    stack(r).last()
}

/// The number of visits on the selected stack.
pub fn depth(r: &Router) -> usize {
    stack(r).len()
}

/// A parameter's non-empty values over the selected stack, duplicates kept.
pub fn params<'a>(r: &'a Router, name: &str) -> Vec<&'a str> {
    stack(r)
        .iter()
        .filter_map(|entry| entry.params.get(name))
        .filter(|value| !value.is_empty())
        .map(String::as_str)
        .collect()
}

fn refusal(message: impl Into<String>) -> Refusal {
    Refusal {
        message: message.into(),
    }
}

fn no_match(location: &str) -> Refusal {
    refusal(format!("no route matches {}", canonical(location)))
}

fn selected(r: &Router) -> Result<usize, Refusal> {
    r.tabs
        .iter()
        .position(|t| t.name == r.tab && !t.stack.is_empty())
        .ok_or_else(|| refusal("router has no selected stack"))
}

fn mint(next: &mut u64, d: Destination) -> Result<Entry, Refusal> {
    let id = *next;
    // Values will also be JavaScript/plan numbers. Refuse before losing exact
    // integer identity; even the counter itself must cross that seam unchanged.
    if id >= (1u64 << 53) - 1 {
        return Err(refusal("router entry ids exhausted"));
    }
    *next += 1;
    Ok(Entry {
        id,
        name: d.name,
        url: d.url,
        tab: d.tab,
        params: d.params,
        fragment: String::new(),
    })
}

/// Select the declared tab and replace its chain. Same URL at the same
/// position keeps its id (and, above the leaf, its fragment); every other
/// visit is fresh. The leaf takes the location's fragment. Other tabs are
/// retained.
pub fn open(table: &Table, r: Router, location: &str) -> (Router, Option<Refusal>) {
    let chain = table.chain(location);
    let Some(target) = chain.first().map(|d| d.tab.clone()) else {
        return (r, Some(no_match(location)));
    };
    let leaf = chain.len() - 1;
    let result = (|| {
        let mut out = r.clone();
        if out.tabs.is_empty() {
            table.check().map_err(|e| refusal(e.message))?;
            for root in table.roots() {
                let route = &table.routes[root];
                let d = Destination {
                    name: route.name.clone(),
                    url: canonical(&route.pattern),
                    tab: route.name.clone(),
                    params: table.empty_params(),
                };
                let entry = mint(&mut out.next, d)?;
                out.tabs.push(Tab {
                    name: route.name.clone(),
                    stack: vec![entry],
                });
            }
        }
        let index = out
            .tabs
            .iter()
            .position(|t| t.name == target)
            .ok_or_else(|| refusal(format!("unknown tab {target}")))?;
        let mut entries = Vec::with_capacity(chain.len());
        for (position, d) in chain.into_iter().enumerate() {
            let mut entry = match out.tabs[index]
                .stack
                .get(position)
                .filter(|old| old.url == d.url)
            {
                Some(old) => Entry {
                    id: old.id,
                    name: d.name,
                    url: d.url,
                    tab: d.tab,
                    params: d.params,
                    fragment: old.fragment.clone(),
                },
                None => mint(&mut out.next, d)?,
            };
            if position == leaf {
                entry.fragment = fragment_of(location);
            }
            entries.push(entry);
        }
        out.tabs[index].stack = entries;
        out.tab = target;
        Ok(out)
    })();
    commit(r, result)
}

/// Append one fresh visit to the selected stack, regardless of its declared
/// tab, unless the location is already on top: then only its fragment
/// changes.
pub fn push(table: &Table, r: Router, location: &str) -> (Router, Option<Refusal>) {
    if r.tabs.is_empty() {
        return open(table, r, location);
    }
    let result = (|| {
        let d = destination(table, &r, location)?;
        let index = selected(&r)?;
        let fragment = fragment_of(location);
        // A push of the location already on top is no new visit: as HTML
        // replaces the entry for a same-URL navigation (LLP 1038), and
        // navigates within the document for a new fragment (LLP 1153 D5).
        if let Some(top) = r.tabs[index].stack.last().filter(|top| top.url == d.url) {
            if top.fragment == fragment {
                return Ok(r.clone());
            }
            let mut out = r.clone();
            if let Some(top) = out.tabs[index].stack.last_mut() {
                top.fragment = fragment;
            }
            return Ok(out);
        }
        let mut out = r.clone();
        let mut entry = mint(&mut out.next, d)?;
        entry.fragment = fragment;
        out.tabs[index].stack.push(entry);
        Ok(out)
    })();
    commit(r, result)
}

/// Rewrite the top visit while keeping its id and owning tab. At depth one
/// only a location matching the tab's own root route can replace it.
pub fn replace(table: &Table, r: Router, location: &str) -> (Router, Option<Refusal>) {
    if r.tabs.is_empty() {
        return open(table, r, location);
    }
    let result = (|| {
        let d = destination(table, &r, location)?;
        let index = selected(&r)?;
        if r.tabs[index].stack.len() == 1 && d.name != r.tabs[index].name {
            return Err(refusal("replace cannot change the tab's root route"));
        }
        let mut out = r.clone();
        let old = out.tabs[index]
            .stack
            .last_mut()
            .ok_or_else(|| refusal("router has no top"))?;
        *old = Entry {
            id: old.id,
            name: d.name,
            url: d.url,
            tab: d.tab,
            params: d.params,
            fragment: fragment_of(location),
        };
        Ok(out)
    })();
    commit(r, result)
}

/// Pop the selected stack. A root (or uninitialized value) is unchanged.
pub fn back(_table: &Table, mut r: Router) -> (Router, Option<Refusal>) {
    if let Ok(index) = selected(&r) {
        if r.tabs[index].stack.len() > 1 {
            r.tabs[index].stack.pop();
        }
    }
    (r, None)
}

/// Show a retained tab; reselecting the current tab pops it to its root position.
/// An unknown tab leaves the entire value unchanged and reports one refusal.
pub fn select(_table: &Table, mut r: Router, name: &str) -> (Router, Option<Refusal>) {
    let Some(index) = r
        .tabs
        .iter()
        .position(|t| t.name == name && !t.stack.is_empty())
    else {
        return (r, Some(refusal(format!("unknown tab {name}"))));
    };
    if r.tab == name {
        r.tabs[index].stack.truncate(1);
    }
    r.tab = name.to_owned();
    (r, None)
}

/// Stay at the top, pop to the nearest selected occurrence, select another
/// tab's top, or push — in that order, comparing canonical locations. The
/// top takes the location's fragment; a visit popped to or selected takes
/// it only when the location has one.
pub fn go(table: &Table, mut r: Router, location: &str) -> (Router, Option<Refusal>) {
    let url = canonical(location);
    if table.match_index(&url).is_none() {
        return (r, Some(no_match(location)));
    }
    let fragment = fragment_of(location);
    if let Some(position) = stack(&r).iter().rposition(|entry| entry.url == url) {
        if let Ok(index) = selected(&r) {
            let stack = &mut r.tabs[index].stack;
            if position + 1 == stack.len() || !fragment.is_empty() {
                stack[position].fragment = fragment;
            }
            stack.truncate(position + 1);
        }
        return (r, None);
    }
    if let Some(index) = r
        .tabs
        .iter()
        .position(|t| t.name != r.tab && t.stack.last().is_some_and(|e| e.url == url))
    {
        if let Some(top) = r.tabs[index]
            .stack
            .last_mut()
            .filter(|_| !fragment.is_empty())
        {
            top.fragment = fragment;
        }
        let name = r.tabs[index].name.clone();
        return select(table, r, &name);
    }
    push(table, r, location)
}

fn destination(table: &Table, r: &Router, location: &str) -> Result<Destination, Refusal> {
    let url = canonical(location);
    let (index, params) = table.match_index(&url).ok_or_else(|| no_match(location))?;
    Ok(Destination {
        name: table.routes[index].name.clone(),
        params,
        url,
        tab: r.tab.clone(),
    })
}

fn commit(input: Router, result: Result<Router, Refusal>) -> (Router, Option<Refusal>) {
    match result {
        Ok(out) => (out, None),
        Err(message) => (input, Some(message)),
    }
}
