//! The stress consumer's single root document, on the existing ordered lane.
use super::{DocumentArgs, MarkdownStress, Parsed};
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Response, Store};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, Weak,
};

// No source/runtime replacement can accept an older source's same-key ACK.
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn reserve_serial(counter: &AtomicU64) -> Result<u64, DataError> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map(|n| n + 1)
        .map_err(|_| DataError::Unavailable("native document serial exhausted".into()))
}

enum Phase {
    Waiting,
    Running,
    Ready(Result<Parsed, DataError>),
    Taken,
}

struct CellState {
    active: bool,
    phase: Phase,
}

struct Cell {
    key: (super::Profile, usize, usize),
    state: Mutex<CellState>,
    running: Arc<AtomicBool>,
    #[cfg(test)]
    probe: Arc<tests::Probe>,
}

struct Running(Arc<AtomicBool>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Cell {
    fn cancel(&self) {
        let retired = {
            let mut state = self.state.lock().unwrap();
            state.active = false;
            std::mem::replace(&mut state.phase, Phase::Taken)
        };
        drop(retired); // A typed document is never destroyed under the cell lock.
    }
}

struct Pending {
    serial: u64,
    args: DocumentArgs,
    claimed: bool,
    cell: Arc<Cell>,
}

/// Explicit native runtime opt-in for Markdown stress's one always-live resource.
///
/// Bake, web and the synchronous pathological control retain [`MarkdownStress`].
/// Cold `answer` calls yield generation/parsing to the host's existing ordered
/// continuation lane; no thread/pool is created here. One accepted document and
/// one latest result cell are retained. Queued work has weak cell references;
/// obsolete work skips parsing, and already-running obsolete work drops its
/// result. The host's bounded closure queue is a separate category.
///
/// Same-key page/eager replacements share the parse but receive new ACK serials.
/// This is not a generic resource-cancellation hook: this app replaces interest
/// through `answer`, or drops the source with its runtime. Live parsing cannot
/// be preempted. Projection into `Value`, previous-value destruction and later
/// Runner/kernel work remain on the UI thread. Direct synchronous `query` is
/// available only without pending document work, to prevent duplicate parsing.
#[derive(Default)]
pub struct NativeMarkdownStress {
    sync: MarkdownStress,
    pending: Option<Pending>,
    // Retired work can still own its allocation after the current cell changes.
    running: Arc<AtomicBool>,
    #[cfg(test)]
    probe: Arc<tests::Probe>,
    #[cfg(test)]
    serial_counter: Option<Arc<AtomicU64>>,
}

impl NativeMarkdownStress {
    fn cancel(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.cell.cancel();
        }
    }

    fn serial(&self) -> Result<u64, DataError> {
        #[cfg(test)]
        if let Some(counter) = &self.serial_counter {
            return reserve_serial(counter);
        }
        reserve_serial(&SERIAL)
    }
}

impl Drop for NativeMarkdownStress {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl DataSource for NativeMarkdownStress {
    fn app_id(&self) -> &str {
        self.sync.app_id()
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == "theme" && args.is_empty() {
            return self.sync.query(source, args);
        }
        DocumentArgs::read(source, args)?;
        if self.pending.is_some() || self.running.load(Ordering::Acquire) {
            return Err(DataError::Unavailable(
                "native document work is pending".into(),
            ));
        }
        self.sync.query(source, args)
    }

    fn answer(
        &mut self,
        _store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if source == "theme" && args.is_empty() {
            return self.sync.query(source, args).map(Answer::Now);
        }
        let args = DocumentArgs::read(source, args)?;
        if self.sync.parsed.as_ref().is_some_and(|p| p.key == args.key) {
            self.cancel();
            return Ok(Answer::Now(self.sync.parsed.as_ref().unwrap().value(args)));
        }
        // Candidate validation and reservation precede any interest/cell change.
        let serial = self.serial()?;
        let same = self.pending.as_ref().filter(|p| p.args.key == args.key);
        let cell = same.map(|p| p.cell.clone()).unwrap_or_else(|| {
            Arc::new(Cell {
                key: args.key,
                state: Mutex::new(CellState {
                    active: true,
                    phase: Phase::Waiting,
                }),
                running: self.running.clone(),
                #[cfg(test)]
                probe: self.probe.clone(),
            })
        });
        if same.is_none() {
            self.cancel();
        }
        self.pending = Some(Pending {
            serial,
            args,
            claimed: false,
            cell,
        });
        Ok(Answer::Later(Request::continuation(serial)))
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let pending = self.pending.as_mut()?;
        if token != pending.serial || pending.claimed {
            return None;
        }
        pending.claimed = true;
        let cell = Arc::downgrade(&pending.cell);
        Some(Box::new(move || run(cell, token)))
    }

    fn parse(
        &mut self,
        _store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let args = DocumentArgs::read(source, args)?;
        let pending = self
            .pending
            .as_ref()
            .ok_or_else(|| unavailable("no native document pending"))?;
        if pending.args != args || !pending.claimed {
            return Err(unavailable("native document arguments or claim mismatch"));
        }
        match outcome {
            Outcome::Response(response)
                if response.status == 200
                    && response.headers.is_empty()
                    && response.body.as_slice() == pending.serial.to_le_bytes() => {}
            Outcome::Failed { message, .. } => {
                self.cancel();
                return Err(unavailable(&message));
            }
            _ => return Err(unavailable("native document ACK mismatch")),
        }
        let result = {
            let mut state = pending.cell.state.lock().unwrap();
            if !state.active || !matches!(state.phase, Phase::Ready(_)) {
                return Err(unavailable("native document result is not ready"));
            }
            let Phase::Ready(result) = std::mem::replace(&mut state.phase, Phase::Taken) else {
                unreachable!()
            };
            result
        };
        self.cancel();
        self.sync.parsed = Some(result?);
        Ok(Answer::Now(self.sync.parsed.as_ref().unwrap().value(args)))
    }
}

fn unavailable(message: &str) -> DataError {
    DataError::Unavailable(message.into())
}

fn ack(serial: u64) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: serial.to_le_bytes().to_vec(),
    })
}

fn run(cell: Weak<Cell>, serial: u64) -> Outcome {
    let Some(cell) = cell.upgrade() else {
        return ack(serial);
    };
    let _running = {
        let mut state = cell.state.lock().unwrap();
        if !state.active || matches!(state.phase, Phase::Ready(_)) {
            return ack(serial);
        }
        if !matches!(state.phase, Phase::Waiting)
            || cell
                .running
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            // Native continuations must run on the one existing ordered lane.
            // Refuse a violating caller instead of starting a second parse.
            return Outcome::Failed {
                kind: FailureKind::Refused,
                message: "overlapping native document continuation".into(),
            };
        }
        state.phase = Phase::Running;
        Running(cell.running.clone())
    };
    #[cfg(test)]
    let _active = cell.probe.enter();
    let result = Parsed::build(cell.key);
    #[cfg(test)]
    let result = result.map(|mut parsed| {
        cell.probe.built(&mut parsed);
        parsed
    });
    let mut result = Some(result);
    {
        let mut state = cell.state.lock().unwrap();
        if state.active {
            state.phase = Phase::Ready(result.take().unwrap());
        }
    }
    drop(result); // Cancelled in flight: dispose even if Runner drops the ACK.
    ack(serial)
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
