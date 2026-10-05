//! Ownership of calls whose answers outlive one runner turn.
use super::*;

impl Module {
    pub(super) fn take_request(&mut self, ticket: u64) -> Option<Request> {
        let pos = self.host.requests.iter().position(|(t, _)| *t == ticket)?;
        Some(self.host.requests.remove(pos).1)
    }

    pub(super) fn turn_open(&self) -> bool {
        self.parked.iter().any(|(_, p)| p.ticket == 0)
    }

    pub(super) fn deferred_work() -> Dispatch {
        Dispatch::Run(Work::Now(Box::new(|| {
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: Vec::new(),
            })
        })))
    }

    pub(super) fn key(target: Option<Target>, source: &str, args: &[Value]) -> Key {
        let mut bytes = Vec::new();
        for a in args {
            bytes.extend(a.to_bytes());
        }
        (target, source.to_string(), bytes)
    }

    /// Parked calls let go in the prelude too, with the fetches they wait on.
    pub(super) fn forget_calls(&mut self, calls: Vec<u64>) {
        if let Some(engine) = self.engine.as_mut() {
            for call in calls {
                let _ = engine.call("__exact_forget", [&call.to_string(), "", ""]);
            }
        }
    }

    pub(super) fn park(&mut self, key: Key, call: u64, ticket: u64, request: &Request) {
        let parked = Parked {
            call,
            ticket,
            work_taken: false,
            staged: None,
        };
        if request.stream {
            self.streams.push((key, parked));
        } else {
            self.parked.push((key, parked));
        }
    }

    pub(super) fn discard_parked(&mut self, token: u64) {
        self.parked.retain(|(_, p)| p.call != token);
        self.held.retain(|held| *held != token);
        self.forget_calls(vec![token]);
    }

    pub(super) fn forget_unheld(&mut self, in_flight: &[InFlight<'_>]) {
        let keep: HashMap<_, _> = in_flight
            .iter()
            .map(|f| (Self::key(Some(f.target), f.source, f.args), f.continuation))
            .collect();
        let gone = |(key, parked): &(Key, Parked)| {
            key.0.is_some()
                && !keep.get(key).is_some_and(|token| match token {
                    Some(token) => *token == parked.call,
                    None => parked.staged.is_none() && parked.ticket != DEFERRED,
                })
        };
        let (gone_calls, kept): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.parked).into_iter().partition(gone);
        self.parked = kept;
        let (ended, open): (Vec<_>, Vec<_>) = std::mem::take(&mut self.streams)
            .into_iter()
            .partition(gone);
        self.streams = open;
        self.forget_calls(
            gone_calls
                .into_iter()
                .chain(ended)
                .map(|(_, p)| p.call)
                .collect(),
        );
    }
}
