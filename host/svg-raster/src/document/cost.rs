//! Bound what usvg expands before it expands it. A `<use>` copies its target,
//! a marker is copied at each vertex of the shape that names it, and a paint
//! server, clip path or mask can be resolved again for each element that
//! references it. None of that is in the source's byte or node count, so a
//! 14 KB document can convert to a gigabyte tree. This walk counts the
//! expanded document on the parsed XML, following references transitively,
//! and stops as soon as the count passes [`LIMIT`]: its own work is bounded
//! by the limit, never by the expansion.
use super::Error;
use std::collections::HashMap;

/// Expanded units: an element is one, plus a unit for each path vertex and
/// [`GLYPH`] for each character of text. Any document within the 256 KiB
/// source limit that expands nothing stays well under it.
pub(super) const LIMIT: u64 = 500_000;
/// A glyph's outline is a path; a generous vertex count for one.
const GLYPH: u64 = 32;
/// Nesting deeper than this (usvg recurses on it too) is refused.
const DEPTH: usize = 256;
const XLINK: &str = "http://www.w3.org/1999/xlink";

type Node<'a, 'input> = roxmltree::Node<'a, 'input>;

/// Refuse a document whose expansion passes the limit.
pub(super) fn check(xml: &roxmltree::Document<'_>) -> Result<(), Error> {
    let ids: HashMap<&str, Node<'_, '_>> = xml
        .descendants()
        .filter(|n| n.is_element())
        .filter_map(|n| Some((n.attribute("id")?, n)))
        .collect();
    let mut walk = Walk {
        ids,
        global: Vec::new(),
        cost: 0,
        active: Vec::new(),
        vertices: HashMap::new(),
        content: HashMap::new(),
    };
    // A stylesheet's rule can apply its references to any element: charge
    // each one to every element, as if every rule matched it.
    let mut global = Vec::new();
    for style in xml.descendants().filter(|n| n.has_tag_name("style")) {
        if let Some(text) = style.text() {
            for id in urls(text) {
                if let Some(target) = walk.ids.get(id) {
                    if !global.contains(target) {
                        global.push(*target);
                    }
                }
            }
        }
    }
    walk.global = global;
    walk.element(xml.root_element(), &[], 0)
}

struct Walk<'a, 'input> {
    ids: HashMap<&'a str, Node<'a, 'input>>,
    global: Vec<Node<'a, 'input>>,
    cost: u64,
    /// The references being expanded: one met again is a cycle usvg drops.
    active: Vec<roxmltree::NodeId>,
    vertices: HashMap<roxmltree::NodeId, u64>,
    /// A marker, pattern, clip path or mask's content inherits from its own
    /// ancestors, not from the element that names it, so it costs the same
    /// wherever it is used.
    content: HashMap<roxmltree::NodeId, u64>,
}

/// Containers usvg renders only when something references them.
fn referenced_only(name: &str) -> bool {
    matches!(
        name,
        "defs"
            | "symbol"
            | "marker"
            | "pattern"
            | "clipPath"
            | "mask"
            | "linearGradient"
            | "radialGradient"
            | "style"
            | "script"
            | "title"
            | "desc"
            | "metadata"
    )
}

fn marker_capable(name: &str) -> bool {
    matches!(name, "path" | "line" | "polyline" | "polygon")
}

/// Properties whose value a descendant inherits (SVG 2 §13, §11.6).
fn inherited(name: &str) -> bool {
    matches!(
        name,
        "fill" | "stroke" | "marker" | "marker-start" | "marker-mid" | "marker-end"
    )
}

impl<'a, 'input> Walk<'a, 'input> {
    fn add(&mut self, units: u64) -> Result<(), Error> {
        self.cost = self.cost.saturating_add(units);
        if self.cost > LIMIT {
            Err(Error::Limit)
        } else {
            Ok(())
        }
    }

    fn href(&self, node: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
        let link = node
            .attribute("href")
            .or_else(|| node.attribute((XLINK, "href")))?;
        self.ids.get(link.trim().strip_prefix('#')?).copied()
    }

    /// The elements `node` names with `url(#…)`, split into those its
    /// descendants inherit and those that apply to it alone.
    fn refs(&self, node: Node<'a, 'input>) -> (Vec<Node<'a, 'input>>, Vec<Node<'a, 'input>>) {
        let (mut own, mut passed) = (Vec::new(), Vec::new());
        let mut push = |name: &str, value: &str| {
            for id in urls(value) {
                if let Some(target) = self.ids.get(id) {
                    if inherited(name) {
                        passed.push(*target);
                    } else {
                        own.push(*target);
                    }
                }
            }
        };
        for attribute in node.attributes() {
            if attribute.name() == "style" {
                for declaration in simplecss::DeclarationTokenizer::from(attribute.value()) {
                    push(declaration.name, declaration.value);
                }
            } else {
                push(attribute.name(), attribute.value());
            }
        }
        (own, passed)
    }

