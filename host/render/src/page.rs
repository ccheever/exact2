//! A rendered document as a page (LLP 1048.000 D3, D6, D7): the one place a
//! page is composed, for the build's `dist/` and the server alike.
use crate::Rendered;

/// The built shell with the renderer's `<head>` in place of its title and
/// viewport meta, the shell's wasm and `navigation.js` preloads removed (a
/// document's page preloads nothing, D3), the document in `#exact-root`,
/// and its checkpoint, with the document's digest, before the glue.
/// Refuses a shell that no longer has those places.
pub fn page(shell: &str, rendered: &Rendered) -> Result<String, String> {
    let mut html = shell.to_string();
    // From `start` through the first `end` after it.
    let cut = |html: &mut String, start: &str, end: &str, with: &str| -> Result<(), String> {
        let lacks = || format!("the shell has no `{start}`…`{end}`");
        let at = html.find(start).ok_or_else(lacks)?;
        let stop = html[at..].find(end).ok_or_else(lacks)? + at + end.len();
        html.replace_range(at..stop, with);
        Ok(())
    };
    // The shell's viewport meta goes before the head, which has its own.
    cut(&mut html, "<meta name=\"viewport\"", ">\n", "")?;
    cut(
        &mut html,
        "<title>",
        "</title>\n",
        &format!("{}\n", rendered.head),
    )?;
    cut(&mut html, "<!-- Fetched in parallel", "-->\n", "")?;
    let navigation = "<link rel=\"modulepreload\" href=\"./navigation.js\">\n";
    cut(&mut html, navigation, navigation, "")?;
    let wasm = "<link rel=\"preload\" href=\"./app.wasm\" as=\"fetch\" crossorigin>\n";
    cut(&mut html, wasm, wasm, "")?;
    let root = "<div id=\"exact-root\"></div>";
    let document = format!("<div id=\"exact-root\">{}</div>", rendered.document.root);
    cut(&mut html, root, root, &document)?;
    let glue = "<script type=\"module\" src=\"./glue.js\"></script>";
    let entry = if rendered.activate == exact_plan::ActivatePolicy::Interaction {
        "<script type=\"module\" src=\"./document-glue.js\"></script>"
    } else {
        glue
    };
    let checkpoint = format!(
        "<script type=\"application/vnd.exact.checkpoint\" data-digest=\"{}\" data-activate=\"{}\">{}</script>\n{entry}",
        rendered.digest, rendered.activate.name(), rendered.checkpoint
    );
    cut(&mut html, glue, glue, &checkpoint)?;
    let viewports = html.matches("<meta name=\"viewport\"").count();
    let preloaded = ["app.wasm", "navigation.js", "glue.js"]
        .iter()
        .any(|file| html.contains(&format!("preload\" href=\"./{file}\"")));
    if viewports != 1 || preloaded || !html.contains(&rendered.head) {
        return Err("the shell no longer has the places a document goes".into());
    }
    Ok(html)
}
