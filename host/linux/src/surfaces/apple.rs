//! CPU frames use the same layer while Metal's archive miss compiles.
//! @ref LLP 1149 D6.3; LLP 1149.001 Stage 0b.
use exact_ink::device::{
    apple::{Layer, Startup},
    Error, Retry, Selected,
};
use std::{sync::Arc, time::Instant};

type Wake = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct Boot {
    pub preparing: Option<Startup>,
    pub layer: Option<Layer>,
    pub recovering: bool,
    pub last_refusal: Option<String>,
    pub presented: Option<&'static str>,
    retry: Retry,
    wake: Wake,
    open: fn(Wake) -> exact_ink::device::Result<Startup>,
}

impl Boot {
    pub fn new(preparing: Startup) -> Self {
        Self {
            wake: preparing.waker(),
            preparing: Some(preparing),
            layer: None,
            recovering: false,
            last_refusal: None,
            presented: None,
            retry: Retry::default(),
            open: Startup::start,
        }
    }

    pub fn refused(&mut self, error: Error, now: Instant) {
        self.last_refusal = Some(error.to_string());
        if matches!(error, Error::DeviceLost(_)) {
            if let Some(layer) = &self.layer {
                layer.invalidate();
            }
            // Drop the failed generation, including queued CPU work and its compiler.
            self.preparing = None;
            self.recovering = true;
            self.retry.failed(now);
        }
    }

    pub fn poll(&mut self, now: Instant) -> Option<Selected> {
        if self.recovering && self.preparing.is_none() && self.retry.ready(now) {
            match (self.open)(self.wake.clone()) {
                Ok(mut preparing) => {
                    if let Some(layer) = &self.layer {
                        preparing.attach(layer.clone());
                    }
                    self.preparing = Some(preparing);
                }
                Err(error) => {
                    self.last_refusal = Some(error.to_string());
                    self.retry.failed(now);
                }
            }
        }
        let preparing = self.preparing.as_mut()?;
        match preparing.poll() {
            Ok(Some(device)) => {
                self.preparing = None;
                self.recovering = false;
                self.retry = Retry::default();
                Some(device)
            }
            Ok(None) => None,
            Err(error) => {
                self.refused(error, now);
                None
            }
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        (self.recovering && self.preparing.is_none())
            .then(|| self.retry.deadline())
            .flatten()
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn cpu_device_loss_discards_the_generation_and_retries_with_bounded_backoff() {
        static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
        let preparing = Startup::start(Arc::new(|| {})).unwrap();
        let mut boot = Boot::new(preparing);
        boot.open = |_| {
            ATTEMPTS.fetch_add(1, Ordering::Relaxed);
            Err(Error::NoAdapter("injected unavailable device".into()))
        };
        let now = Instant::now();
        boot.refused(Error::DeviceLost("injected CPU failure".into()), now);
        assert!(boot.preparing.is_none());
        assert!(boot.recovering);
        assert!(boot
            .last_refusal
            .as_ref()
            .unwrap()
            .contains("injected CPU failure"));
        assert_eq!(boot.deadline(), Some(now + Duration::from_millis(250)));
        boot.poll(now);
        assert_eq!(ATTEMPTS.load(Ordering::Relaxed), 0);
        for delay in [500, 1000, 2000, 4000, 8000, 16000, 30000, 30000] {
            let due = boot.deadline().unwrap();
            boot.poll(due);
            assert_eq!(boot.deadline(), Some(due + Duration::from_millis(delay)));
        }
        assert_eq!(ATTEMPTS.load(Ordering::Relaxed), 8);
        boot.open = Startup::start;
        boot.poll(boot.deadline().unwrap());
        assert!(
            boot.preparing.is_some() || !boot.recovering,
            "recreation must resume startup or install ink"
        );
    }
}
