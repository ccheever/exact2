//! The runner's tree read, with opt-in plan identity for development layout.
//! @ref LLP 1104 D2; LLP 1035.002 D6 — only the driver joins the source map.

use super::*;

pub(super) fn request<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    let shallow = match after_key(request, "shallow") {
        None => false,
        Some(value) => match value.split([',', '}']).next().unwrap().trim() {
            "true" => true,
            "false" => false,
            _ => return error("tree shallow must be a boolean"),
        },
    };
    let Some((root, depth)) = (match target(runner, request) {
        Ok(found) => found,
        Err(e) => return error(&e),
    }) else {
        if shallow {
            return error("shallow tree needs a target");
        }
        return all(runner, field_bool(request, "plan"));
    };
    let kernel = runner.kernel();
    if shallow {
        let mut row = kernel.row(root).expect("located live node");
        row.depth = depth;
        return tree_rows(runner, &[row], &[root], field_bool(request, "plan"));
    }
    let mut subtree = kernel.rows(Some(root)).unwrap_or_default();
    for row in &mut subtree {
        row.depth = row.depth.saturating_add(depth);
    }
    tree_rows(runner, &subtree, &[root], field_bool(request, "plan"))
}

pub(super) fn all<D: DataSource>(runner: &Runner<D>, plan: bool) -> String {
    let rows = runner.kernel().rows(None).unwrap_or_default();
    tree_rows(runner, &rows, &runner.roots(), plan)
}

fn tree_rows<D: DataSource>(
    runner: &Runner<D>,
    rows: &[exact_kernel::export::NodeRow],
    roots: &[u32],
    plan: bool,
) -> String {
    let kernel = runner.kernel();
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{},\"roots\":",
        kernel.epoch(),
        kernel.incarnation(),
        num(runner.now_ms())
    );
    ids(roots, &mut s);
    s.push_str(",\"nodes\":[");
    let handlers = (rows.len() != 1).then(|| runner.handlers());
    let mut first = true;
    for row in rows {
        let Some(node) = kernel.node(row.id) else {
            continue;
        };
        if !first {
            s.push(',');
        }
        first = false;
        let _ = write!(s, "{{\"id\":{},\"parent\":", node.id);
        match node.parent {
            Some(p) => {
                let _ = write!(s, "{p}");
            }
            None => s.push_str("null"),
        }
        let _ = write!(s, ",\"depth\":{},\"type\":", row.depth);
        quote(node.node_type.name(), &mut s);
        if plan {
            if let Some((site, _)) = runner.site_of(node.id) {
                let _ = write!(s, ",\"site\":{}", site.0);
            }
        }
        s.push_str(",\"props\":{");
        props_json(&node, &mut s);
        s.push_str("},\"handlers\":[");
        let single;
        let events = if let Some(all) = &handlers {
            all.get(&node.id).map_or(&[][..], Vec::as_slice)
        } else {
            single = runner.handlers_of(node.id);
            &single
        };
        for (i, e) in events.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(e.name(), &mut s);
        }
        s.push(']');
        if runner.inactive(node.id) {
            s.push_str(",\"inactive\":true");
        }
        s.push_str(",\"children\":");
        ids(&node.children(), &mut s);
        s.push('}');
    }
    s.push(']');
    if plan {
        s.push_str(",\"planDigest\":");
        quote(runner.inspection_digest(), &mut s);
    }
    s.push('}');
    s
}
