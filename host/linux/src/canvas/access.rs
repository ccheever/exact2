//! The accessibility tree for the Canvas host's reader (LLP 1115.001 D13).
//! Android's reader answers TalkBack through an `AccessibilityNodeProvider`
//! (`host/android/canvas`, `Access.kt`) whose virtual nodes are the
//! presenter's ([`crate::presenter::access`]): what is in the reader's view
//! is painted, and says nothing by itself.
//!
//! The reader asks only while an accessibility service is on, with each
//! stream, as it asks for the bars and the fields; a service's press, held
//! press, scroll and range step come back by one call and go through the
//! presenter's own paths. With no service on, none of these is called and
//! the presenter builds nothing.

use super::CanvasHost;
use exact_runner::DataSource;

impl<D: DataSource + Default> CanvasHost<D> {
    /// What the reader is told of the accessibility tree, in its pixels,
    /// when that is not what it was last told
    /// ([`crate::presenter::Presenter::reader_access`] has the words).
    /// `again`: the reader holds nothing (it was made again, or a service
    /// was just turned on) and is told the tree whole.
    pub fn access(&mut self, again: bool) -> Option<Vec<i32>> {
        self.p.reader_access(self.scale, again)
    }

    /// A service acts on node `id` of the tree last told
    /// ([`crate::presenter::access::act`]): 1 press, 2 hold, 3 scroll on or
    /// step up, 4 scroll back or step down, 5 set a range to `value`.
    /// Whether anything took it. What it changed commits, and the page
    /// paints with it.
    pub fn access_act(&mut self, id: i32, action: i32, value: f32) -> bool {
        let now = self.now();
        let done = self.p.access_act(id, action, value, now);
        self.force |= done;
        done
    }

    /// No service is on any more: the presenter forgets what it told, and
    /// builds nothing until the reader asks again.
    pub fn access_off(&mut self) {
        self.p.access_reset();
    }
}
