//! A mutation's, a mapper call's or an answer's socket operations (LLP
//! 1016.000 D7). The prelude's handle checks and copies each `send` and
//! `close` while the mutation's source, the socket's own mapper or a turn of
//! a resource's answer runs, and hands them through the door (op 14): a
//! mutation's when its answer is given now, a mapper call's as it returns,
//! an answer's as each is made. Each is addressed by the prelude's ticket
//! for the stream's `fetch`; here it becomes the resource that owns the
//! stream, staged on the runner's store for the runner to address.
use super::{Key, Module, Parked};
use exact_runner::{SocketOp, Store, Target};
use serde_json::Value as Json;

/// One operation as the prelude records it: `{"op":"text","data":…}`,
/// `{"op":"binary","data":base64}` or `{"op":"close","code":…,"reason":…}`.
pub(crate) fn op_from_json(text: &str) -> Result<SocketOp, String> {
    let j: Json = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let data = || j.get("data").and_then(Json::as_str).unwrap_or("");
    match j.get("op").and_then(Json::as_str) {
        Some("text") => Ok(SocketOp::Text(data().to_string())),
        Some("binary") => exact_runner::agent::unbase64(data())
            .map(SocketOp::Binary)
            .ok_or_else(|| "socket: binary data that is not base64".into()),
        Some("close") => Ok(SocketOp::Close {
            code: j
                .get("code")
                .and_then(Json::as_u64)
                .and_then(|c| u16::try_from(c).ok())
                .ok_or("socket: a close code that is not a number")?,
            reason: j
                .get("reason")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
        }),
        _ => Err("socket: an operation that is not text, binary or close".into()),
    }
}

/// The turn's socket snapshot, from the runner's by resource to the
/// prelude's by stream ticket: what each handle's `bufferedAmount` starts
/// from (op 15).
pub(crate) fn snapshot(streams: &[(Key, Parked)], store: &Store) -> Vec<(u64, u64)> {
    (store.socket_buffered().iter())
        .filter_map(|&(resource, buffered)| {
            let ticket = streams.iter().find_map(|(key, parked)| {
                (key.0 == Some(Target::Resource(resource))).then_some(parked.ticket)
            })?;
            Some((ticket, buffered))
        })
        .collect()
}

impl Module {
    /// Stage the operations a mutation's answer or a mapper call made on
    /// `store`, by the resource that owns each socket. A ticket that names
    /// no open stream of a resource is a handle the module kept past its
    /// socket's life.
    pub(crate) fn stage_sockets(
        &self,
        store: &mut Store,
        ops: Vec<(u64, SocketOp)>,
    ) -> Result<(), String> {
        let mut staged = Vec::with_capacity(ops.len());
        for (ticket, op) in ops {
            let owner = self.streams.iter().find_map(|(key, parked)| match key.0 {
                Some(Target::Resource(i)) if parked.ticket == ticket => Some(i),
                _ => None,
            });
            staged.push((
                owner.ok_or("InvalidStateError: the socket is not open")?,
                op,
            ));
        }
        for (resource, op) in staged {
            store.stage_socket(resource, op);
        }
        Ok(())
    }

    /// Stage what a turn of `source`'s answer sent, whatever it answered
    /// (the amendment of 2026-10-18). The prelude refuses a handle whose
    /// stream it saw end, so a ticket this module no longer streams on is a
    /// turn that outlived its socket: the whole batch is refused, and said.
    pub(crate) fn stage_answer(
        &mut self,
        store: &mut Store,
        source: &str,
        ops: Vec<(u64, SocketOp)>,
    ) {
        if let Err(why) = self.stage_sockets(store, ops) {
            (self.logs).push(format!(
                "answer `{source}` failed: {why}; socket frames were refused"
            ));
        }
    }
}
