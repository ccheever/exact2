//! The element resize event's rounds after layout (`exact_runner`'s
//! `resize`), run before a batch is finished, so the presenter hears the
//! resized handlers' commits in the batch whose layout raised them.
use super::*;

impl<D: DataSource> Host<D> {
    /// Deliver the element resize event as the browser's loop does (Resize
    /// Observer 1 §3.4.1): every `resize` node whose content box changed,
    /// each round deeper than the last one's shallowest, each round's
    /// commits laid out and presented into `batch` before the next looks.
    pub(super) fn resize_rounds(
        &mut self,
        mut batch: Batch,
        mut error: Option<String>,
    ) -> (Batch, Option<String>) {
        let mut depth = 0;
        loop {
            let due = self.runner.resize_due(depth);
            let Some(&(_, _, shallowest)) = due.first() else {
                break;
            };
            depth = shallowest + 1;
            let mut receipts = Vec::new();
            for (view, rect, _) in due {
                self.runner.resize_delivered(view, rect);
                match self.runner.dispatch(view, Event::Resize(rect)) {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(e) => {
                        error.get_or_insert(format!("{e:?}"));
                    }
                }
            }
            if receipts.is_empty() {
                continue;
            }
            let (next, e) = self.commit_tree(&receipts, None, batch);
            batch = next;
            let arrange = self.arrange_after_commit(&mut batch);
            error = error.or(e).or(arrange);
            self.persist();
            self.emit_language(&mut batch);
            self.emit_transform_drags(&mut batch);
            self.present(&mut batch, false);
        }
        self.runner.resize_settled();
        (batch, error)
    }
}
