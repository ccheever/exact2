//! Pictures of a package's `img` scheme (LLP 1133.019 D2, D11 item 1): a
//! source such as `photos:<id>` is never a file. Each image node that shows
//! one is asked of the package's implementation at its box's device pixels
//! and fit, through admission ([`Images::package_asks`] is the outbox the
//! presenter admits and sends); the answer is a decoded picture, charged to
//! the session's raster budget like any decode, or a failure, which empties
//! the box. An implementation may announce a picture changed (it is asked
//! again) or revoke it (every box showing it is emptied, and no later reply
//! to it draws).
use super::{Bitmap, Images, Report};
use exact_kernel::{ObjectFit, ViewId};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One picture to ask for: the presenter admits it, then sends it.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// What [`Images::package_answer`] names the answer by.
    pub ticket: u64,
    /// The `img` source, such as `photos:<id>`.
    pub source: String,
    /// The box's device pixels.
    pub width: u32,
    /// The box's device pixels.
    pub height: u32,
    /// `object-fit`'s word: what the implementation draws the picture for.
    pub fit: &'static str,
}

/// Where a view's package picture is.
#[derive(Debug, Default)]
struct Wanted {
    source: String,
    /// The device pixels asked for last.
    asked: (u32, u32),
    /// The ask in flight, whose answer alone is taken.
    ticket: Option<u64>,
    /// A picture is shown.
    shown: bool,
    /// The implementation failed or revoked it: not asked again until the
    /// source changes or is announced.
    failed: bool,
    /// Admission refused it: asked again after the next commit, by which
    /// the app may have been granted it or the implementation bound.
    refused: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Pictures {
    /// The schemes packages serve (`photos`), as the plan declares them.
    schemes: Vec<String>,
    views: BTreeMap<ViewId, Wanted>,
    asks: Vec<Ask>,
    next: u64,
    /// The kernel epoch of the last sync: a commit since lets refusals be
    /// asked again.
    epoch: Option<u64>,
}

fn fit_word(fit: ObjectFit) -> &'static str {
    match fit {
        ObjectFit::Contain => "contain",
        ObjectFit::Cover => "cover",
        ObjectFit::Fill => "fill",
        ObjectFit::None => "none",
        ObjectFit::ScaleDown => "scale-down",
    }
}

impl Pictures {
    /// Whether `source` is of a package's scheme.
    pub(crate) fn serves(&self, source: &str) -> bool {
        source
            .split_once(':')
            .is_some_and(|(scheme, _)| self.schemes.iter().any(|s| s == scheme))
    }

    /// A sync at kernel `epoch` began.
    pub(super) fn begin(&mut self, epoch: u64) {
        if self.epoch != Some(epoch) {
            self.epoch = Some(epoch);
            for w in self.views.values_mut() {
                w.refused = false;
            }
        }
    }

    /// View `id` shows `source` in a box of `desired` device pixels at
    /// `fit`: asked for when it shows and has no picture, or a picture much
    /// smaller than the box now is.
    pub(super) fn want(
        &mut self,
        id: ViewId,
        source: &str,
        desired: (f32, f32),
        fit: ObjectFit,
        visible: bool,
    ) {
        let w = self.views.entry(id).or_default();
        let changed = w.source != source;
        if changed {
            *w = Wanted {
                source: source.to_owned(),
                ..Wanted::default()
            };
        }
        let pixels = (
            desired.0.ceil().max(1.) as u32,
            desired.1.ceil().max(1.) as u32,
        );
        let grown = w.shown && (pixels.0 > w.asked.0 * 3 / 2 || pixels.1 > w.asked.1 * 3 / 2);
        if visible && w.ticket.is_none() && !w.failed && !w.refused && (!w.shown || grown) {
            self.next += 1;
            w.ticket = Some(self.next);
            w.asked = pixels;
            self.asks.push(Ask {
                ticket: self.next,
                source: source.to_owned(),
                width: pixels.0,
                height: pixels.1,
                fit: fit_word(fit),
            });
        }
    }

    pub(super) fn forget(&mut self, id: ViewId) {
        self.views.remove(&id);
    }

    /// No view and nothing asked, the schemes kept: a reset generation's.
    pub(super) fn reset(self) -> Self {
        Self {
            schemes: self.schemes,
            next: self.next,
            ..Self::default()
        }
    }

    /// A replacement generation serves the schemes this one did.
    pub(super) fn inherit(&mut self, from: &Pictures) {
        self.schemes = from.schemes.clone();
    }
}

impl Images {
    /// The schemes packages serve here, from the plan's interfaces.
    pub fn set_package_schemes(&mut self, schemes: Vec<String>) {
        self.package.schemes = schemes;
    }

    /// The pictures to ask for since the last call.
    pub fn package_asks(&mut self) -> Vec<Ask> {
        std::mem::take(&mut self.package.asks)
    }

    /// Ask `ticket`'s answer: the picture, RGBA premultiplied, or why not
    /// (`refused` by admission, else the implementation's failure).
    pub fn package_answer(
        &mut self,
        ticket: u64,
        answer: Result<tiny_skia::Pixmap, (bool, String)>,
    ) -> Vec<Report> {
        let Some((&id, w)) =
            (self.package.views.iter_mut()).find(|(_, w)| w.ticket == Some(ticket))
        else {
            return Vec::new();
        };
        w.ticket = None;
        match answer {
            Ok(pixels) => {
                let natural = (pixels.width(), pixels.height());
                let bytes = u64::from(natural.0) * u64::from(natural.1) * 4;
                let Ok(reservation) = self.backend.session.reserve_allocation(bytes) else {
                    // No room in the budget now: asked again at a later sync.
                    return Vec::new();
                };
                let Ok(charge) = reservation.commit(bytes) else {
                    return Vec::new();
                };
                w.shown = true;
                self.bitmaps
                    .insert(id, Arc::new(Bitmap::decoded(pixels, natural, charge)));
                vec![(id, Some((natural.0 as f32, natural.1 as f32)))]
            }
            Err((refused, _)) => {
                w.refused = refused;
                w.failed = !refused;
                w.shown = false;
                self.bitmaps.remove(&id);
                vec![(id, None)]
            }
        }
    }

    /// The implementation announced `source` changed (asked again) or
    /// revoked it (every box showing it emptied; a reply in flight dropped).
    pub fn package_changed(&mut self, source: &str, revoked: bool) -> Vec<Report> {
        let mut reports = Vec::new();
        for (id, w) in &mut self.package.views {
            if w.source != source {
                continue;
            }
            w.ticket = None;
            w.failed = revoked;
            if revoked {
                w.shown = false;
                self.bitmaps.remove(id);
                reports.push((*id, None));
            } else {
                // Asked again at the next sync; the old picture shows meanwhile.
                w.shown = false;
            }
        }
        // The next sync runs, though nothing committed.
        self.settled = None;
        reports
    }
}

#[cfg(test)]
#[path = "package_tests.rs"]
mod tests;
