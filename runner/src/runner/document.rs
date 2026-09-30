//! The document of a runner that mirrors no kernel (LLP 1048.004).
use super::{DataSource, Runner};
use crate::instance::{DocTree, DocTreeError};

impl<D: DataSource> Runner<D> {
    /// The current tree's document nodes, folded from its bindings' values:
    /// what a render writes when its kernel is detached
    /// (`exact_kernel::Kernel::detached`). A tree the fold doesn't cover, or
    /// one a kernel would have refused, is the error: render it with a
    /// kernel instead.
    pub fn document_tree(&self) -> Result<DocTree, DocTreeError> {
        self.tree
            .as_ref()
            .ok_or(DocTreeError::Unsupported("a runner with no tree"))?
            .document(&self.plan, &self.sites)
    }
}