    fn element(
        &mut self,
        node: Node<'a, 'input>,
        inherited: &[Node<'a, 'input>],
        depth: usize,
    ) -> Result<(), Error> {
        let name = node.tag_name().name();
        if referenced_only(name) {
            return Ok(());
        }
        self.rendered(node, inherited, depth)
    }

    /// `node` as usvg converts it where it is shown: itself, what it
    /// references, and its children.
    fn rendered(
        &mut self,
        node: Node<'a, 'input>,
        inherited: &[Node<'a, 'input>],
        depth: usize,
    ) -> Result<(), Error> {
        if depth > DEPTH {
            return Err(Error::Limit);
        }
        let name = node.tag_name().name();
        let vertices = self.vertices(node);
        self.add(1 + vertices)?;
        let (own, passed) = self.refs(node);
        let mut inherit = inherited.to_vec();
        for target in passed {
            if !inherit.contains(&target) {
                inherit.push(target);
            }
        }
        let global = std::mem::take(&mut self.global);
        let mut applied = own.iter().chain(&inherit).chain(&global).copied();
        let result = applied.try_for_each(|target| self.reference(name, vertices, target, depth));
        self.global = global;
        result?;
        if name == "use" {
            if let Some(target) = self.href(node) {
                self.expand(target, &inherit, depth)?;
            }
        } else if name == "textPath" {
            // A text path borrows the referenced path's geometry only.
            if let Some(target) = self.href(node) {
                let geometry = self.vertices(target);
                self.add(geometry)?;
            }
        }
        if matches!(name, "text" | "tspan" | "textPath" | "a") {
            let chars: usize = node
                .children()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .map(|t| t.chars().count())
                .sum();
            self.add(chars as u64 * GLYPH)?;
        }
        for child in node.children().filter(|n| n.is_element()) {
            self.element(child, &inherit, depth + 1)?;
        }
        Ok(())
    }

    /// A `<use>`'s target, shown in the use's place.
    fn expand(
        &mut self,
        target: Node<'a, 'input>,
        inherited: &[Node<'a, 'input>],
        depth: usize,
    ) -> Result<(), Error> {
        if self.active.contains(&target.id()) {
            return Ok(());
        }
        self.active.push(target.id());
        let result = match target.tag_name().name() {
            // A symbol is shown only through a use, as a nested viewport.
            "symbol" | "svg" => self.rendered(target, inherited, depth + 1),
            _ => self.element(target, inherited, depth + 1),
        };
        self.active.pop();
        result
    }

    /// An element's charge for one reference: a marker's content at each
    /// vertex of a shape, or a paint server, clip path or mask's once.
    fn reference(
        &mut self,
        owner: &str,
        vertices: u64,
        target: Node<'a, 'input>,
        depth: usize,
    ) -> Result<(), Error> {
        let instances = match target.tag_name().name() {
            "marker" if marker_capable(owner) => vertices + 1,
            "pattern" | "clipPath" | "mask" | "linearGradient" | "radialGradient" => 1,
            _ => 0,
        };
        // Applying even an empty reference is work: charge it, so the
        // number of references applied is bounded too.
        self.add(1)?;
        if instances == 0 || self.active.contains(&target.id()) {
            return Ok(());
        }
        let content = self.content(target, depth)?;
        self.add(instances.saturating_mul(content))
    }

    /// What one copy of a referenced marker, pattern, clip path or mask
    /// costs: its children, the references on the element itself, and the
    /// template a pattern or gradient names with `href`.
    fn content(&mut self, target: Node<'a, 'input>, depth: usize) -> Result<u64, Error> {
        if let Some(cost) = self.content.get(&target.id()) {
            return Ok(*cost);
        }
        let outer = std::mem::replace(&mut self.cost, 0);
        self.active.push(target.id());
        let mut inherit = Vec::new();
        for ancestor in target.ancestors().skip(1).filter(|n| n.is_element()) {
            inherit.extend(self.refs(ancestor).1);
        }
        let result = (|| {
            self.add(1)?;
            let (own, passed) = self.refs(target);
            inherit.extend(passed);
            let name = target.tag_name().name();
            for referenced in own {
                self.reference(name, 0, referenced, depth + 1)?;
            }
            if let Some(template) = self.href(target) {
                if !self.active.contains(&template.id()) {
                    let cost = self.content(template, depth + 1)?;
                    self.add(cost)?;
                }
            }
            for child in target.children().filter(|n| n.is_element()) {
                self.element(child, &inherit, depth + 1)?;
            }
            Ok(())
        })();
        self.active.pop();
        let cost = std::mem::replace(&mut self.cost, outer);
        result?;
        self.content.insert(target.id(), cost);
        Ok(cost)
    }

