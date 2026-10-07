//! Multi-column boxes the kernel kept whole (LLP 1093 D10).
use super::{DataSource, Runner};

impl<D: DataSource> Runner<D> {
    /// Journal, once per box, each box a multi-column flow kept whole across
    /// a column's end where Chrome would fragment it (LLP 1093 D10).
    pub fn report_fragment_skipped(&mut self, keys: &[exact_kernel::NodeKey]) {
        for &key in keys {
            if self.fragment_warned.insert(key) {
                if let Some(node) = self.kernel.node_by_key(key) {
                    let why = self
                        .kernel
                        .fragment_refusal(key)
                        .map_or("", |r| r.message());
                    self.log(format!(
                        "columns: #{} is kept whole in its column: {why} (LLP 1093 D10)",
                        node.id
                    ));
                }
            }
        }
    }
}
