//! Transport admission and physical-retirement accounting for one Exact lane.
//! @ref LLP 1120#d1-ibex2-an-asynchronous-transport-and-a-fetch-state-machine — the runner retains scheduling.
use super::*;

impl AsyncTransport for Tracked {
    fn start(&self, request: HopRequest, sink: Box<dyn Sink>) -> Result<Exchange, HostError> {
        let origin = origin(&request.url);
        let sends = request.body.is_some();
        {
            let mut state = self.machine.shared.state.lock().unwrap();
            let book = &mut state.book;
            // The places the starter held, the origin's too when it was
            // not known then (a redirect's).
            match book.reserved.remove(&self.run) {
                Some(Some(_)) => {}
                Some(None) => *book.origins[self.lane].entry(origin.clone()).or_default() += 1,
                None => {
                    *book.origins[self.lane].entry(origin.clone()).or_default() += 1;
                    if self.lane == 1 {
                        book.in_flight += 1;
                    }
                }
            }
            if let Some(r) = find(&mut state.running, self.run) {
                r.live += 1;
            }
            if let Some(u) = state.book.uploads.get_mut(&self.run).filter(|_| sends) {
                u.hops += 1;
            }
        }
        let sink = Box::new(TrackedSink {
            inner: sink,
            machine: self.machine.clone(),
            lane: self.lane,
            run: self.run,
            origin: Some(origin.clone()),
            sends,
        });
        let exchange = match self.inner.start(request, sink) {
            Ok(exchange) => Arc::new(exchange),
            Err(error) => {
                self.machine
                    .post(self.lane, Event::Ended(self.run, Some(origin), sends));
                return Err(error);
            }
        };
        let first = self.deadline.lock().unwrap().take();
        let (machine, lane, run) = (self.machine.clone(), self.lane, self.run);
        let activate = exchange.clone();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let activated_cancelled = cancelled.clone();
        Ok(Exchange::new(
            move || {
                // Upload sizing and parking precede preparation. The one
                // deadline starts only when its first accepted hop activates.
                if !activated_cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    if let Some((ms, abort)) = first {
                        machine.timer.arm(ms, lane, run, abort);
                    }
                }
                activate.activate();
            },
            move || {
                cancelled.store(true, std::sync::atomic::Ordering::Release);
                exchange.cancel();
            },
        ))
    }
}

struct TrackedSink {
    inner: Box<dyn Sink>,
    machine: Arc<Machine>,
    lane: usize,
    run: Run,
    origin: Option<String>,
    /// It sends the run's upload.
    sends: bool,
}

impl Sink for TrackedSink {
    fn head(&mut self, head: Head) -> Flow {
        self.inner.head(head)
    }
    fn data(&mut self, bytes: &[u8]) -> Flow {
        self.inner.data(bytes)
    }
    fn end(&mut self, result: Result<(), HostError>) {
        // The release is the finisher's; posted before the task hears the
        // end, so a next hop is queued after it.
        let origin = self.origin.take();
        self.machine
            .post(self.lane, Event::Ended(self.run, origin, self.sends));
        self.inner.end(result);
    }
    fn metrics(&mut self, metrics: HopMetrics) {
        self.inner.metrics(metrics);
    }
}