    /// An upper bound of the vertices usvg makes of a shape.
    fn vertices(&mut self, node: Node<'a, 'input>) -> u64 {
        if let Some(count) = self.vertices.get(&node.id()) {
            return *count;
        }
        let count = match node.tag_name().name() {
            "path" => node.attribute("d").map_or(0, |d| {
                svgtypes::PathParser::from(d)
                    .map_while(Result::ok)
                    .map(|segment| match segment {
                        // An arc becomes up to four cubics.
                        svgtypes::PathSegment::EllipticalArc { .. } => 4,
                        _ => 1,
                    })
                    .sum()
            }),
            "polyline" | "polygon" => node
                .attribute("points")
                .map_or(0, |p| svgtypes::PointsParser::from(p).count() as u64),
            "line" => 2,
            "rect" | "circle" | "ellipse" => 9,
            _ => 0,
        };
        self.vertices.insert(node.id(), count);
        count
    }
}

/// The fragment ids named by `url(#id)`, `url('#id')` and `url("#id")`.
fn urls(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices("url(").filter_map(move |(at, _)| {
        let rest = text[at + 4..].trim_start();
        let rest = rest.strip_prefix(['\'', '"']).unwrap_or(rest);
        let rest = rest.strip_prefix('#')?;
        let end = rest
            .find(|c: char| c == ')' || c == '\'' || c == '"' || c.is_whitespace())
            .unwrap_or(rest.len());
        Some(&rest[..end])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(source: &str) -> Result<u64, Error> {
        let xml = roxmltree::Document::parse(source).unwrap();
        let ids = xml
            .descendants()
            .filter(|n| n.is_element())
            .filter_map(|n| Some((n.attribute("id")?, n)))
            .collect();
        let mut walk = Walk {
            ids,
            global: Vec::new(),
            cost: 0,
            active: Vec::new(),
            vertices: HashMap::new(),
            content: HashMap::new(),
        };
        walk.element(xml.root_element(), &[], 0)?;
        Ok(walk.cost)
    }

    #[test]
    fn use_copies_and_marker_instances_are_counted_where_shown() {
        let path = "<path id='p' d='M0 0L1 1L2 2L3 3'/>";
        let plain = cost(&format!("<svg><defs>{path}</defs></svg>")).unwrap();
        let used = cost(&format!(
            "<svg><defs>{path}</defs><use href='#p'/><use xlink:href='#p' xmlns:xlink='{XLINK}'/></svg>"
        ))
        .unwrap();
        // Two uses, each charging itself and a four-vertex path.
        assert_eq!(used - plain, 2 * (1 + 1 + 4));
        let marked = cost("<svg><defs><marker id='m'><circle r='1'/><circle r='1'/></marker></defs><path d='M0 0L1 1L2 2' style='marker-mid:url(#m)'/></svg>").unwrap();
        let unmarked =
            cost("<svg><defs><marker id='m'><circle r='1'/><circle r='1'/></marker></defs><path d='M0 0L1 1L2 2'/></svg>")
                .unwrap();
        // Four instances (vertices + 1) of a marker costing 1 + 2 × 10.
        assert_eq!(marked - unmarked, 1 + 4 * 21);
    }

    #[test]
    fn inherited_and_stylesheet_references_reach_every_shape() {
        let inherited = cost("<svg><defs><marker id='m'><rect/></marker></defs><g marker-end='url(#m)'><path d='M0 0L1 1'/><path d='M0 0L1 1'/></g></svg>").unwrap();
        let none = cost("<svg><defs><marker id='m'><rect/></marker></defs><g><path d='M0 0L1 1'/><path d='M0 0L1 1'/></g></svg>").unwrap();
        assert!(inherited > none + 2 * 3 * 10);
        let mut source =
            String::from("<svg><style>path{marker-mid:url(#m)}</style><defs><marker id='m'>");
        source += &"<circle r='1'/>".repeat(50);
        source += "</marker></defs><path d='M0 0";
        source += &" L1 1".repeat(12_000);
        source += "'/></svg>";
        let xml = roxmltree::Document::parse(&source).unwrap();
        assert_eq!(check(&xml), Err(Error::Limit));
    }

    #[test]
    fn reference_cycles_end_where_usvg_drops_them() {
        assert!(cost("<svg><g id='g'><use href='#g'/></g><use href='#g'/></svg>").is_ok());
        assert!(cost("<svg><defs><pattern id='p' href='#q'><rect fill='url(#p)'/></pattern><pattern id='q' href='#p'/></defs><rect fill='url(#p)'/></svg>").is_ok());
    }

    #[test]
    fn urls_accept_quotes_and_fallbacks() {
        let found: Vec<_> = urls("url(#a) url('#b') url( \"#c\" ) url(#d) red url(x)").collect();
        assert_eq!(found, ["a", "b", "c", "d"]);
    }
}
