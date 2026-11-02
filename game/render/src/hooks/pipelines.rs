//! Validation happens before a candidate can replace the last usable pipelines.
use exact_gpu::wgpu;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

type Validation = Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>;

/// A device-owned set of pipelines, replaced atomically after GPU validation.
/// Call `update` from prepare, include `Needs::PENDING` while `pending()` is true,
/// and report `error()` in hook diagnostics. A rejected update preserves `get()`.
/// Dropping this cache on device loss drops both the candidate and current set.
pub struct Pipelines<T> {
    current: Option<T>,
    candidate: Option<(u32, T, Validation)>,
    attempted: Option<u32>,
    error: Option<String>,
}
impl<T> Default for Pipelines<T> {
    fn default() -> Self {
        Self {
            current: None,
            candidate: None,
            attempted: None,
            error: None,
        }
    }
}
impl<T> Pipelines<T> {
    /// Start or poll validation. `make` creates every pipeline/binding in the set.
    /// No candidate is returned for drawing until its validation future completes.
    pub fn update(&mut self, device: &wgpu::Device, generation: u32, make: impl FnOnce() -> T) {
        if self.attempted != Some(generation) {
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let candidate = make();
            self.candidate = Some((generation, candidate, Box::pin(scope.pop())));
            self.attempted = Some(generation);
        }
        if let Some((_, _, future)) = &mut self.candidate {
            if let Poll::Ready(error) = future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                let (_, value, _) = self.candidate.take().unwrap();
                self.error = error.map(|e| e.to_string());
                if self.error.is_none() {
                    self.current = Some(value);
                }
            }
        }
    }
    /// The last validated resource set, including during a replacement attempt.
    pub fn get(&self) -> Option<&T> {
        self.current.as_ref()
    }
    /// Mutable access to the last validated set for uniform uploads and resizing.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.current.as_mut()
    }
    /// Keep requesting frames while the browser validates a candidate.
    pub fn pending(&self) -> bool {
        self.candidate.is_some()
    }
    /// Named GPU validation failure from the most recently completed attempt.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
