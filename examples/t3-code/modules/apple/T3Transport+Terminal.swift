import Foundation

/// One `terminal.attach` stream on the transport (task terminal-drawer). Its chunks go to the
/// native session (T3TerminalSessions.swift), never to the app's event inbox, and each chunk is
/// acknowledged only after the session's buffer holds it: the server sends at most 8 chunks or 64 KiB
/// ahead of the acknowledgements (T3 Code 1e2ecbd975 apps/server/src/terminal/OutputProtocol.ts), so a
/// busy main thread slows the shell instead of growing a queue.
final class T3TerminalStream {
    let id: String
    let epoch: Int
    /// On the transport queue: the chunk's values and the acknowledgement to call once they are held.
    let receive: ([Any], @escaping () -> Void) -> Void
    /// On the transport queue: the stream ended (nil: a normal end) or the socket went away (`lost`).
    let ended: (_ failure: T3Failure?, _ lost: Bool) -> Void
    init(id: String, epoch: Int, receive: @escaping ([Any], @escaping () -> Void) -> Void, ended: @escaping (T3Failure?, Bool) -> Void) {
        self.id = id; self.epoch = epoch; self.receive = receive; self.ended = ended
    }
}

extension T3Transport {
    /// The reference's worst case: 11 mounted threads × 4 terminals. Counted apart from the 16 app streams.
    static let maximumTerminalStreams = 44

    /// Opens an attach stream (`terminal.attach`, which also starts a shell when none runs). `opened`
    /// answers on the transport queue with the stream id, or the failure that kept it from opening.
    func terminalAttach(payload: [String: Any], receive: @escaping ([Any], @escaping () -> Void) -> Void,
                        ended: @escaping (T3Failure?, Bool) -> Void, opened: @escaping (String?, T3Failure?) -> Void) {
        queue.async { [self] in
            guard alive, state == "connected" else { return opened(nil, T3Failure(kind: "Disconnected", message: "The server is not connected.")) }
            guard terminalStreams.count < Self.maximumTerminalStreams else {
                return opened(nil, T3Failure(kind: "Busy", message: "Too many terminals are attached."))
            }
            let id = nextID(), epoch = generation
            do {
                let text = try T3Wire.encode(T3Wire.request(id: id, method: "terminal.attach", payload: payload))
                terminalStreams[id] = T3TerminalStream(id: id, epoch: epoch, receive: receive, ended: ended)
                send(text, epoch: epoch)
                opened(id, nil)
            } catch { opened(nil, failure(error)) }
        }
    }

    /// Stops an attach stream (an Interrupt, as unsubscribe sends). A late chunk is still acknowledged.
    func terminalDetach(_ id: String) {
        queue.async { [self] in
            guard let stream = terminalStreams.removeValue(forKey: id) else { return }
            send(["_tag": "Interrupt", "requestId": id], epoch: stream.epoch)
        }
    }

    /// A one-shot terminal RPC (`terminal.write`, `terminal.resize`): `done` gets nil or the failure, on the transport queue.
    func terminalCall(_ method: String, payload: [String: Any], done: @escaping (T3Failure?) -> Void) {
        queue.async { [self] in
            guard alive else { return done(T3Failure(kind: "Closed", message: "The window was closed.")) }
            do {
                try rpc(["method": method, "payload": payload]) { response in
                    guard response["ok"] as? Bool != true else { return done(nil) }
                    let error = response["error"] as? [String: Any] ?? [:]
                    done(T3Failure(kind: error["kind"] as? String ?? "Network", message: error["message"] as? String ?? "The terminal request failed."))
                }
            } catch { done(failure(error)) }
        }
    }

    /// From `consume`: a chunk of a terminal stream. True when the stream is a terminal's.
    func terminalChunk(id: String, requestId: Any, values: [Any], epoch: Int) -> Bool {
        guard let stream = terminalStreams[id] else { return false }
        stream.receive(values) { [weak self] in
            self?.queue.async { [weak self] in
                guard let self, self.alive, self.generation == epoch else { return }
                self.send(["_tag": "Ack", "requestId": requestId], epoch: epoch)
            }
        }
        return true
    }

    /// From `consume`: a terminal stream's Exit. True when the stream was a terminal's.
    func terminalEnded(id: String, failure: T3Failure?) -> Bool {
        guard let stream = terminalStreams.removeValue(forKey: id) else { return false }
        stream.ended(failure, false)
        return true
    }

    /// From `retire`: the socket is gone, and every attach stream with it.
    func terminalRetired() {
        let gone = Array(terminalStreams.values); terminalStreams.removeAll()
        for stream in gone { stream.ended(nil, true) }
        terminalConnection?(false)
    }

    var terminalStreamCount: Int { terminalStreams.count }
}
