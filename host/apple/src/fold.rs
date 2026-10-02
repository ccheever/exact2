//! The page's environment as the host sets it (LLP 1001 §2; LLP 1078 D4):
//! the safe-area insets and the viewport segments a fold makes, each a call
//! that re-sends the dictionaries of the nodes whose style reads the
//! environment and lays out again.

use super::{Batch, Host, Timed};
use exact_kernel::Env;
use exact_runner::DataSource;

impl<D: DataSource> Host<D> {
    /// The safe-area insets changed (a boot under `viewport-fit=cover`, a
    /// rotation): the kernel's environment is set, every node whose style
    /// holds an `env()` length gets its dictionary re-sent with the new
    /// points and is laid out again; the batch carries what moved. Empty
    /// when nothing reads the insets, or they did not change.
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> String {
        let mut batch = Batch::new();
        let error = match self
            .runner
            .kernel_mut()
            .set_env(Env::new(top, right, bottom, left))
        {
            Ok(false) => None,
            Ok(true) => {
                self.height_targets_dirty = true;
                for id in self.preorder() {
                    self.update(id, &mut batch);
                }
                self.emit_paragraphs(&mut batch);
                self.layout(&mut batch).err()
            }
            Err(e) => Some(format!("insets: {e:?}")),
        };
        self.finish(batch, error)
    }

    /// The device's posture and the viewport segments a fold makes (LLP
    /// 1078 D4): the kernel's grid and `exactViewport`'s three fields in one
    /// batch — the nodes whose style reads the environment get their
    /// dictionaries re-sent and are laid out again, as for the insets, and
    /// every reader of the fields recommits. Empty when nothing changed. A
    /// malformed grid (the kernel's refusal, or a count past 255) is an
    /// error on the batch and changes nothing.
    pub fn set_segments(
        &mut self,
        posture: u32,
        cols: u32,
        rows: u32,
        rects: Vec<exact_kernel::Rect>,
    ) -> String {
        let (Ok(c), Ok(r)) = (u8::try_from(cols), u8::try_from(rows)) else {
            return self.finish(
                Batch::new(),
                Some(format!("segments: {cols}x{rows} is past the kernel's grid")),
            );
        };
        let mut batch = Batch::new();
        let restyled = match self.runner.kernel_mut().set_segments(c, r, rects) {
            Ok(changed) => changed,
            Err(e) => return self.finish(batch, Some(format!("segments: {e:?}"))),
        };
        if restyled {
            self.height_targets_dirty = true;
            for id in self.preorder() {
                self.update(id, &mut batch);
            }
            self.emit_paragraphs(&mut batch);
        }
        let fold = exact_runner::Fold {
            posture: exact_runner::Posture::from_bits(posture),
            cols,
            rows,
        };
        match self.runner.set_fold(fold) {
            Ok(Some(receipt)) => self.commit_into(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
                batch,
            ),
            Ok(None) if restyled => {
                let error = self.layout(&mut batch).err();
                self.finish(batch, error)
            }
            Ok(None) => self.finish(batch, None),
            Err(e) => self.finish(batch, Some(format!("segments: {e:?}"))),
        }
    }
}
