//! Native list geometry and logical text ABI.
use super::*;

impl<D: DataSource> Bridge<D> {
    /// Canonical location from the input URL, in the output buffer. @ref LLP 1038 D8
    pub fn location_of(&mut self, len: usize) -> u32 {
        let href = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        self.emit(exact_route::location_of(&href))
    }

    /// A pre-boot location; a live session receives dispatch kind 14 instead.
    pub fn set_launch_location(&mut self, len: usize) {
        if self.host.is_none() {
            self.launch = Some(
                String::from_utf8_lossy(&self.input[..len.min(self.input.len())]).into_owned(),
            );
        }
    }

    /// Actual native list scrollport; measured rows are read from kernel layout.
    pub fn list_viewport(
        &mut self,
        view: u32,
        geometry: exact_runner::ListViewport<'_>,
        create_limit: Option<usize>,
    ) -> u32 {
        let out = self.host.as_mut().map_or_else(not_booted, |h| {
            h.list_viewport_within(view, geometry, create_limit)
        });
        self.emit(out)
    }

    /// Whether the list's last report left budgeted work over: 1 or 0.
    pub fn list_pending(&self, view: u32) -> u32 {
        u32::from(self.host.as_ref().is_some_and(|h| h.list_pending(view)))
    }

    /// Resolve an opaque list key, or return the absent-index sentinel.
    pub fn list_index(&self, view: u32, len: usize) -> u32 {
        let key = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        self.host
            .as_ref()
            .and_then(|h| h.runner().list_index(view, &key))
            .and_then(|i| u32::try_from(i).ok())
            .unwrap_or(u32::MAX)
    }

    /// Logical text for all rows (empty keys), or two UTF-16 endpoints.
    #[allow(clippy::too_many_arguments)]
    pub fn list_text(
        &mut self,
        view: u32,
        first_len: usize,
        len: usize,
        first_paragraph: usize,
        first_offset: usize,
        last_paragraph: usize,
        last_offset: usize,
    ) -> u32 {
        let bytes = &self.input[..len.min(self.input.len())];
        if first_len > bytes.len() {
            return self.emit(String::new());
        }
        let first = String::from_utf8_lossy(&bytes[..first_len]);
        let last = String::from_utf8_lossy(&bytes[first_len..]);
        let range = (first_len != 0).then_some((
            exact_runner::ListTextPosition {
                key: &first,
                paragraph: first_paragraph,
                offset: first_offset,
            },
            exact_runner::ListTextPosition {
                key: &last,
                paragraph: last_paragraph,
                offset: last_offset,
            },
        ));
        let text = self
            .host
            .as_ref()
            .and_then(|h| h.runner().list_text(view, range).ok())
            .unwrap_or_default();
        self.emit(text)
    }
}
