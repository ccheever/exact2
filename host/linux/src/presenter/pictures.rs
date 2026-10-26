//! Pictures of a package's `img` scheme through the shell (LLP 1133.019 D2,
//! D11 item 1): what the image sync asks for (`image/package.rs`) is
//! admitted by the runner, then asked of the reader's extension as a module
//! request (`{"extension", "picture", "width", "height", "fit", "owned",
//! "agent"}`), which answers with its pixels ([`Presenter::picture_answered`])
//! or why it has none. A refusal empties the box, as an unresolved image.
use super::*;

/// The one host whose shell serves package pictures through this door: the
/// Android reader.
const TARGET: &str = "android";

impl<D: DataSource> Presenter<D> {
    /// The plan's picture schemes, served by the shell's extensions from now
    /// on (`CanvasHost::bind_extensions`).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn bind_pictures(&mut self) {
        let schemes = self.host.runner().picture_schemes();
        self.images.set_package_schemes(schemes);
        self.images.settled = None;
        self.resync_pictures();
    }

    /// The image sync again, outside a commit: what was announced or bound
    /// is asked for now.
    fn resync_pictures(&mut self) {
        self.dirty = true;
        if let Some(e) = self.sync_images() {
            self.host.log(e);
        }
    }

    /// The pictures the last image sync asked for: each admitted and asked
    /// of the shell, or refused. The boxes a refusal empties.
    pub(crate) fn pictures_out(&mut self) -> Vec<crate::image::Report> {
        let mut reports = Vec::new();
        let mut asked = false;
        for ask in self.images.package_asks() {
            let admitted = (self.host.runner_mut())
                .admit_picture(&ask.source, TARGET)
                .unwrap_or_default();
            let refusal = match admitted["error"].as_str() {
                Some(_) => Some(admitted["message"].as_str().unwrap_or("refused").to_owned()),
                None if !self.requests.shell => Some("no shell serves pictures".into()),
                None => None,
            };
            if let Some(why) = refusal {
                reports.extend(self.images.package_answer(ask.ticket, Err((true, why))));
                continue;
            }
            let body = serde_json::json!({
                "extension": admitted["extension"],
                "picture": ask.source,
                "width": ask.width,
                "height": ask.height,
                "fit": ask.fit,
                "owned": admitted["owned"].as_bool().unwrap_or(false),
                "agent": admitted["agent"],
            });
            let id = self.queue(Request::Module(body.to_string()));
            (self.requests.waiting).insert(id, Waiting::Picture(ask.ticket));
            asked = true;
        }
        if asked {
            self.executor.notify();
        }
        reports
    }

    /// The shell's answer to picture request `id`: its pixels, RGBA
    /// premultiplied, or the implementation's error. Whether it waited.
    pub fn picture_answered(&mut self, id: u32, answer: Result<tiny_skia::Pixmap, String>) -> bool {
        let Some(Waiting::Picture(ticket)) = self.requests.waiting.get(&id).copied() else {
            return false;
        };
        self.requests.waiting.remove(&id);
        let reports = (self.images).package_answer(ticket, answer.map_err(|why| (false, why)));
        self.apply_reports(reports);
        true
    }

    /// An implementation announced picture `source` changed (asked again)
    /// or revoked it (every box showing it emptied), as
    /// `ExtensionContext.revokePicture` and `changedPicture` say.
    pub fn picture_changed(&mut self, source: &str, revoked: bool) {
        let reports = self.images.package_changed(source, revoked);
        self.apply_reports(reports);
        self.resync_pictures();
    }

    /// A picture request cancelled, or answered with JSON (`{"error"}`)
    /// instead of pixels: its box emptied, and a cancelled one asked again
    /// after the next commit.
    pub(super) fn picture_cancelled(&mut self, ticket: u64, answer: Option<&str>) {
        let why = answer
            .and_then(|a| serde_json::from_str::<serde_json::Value>(a).ok())
            .and_then(|v| v["error"].as_str().map(str::to_owned));
        let refused = answer.is_none();
        let why = why.unwrap_or_else(|| "cancelled".into());
        let reports = self.images.package_answer(ticket, Err((refused, why)));
        self.apply_reports(reports);
    }
}
