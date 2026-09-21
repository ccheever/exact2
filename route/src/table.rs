//! Declared order decides matching; parent links decide a deep link's stack.
//!
//! @ref LLP 1038 §3 — Table, Match, Chain; D2's static rejects, D3's path.
//!
//! Parameter records have every name in the table. Ancestors bind only their
//! own names, so matching an ancestor's URL reads the same record back. The
//! leaf keeps the incoming URL, including its query and escape spelling;
//! synthesized ancestors are formatted from their own patterns.

use crate::location::decode;
use crate::{
    canonical, encode_uri_component, CheckError, Destination, Match, Params, PathError, Table,
};
use std::collections::BTreeSet;

impl Table {
    /// Parameter names in first-declaration order, each exactly once.
    /// This is the field order for the plan's positional Params record.
    pub fn param_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for route in &self.routes {
            if !route.notfound {
                for name in names_in(&route.pattern) {
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        names
    }

    /// D2's static checks. Parameter identifiers use the ASCII subset
    /// `[A-Za-z_$][A-Za-z0-9_$]*`; literals may contain Unicode and spaces. Malformed parent links use route-parent-param;
    /// an empty table uses route-pattern; an unmatched boot location uses route-root.
    pub fn check(&self) -> Result<(), CheckError> {
        let reject = |code: &str, route, message: String| CheckError {
            code: code.into(),
            route,
            message,
        };
        if self.routes.is_empty() {
            return Err(reject(
                "route-pattern",
                0,
                "a route table needs a root".into(),
            ));
        }
        let mut names = BTreeSet::new();
        let mut patterns: Vec<Vec<String>> = Vec::new();
        let mut notfound = false;
        for (i, route) in self.routes.iter().enumerate() {
            if !names.insert(&route.name) {
                return Err(reject(
                    "route-duplicate",
                    i,
                    format!("duplicate route {}", route.name),
                ));
            }
            if route.name.is_empty()
                || if route.notfound {
                    !route.pattern.is_empty() || route.tab
                } else {
                    !valid_pattern(&route.pattern)
                }
            {
                return Err(reject(
                    "route-pattern",
                    i,
                    format!("invalid pattern for {}", route.name),
                ));
            }
            if route.notfound {
                if notfound {
                    return Err(reject(
                        "route-shadowed",
                        i,
                        "an earlier notfound already absorbs unmatched locations".into(),
                    ));
                }
                notfound = true;
            } else {
                let normalized = canonical(&route.pattern);
                let segments: Vec<String> = segments(&normalized)
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                if patterns.iter().any(|earlier| shadows(earlier, &segments)) {
                    return Err(reject(
                        "route-shadowed",
                        i,
                        format!("an earlier pattern fully shadows {}", route.name),
                    ));
                }
                patterns.push(segments);
            }
            let own = names_in(&route.pattern);
            let mut seen = BTreeSet::from([i]);
            let mut parent = route.parent;
            while let Some(p) = parent {
                let Some(ancestor) = self.routes.get(p).filter(|_| seen.insert(p)) else {
                    return Err(reject(
                        "route-parent-param",
                        i,
                        format!("invalid or cyclic parent of {}", route.name),
                    ));
                };
                if ancestor.notfound
                    || names_in(&ancestor.pattern)
                        .iter()
                        .any(|name| !own.contains(name))
                {
                    return Err(reject(
                        "route-parent-param",
                        i,
                        format!("{} cannot supply its parent's parameters", route.name),
                    ));
                }
                parent = ancestor.parent;
            }
        }
        for root in self.roots() {
            let route = &self.routes[root];
            if route.notfound || !names_in(&route.pattern).is_empty() {
                return Err(reject(
                    "route-pattern",
                    root,
                    format!("tab root {} needs a literal path", route.name),
                ));
            }
        }
        if !notfound && self.matches_pattern("/").is_none() {
            return Err(reject(
                "route-root",
                self.roots()[0],
                "the first tab's root must be `/`, or declare `notfound`".into(),
            ));
        }
        Ok(())
    }

    /// Match only declared patterns: a fallback never makes a literal valid.
    pub fn matches_pattern(&self, location: &str) -> Option<Match> {
        self.match_pattern_index(&canonical(location))
            .map(|(index, params)| Match {
                name: self.routes[index].name.clone(),
                params,
            })
    }

    /// Canonicalize, match segment for segment, and bind decoded strings.
    /// A trailing slash (except `/`) matches no pattern. Notfound is a fallback
    /// after all ordinary patterns, wherever its row was declared.
    pub fn matches(&self, location: &str) -> Option<Match> {
        self.match_index(&canonical(location))
            .map(|(index, params)| Match {
                name: self.routes[index].name.clone(),
                params,
            })
    }

    pub(crate) fn match_index(&self, url: &str) -> Option<(usize, Params)> {
        self.match_pattern_index(url).or_else(|| {
            self.routes
                .iter()
                .position(|r| r.notfound)
                .map(|i| (i, self.empty_params()))
        })
    }

    fn match_pattern_index(&self, url: &str) -> Option<(usize, Params)> {
        let path = url.split('?').next().unwrap_or(url);
        let parts = segments(path);
        if path == "/" || !path.ends_with('/') {
            for (i, route) in self.routes.iter().enumerate().filter(|(_, r)| !r.notfound) {
                let pattern = canonical(&route.pattern);
                let pattern = segments(&pattern);
                if pattern.len() != parts.len() {
                    continue;
                }
                let mut params = self.empty_params();
                let matched = pattern.iter().zip(&parts).all(|(segment, value)| {
                    if let Some(name) = segment.strip_prefix(':') {
                        if value.is_empty() {
                            return false;
                        }
                        params.insert(name.to_owned(), decode(value, false));
                        true
                    } else {
                        segment == value
                    }
                });
                if matched {
                    return Some((i, params));
                }
            }
        }
        None
    }

    /// Format a named route with encodeURIComponent on each supplied segment.
    /// Unknown names, notfound, wrong arity, and empty/dot-only parameters
    /// report `route-unknown`; every parameter must survive canonicalization.
    pub fn path(&self, name: &str, params: &[&str]) -> Result<String, PathError> {
        let route = self
            .routes
            .iter()
            .find(|r| r.name == name && !r.notfound)
            .ok_or_else(|| {
                let choices = self
                    .routes
                    .iter()
                    .filter(|r| !r.notfound)
                    .map(|r| format!("`{}`", r.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                let hint = if choices.is_empty() {
                    "no routes can be used with path".to_owned()
                } else {
                    format!("available routes: {choices}")
                };
                PathError {
                    code: "route-unknown".into(),
                    message: format!("unknown path route `{name}`; {hint}"),
                }
            })?;
        {
            let expected = names_in(&route.pattern);
            if expected.len() != params.len() {
                let noun = if expected.len() == 1 {
                    "parameter"
                } else {
                    "parameters"
                };
                let names = if expected.is_empty() {
                    String::new()
                } else {
                    format!(
                        " ({})",
                        expected
                            .iter()
                            .map(|n| format!("`{n}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                return Err(PathError {
                    code: "route-unknown".into(),
                    message: format!(
                        "`{name}` expects {} path {noun}{names}, received {}",
                        expected.len(),
                        params.len()
                    ),
                });
            }
        }
        let mut values = params.iter();
        Ok(route
            .pattern
            .split('/')
            .map(|segment| {
                if segment.starts_with(':') {
                    encode_route_segment(values.next().copied().unwrap_or(""))
                } else {
                    Ok(segment.to_owned())
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/"))
    }

    /// The declared root-to-leaf chain without ids, or empty on no match.
    /// Only the leaf carries the incoming query. Notfound belongs to the first
    /// tab and carries the unmatched location verbatim in canonical form.
    pub fn chain(&self, location: &str) -> Vec<Destination> {
        let url = canonical(location);
        let Some((index, params)) = self.match_index(&url) else {
            return Vec::new();
        };
        let Some(root) = self.root_for(index) else {
            return Vec::new();
        };
        let tab = self.routes[root].name.clone();
        let mut indices = vec![index];
        if !self.routes[index].notfound {
            let mut parent = self.routes[index].parent;
            while *indices.last().unwrap_or(&root) != root {
                let Some(p) = parent else {
                    break;
                };
                if indices.contains(&p) {
                    return Vec::new();
                }
                let Some(route) = self.routes.get(p) else {
                    return Vec::new();
                };
                indices.push(p);
                parent = route.parent;
            }
        }
        if !indices.contains(&root) {
            indices.push(root);
        }
        indices.reverse();
        indices
            .into_iter()
            .map(|i| {
                let route = &self.routes[i];
                let entry_url = if i == index {
                    url.clone()
                } else {
                    let values: Vec<&str> = names_in(&route.pattern)
                        .iter()
                        .map(|name| params.get(*name).map(String::as_str).unwrap_or(""))
                        .collect();
                    canonical(&self.path(&route.name, &values).unwrap_or_default())
                };
                let mut own = self.empty_params();
                for name in names_in(&route.pattern) {
                    if let Some(value) = params.get(name) {
                        own.insert(name.into(), value.clone());
                    }
                }
                Destination {
                    name: route.name.clone(),
                    url: entry_url,
                    tab: tab.clone(),
                    params: own,
                }
            })
            .collect()
    }

    pub(crate) fn empty_params(&self) -> Params {
        self.param_names()
            .into_iter()
            .map(|name| (name.to_owned(), String::new()))
            .collect()
    }

    /// Tab names in declaration order, including the implicit first-route tab.
    pub fn tab_names(&self) -> Vec<&str> {
        self.roots()
            .into_iter()
            .map(|i| self.routes[i].name.as_str())
            .collect()
    }

    pub(crate) fn roots(&self) -> Vec<usize> {
        let roots: Vec<_> = self
            .routes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.tab)
            .map(|(i, _)| i)
            .collect();
        if roots.is_empty() && !self.routes.is_empty() {
            vec![0]
        } else {
            roots
        }
    }

    fn root_for(&self, index: usize) -> Option<usize> {
        let roots = self.roots();
        if !self.routes.get(index)?.notfound {
            let mut current = Some(index);
            for _ in 0..self.routes.len() {
                let Some(i) = current else {
                    break;
                };
                if roots.contains(&i) {
                    return Some(i);
                }
                current = self.routes.get(i)?.parent;
            }
        }
        roots.first().copied()
    }
}

fn names_in(pattern: &str) -> Vec<&str> {
    pattern
        .split('/')
        .filter_map(|segment| segment.strip_prefix(':'))
        .collect()
}

fn segments(path: &str) -> Vec<&str> {
    if path == "/" {
        Vec::new()
    } else {
        path.strip_prefix('/').unwrap_or(path).split('/').collect()
    }
}

fn valid_pattern(pattern: &str) -> bool {
    if !pattern.starts_with('/') || pattern.starts_with("//") {
        return false;
    }
    let mut names = BTreeSet::new();
    segments(pattern).into_iter().all(|segment| {
        if let Some(name) = segment.strip_prefix(':') {
            let mut chars = name.chars();
            // The whole segment is a name, not URLPattern's name + suffix.
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '$'))
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
                && names.insert(name)
        } else {
            !segment.is_empty()
                && !segment.chars().any(|c| {
                    c < ' '
                        || matches!(
                            c,
                            ':' | '?' | '#' | '*' | '(' | ')' | '{' | '}' | '+' | '\\'
                        )
                })
                && !matches!(
                    segment.to_ascii_lowercase().as_str(),
                    "." | ".." | "%2e" | ".%2e" | "%2e." | "%2e%2e"
                )
        }
    })
}

fn shadows(earlier: &[String], later: &[String]) -> bool {
    earlier.len() == later.len()
        && earlier
            .iter()
            .zip(later)
            .all(|(a, b)| a.starts_with(':') || a == b)
}

/// Encode a route parameter that survives standard URL canonicalization.
/// Empty and dot-only segments have no representable parameter location:
/// even percent-encoded dots are removed by WHATWG URL parsing.
pub fn encode_route_segment(value: &str) -> Result<String, PathError> {
    if matches!(value, "" | "." | "..") {
        return Err(PathError {
            code: "route-unknown".into(),
            message: "a path parameter cannot be empty, `.` or `..`".into(),
        });
    }
    Ok(encode_uri_component(value))
}
