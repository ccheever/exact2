//! A mutation's, a mapper call's or an answer's socket operations (LLP
//! 1016.000 D7). The prelude's handle checks and copies each `send` and
//! `close` while the mutation's source, the socket's own mapper or a turn of
//! an answer runs, and hands them through the door: a mutation's when its
//! answer is given now and a mapper call's as it returns (op 14), an
//! answer's as each is made (op 17). Each is addressed by the prelude's ticket
//! for the stream's `fetch`; here it becomes the resource that owns the
//! stream, staged on the runner's store for the runner to address.
use super::{Key, Module, Parked};
use exact_runner::{SocketOp, Store, Target};
use serde_json::Value as Json;

/// One operation as the prelude records it: `{"op":"text","data":…}`,
/// `{"op":"binary","data":base64}` or `{"op":"close","code":…,"reason":…}`;
/// a listener's `{"op":"line","connection":…,"data":…}` or
/// `{"op":"end","connection":…}`; a child's `{"op":"write","data":base64,"incarnation":n,"drain":true}`, `end` or
/// `{"op":"kill","signal":…}`.
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
        // A child's (LLP 1133.007 D2; js/src/process.js), by incarnation.
        Some(op @ ("write" | "end" | "kill")) if j.get("incarnation").is_some() => {
            let incarnation = (j.get("incarnation").and_then(Json::as_u64))
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("child: an operation with no incarnation")?;
            let op = match op {
                "write" => exact_runner::ProcessOp::Write {
                    data: exact_runner::agent::unbase64(data())
                        .ok_or("child: bytes that are not base64")?,
                    drain: j.get("drain").and_then(Json::as_bool).unwrap_or(false),
                },
                "end" => exact_runner::ProcessOp::End,
                _ => exact_runner::ProcessOp::Kill(
                    j.get("signal")
                        .and_then(Json::as_str)
                        .unwrap_or("SIGTERM")
                        .to_string(),
                ),
            };
            Ok(SocketOp::Process { incarnation, op })
        }
        // A listener's, to one connection (LLP 1133.005 D6).
        Some(op @ ("line" | "end")) => {
            let connection = (j.get("connection").and_then(Json::as_u64))
                .ok_or("listen: a connection that is not a number")?;
            Ok(match op {
                "line" => SocketOp::Line {
                    connection,
                    text: data().to_string(),
                },
                _ => SocketOp::End { connection },
            })
        }
        _ => Err(
            "socket: an operation that is not text, binary, close, line, end, write or kill".into(),
        ),
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
        for (resource, op) in self.owners(ops)? {
            store.stage_socket(resource, op);
        }
        Ok(())
    }

    /// Each operation by the resource that owns its socket's stream, or
    /// refused whole.
    fn owners(&self, ops: Vec<(u64, SocketOp)>) -> Result<Vec<(usize, SocketOp)>, String> {
        let mut owned = Vec::with_capacity(ops.len());
        for (ticket, op) in ops {
            let owner = self.streams.iter().find_map(|(key, parked)| match key.0 {
                Some(Target::Resource(i)) if parked.ticket == ticket => Some(i),
                _ => None,
            });
            owned.push((
                owner.ok_or("InvalidStateError: the socket is not open")?,
                op,
            ));
        }
        Ok(owned)
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
        match self.owners(ops) {
            Ok(owned) => owned
                .into_iter()
                .for_each(|(resource, op)| store.stage_answer_socket(resource, op)),
            Err(why) => (self.logs).push(format!(
                "answer `{source}` failed: {why}; socket frames were refused"
            )),
        }
    }
}
