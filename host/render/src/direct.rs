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
use exact_plan::{ActivatePolicy, PaintPolicy, Plan};
use exact_runner::DataSource;
use exact_web::document::{
    before_root, digest, project_tree, read_checkpoint, route_at, Document, Site, Writing,
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
    // A route that paints its boot document first (`paint=boot`, LLP
    // 1048.005): the page with every answer still to come, its placeholders
    // showing, goes before the render waits on anything — after the head's
    // fields, which are the boot document's — in an `#exact-root` marked
    // `data-boot`. The settled page that follows hides it (a style, for a
    // reader without JavaScript) and removes it (a hashed inline script)
    // before the runtime adopts. A page with nothing to wait for has no
    // boot document: it settles as it boots.
    let paints_boot = route_at(plan, location).is_some_and(|r| r.paint == PaintPolicy::Boot);
    let mut boot: Option<Boot> = None;
    let page = {
        let mut on_boot = |runner: &exact_runner::Runner<crate::Anonymous<D>>| {
            if runner.pending().is_empty() {
                return;
            }
            let Ok(tree) = runner.document_tree() else {
                return;
            };
            let rewrite = runtime_style_of(js);
            let writing = Writing {
                style: Some(&rewrite),
                sink: None,
            };
            let Ok((document, _)) = project_tree(&tree, runner, writing) else {
                return;
            };
            let Ok(head) = document.page_head(plan, site, location) else {
                return;
            };
            let (keyframes, scroll) = before_root(&tree);
            let lead = body_open_js(shell, js, &head, scroll, false, false);
            let lead = lead.strip_suffix(ROOT_OPEN).unwrap_or(&lead);
            let bytes = format!(
                "{lead}<div id=\"exact-root\" data-boot=\"\">{}</div>",
                document.root
            );
            send(bytes.as_bytes());
            boot = Some(Boot {
                sent: bytes.len(),
                keyframes,
                scroll,
            });
        };
        settle_at(
            plan,
            data,
            viewport,
            location,
            deadline,
            true,
            crate::render_time(),
            paints_boot.then_some(&mut on_boot as &mut dyn FnMut(&_)),
        )?
    };
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
    let mut sent = boot.as_ref().map_or(0, |b| b.sent);
    let mut over = false;
    let mut hand = |bytes: &[u8]| {
        sent += bytes.len();
        if sent > limit {
            over = true;
        } else {
            send(bytes);
        }
    };
    let late = late(activate);
    // The page as a cache keeps it: the settled head, no boot document.
    let open = body_open_js(shell, js, &head, scroll, late, false);
    match &boot {
        // After a boot document, the head's fields went with it: what the
        // settled page adds is the `@keyframes` its elements name beyond the
        // boot document's, and the scroll policy if only it holds one.
        Some(b) => {
            let mut more = String::from(SHELL_HIDDEN);
            if ahead.keyframes != b.keyframes && !ahead.keyframes.is_empty() {
                more.push_str(&format!("<style>{}</style>", ahead.keyframes));
            }
            more.push_str(&body_open_js(
                shell,
                js,
                "",
                scroll && !b.scroll,
                late,
                false,
            ));
            hand(more.as_bytes());
        }
        None => hand(open.as_bytes()),
    }
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
    if boot.is_some() {
        // Right after the settled root's `</div>`: before the runtime runs.
        let (root_end, rest) = close.split_at("</div>".len());
        hand(format!("{root_end}<script>{}</script>{rest}", boot_swap_js()).as_bytes());
    } else {
        hand(close.as_bytes());
    }
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

/// A boot document sent ahead of its settled page: its length, and what of
/// the head's fields the settled page need not repeat.
struct Boot {
    sent: usize,
    keyframes: String,
    scroll: bool,
}

/// What [`body_open_js`] ends with: the root's opening tag.
const ROOT_OPEN: &str = "<div id=\"exact-root\">";

/// What hides a boot document once the settled page arrives, JavaScript or
/// none.
const SHELL_HIDDEN: &str = "<style>#exact-root[data-boot]{display:none}</style>";

/// The inline script that removes a boot document before the runtime adopts
/// the settled page: each press or edit the capture script queued on a boot
/// element moves to the settled element at the same place (an edit with the
/// control's value), and focus follows. The server's CSP admits it by hash.
pub fn boot_swap_js() -> &'static str {
    concat!(
        "(()=>{let d=document,b=d.querySelector(\"#exact-root[data-boot]\"),",
        "s=d.querySelector(\"#exact-root:not([data-boot])\"),",
        "m=e=>{let p=[];while(e&&e!=b){p.unshift([...e.parentNode.children].indexOf(e));e=e.parentElement}",
        "if(!e)return null;let t=s;for(let n of p)t=t?.children[n];return t},",
        "f=m(d.activeElement);",
        "for(let i of self.exact?.q??[]){let t=m(i.target);",
        "if(t){if(i.type!=\"click\"){t.value=i.target.value;t.checked=i.target.checked}i.target=t}}",
        "b.remove();f?.focus()})()"
    )
}
