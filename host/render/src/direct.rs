//! A JavaScript page written from its runner's instance tree, with no
//! kernel, and sent as it is written (LLP 1048.004 D4, Q3).
//!
//! The server's navigation path flushes a page's head before its render
//! (LLP 1071 D6). Here the rest follows in the order the page holds it: the
//! head's fields and what else goes before the document — known from the
//! settled tree before any element is written — then the root, handed on
//! every [`exact_web::document::STREAM_CHUNK`] as the walk writes it in the
//! runtime's form, then the checkpoint. The bytes are the page
//! [`crate::page::body_js`] composes from a kernel render, but for the
//! digest, which is over the runtime's form of the root (the JavaScript
//! runtime never reads it).

use crate::page::{body_close_js, body_open_js, runtime_style_of, Js};
use crate::{activation, direct_for, encoded, retire, settle_at, Ids, Projection, Rendered};
use exact_plan::{ActivatePolicy, Plan};
use exact_runner::DataSource;
use exact_web::document::{
    before_root, digest, project_tree, read_checkpoint, Document, Site, Writing,
};
use std::time::Duration;

/// Render `location` and write its page after the head a server flushed
/// (`body_js`'s bytes), handing each run to `send` as it is written.
/// `late`: whether the page, by the activation its render decides, needs
/// the entry's preloads its early head left out. `Ok(None)` when the page
/// is not written without a kernel (the plan, or a tree the fold doesn't
/// cover); nothing was sent, and the caller renders it with a kernel.
/// `limit` bounds what is sent: a page past it is the error, with what went
/// before it sent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_js<D: DataSource + 'static, F: Fn() -> D>(
    plan: &Plan,
    data: &F,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
    shell: &str,
    js: &Js,
    late: impl Fn(ActivatePolicy) -> bool,
    limit: usize,
    send: &mut dyn FnMut(&[u8]),
) -> Result<Option<(Rendered, String)>, String> {
    if !direct_for(plan, Ids::Any, Projection::Auto)? {
        return Ok(None);
    }
    let page = settle_at(
        plan,
        data,
        viewport,
        location,
        deadline,
        true,
        crate::render_time(),
    )?;
    let tree = match page.runner.document_tree() {
        Ok(tree) => tree,
        Err(why) => {
            println!("render {location}: with a kernel ({why})");
            retire(page, data);
            return Ok(None);
        }
    };
    let state = read_checkpoint(&page.checkpoint).map_err(|e| format!("the checkpoint: {e}"))?;
    let handlers = tree.handlers(plan);
    let activate = activation(plan, location, &state, &handlers);
    // The head's fields, before any element: the tree's head, its first
    // root's viewport policies, and the `@keyframes` its elements name.
    let (keyframes, scroll) = before_root(&tree);
    let runner = &page.runner;
    let first = |id| {
        tree.roots()
            .first()
            .and_then(|root| tree.node(*root))
            .and_then(|n| n.props.str(id).map(str::to_owned))
    };
    let ahead = Document {
        lang: runner.resolved_locale().into(),
        dir: runner.direction().into(),
        root: String::new(),
        viewport_fit: first(exact_kernel::PropId::ViewportFit),
        interactive_widget: first(exact_kernel::PropId::InteractiveWidget),
        head: tree.head(),
        keyframes,
        scroll_document: scroll,
    };
    let head = ahead
        .page_head(plan, site, location)
        .map_err(|e| e.to_string())?;
    let mut sent = 0usize;
    let mut over = false;
    let mut hand = |bytes: &[u8]| {
        sent += bytes.len();
        if sent > limit {
            over = true;
        } else {
            send(bytes);
        }
    };
    let open = body_open_js(shell, js, &head, scroll, late(activate), false);
    hand(open.as_bytes());
    let rewrite = runtime_style_of(js);
    let written = {
        let mut sink = |chunk: &str| hand(chunk.as_bytes());
        project_tree(
            &tree,
            runner,
            Writing {
                style: Some(&rewrite),
                sink: Some(&mut sink),
            },
        )
    };
    let (document, rest) = match written {
        Ok(done) => done,
        Err(e) => {
            retire(page, data);
            return Err(e.to_string());
        }
    };
    hand(rest.as_bytes());
    let checkpoint = page.checkpoint.clone();
    let digest = digest(&encoded(plan), location, &checkpoint, &document.root);
    let close = body_close_js(shell, js, &digest, activate, &checkpoint);
    hand(close.as_bytes());
    let settled = page.settled;
    retire(page, data);
    if over {
        return Err(format!("the page is {sent} bytes"));
    }
    let body = open + &document.root + &close;
    Ok(Some((
        Rendered {
            document,
            head,
            checkpoint,
            digest,
            state,
            settled,
            activate,
            runtime_form: true,
        },
        body,
    )))
}
