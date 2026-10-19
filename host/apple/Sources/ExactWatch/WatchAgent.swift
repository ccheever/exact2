// @ref LLP 1126 D10 — a development build's agent link on the watch
// simulator (spike): JSON lines over the Unix socket `EXACT_AGENT_SOCKET`
// names, as the iOS simulator's carrier. The runner answers its own ops
// (`tree`, `state`, `logs`, `tags`); `tap` presses the node with a testId.
// A device has no such socket (TN3135).
#if os(watchOS) && EXACT_INSPECT
import Foundation
import ExactCore
import ExactSnapshot

@MainActor enum WatchAgent {
    private static weak var model: LiveModel?
    private static var started = false

    static func start(model: LiveModel) {
        self.model = model
        guard !started, let path = ProcessInfo.processInfo.environment["EXACT_AGENT_SOCKET"], !path.isEmpty else { return }
        started = true
        Thread.detachNewThread { serve(path) }
    }

    private static func answer(_ line: String) -> String {
        guard let model else { return #"{"error":"no live session"}"# }
        let request = (try? JSONSerialization.jsonObject(with: Data(line.utf8))) as? [String: Any] ?? [:]
        if request["op"] as? String == "frames" {
            // What the live driver draws: each node's kind, testId and kernel frame.
            let rows = model.snapshot.nodes.values.sorted { $0.id < $1.id }.map { node -> [String: Any] in
                let f = node.frame
                return ["id": node.id, "kind": node.kind, "props": node.props, "handlers": Array(node.handlers), "children": node.children,
                        "frame": [f.minX, f.minY, f.width, f.height], "content": node.scrollContent.map { [$0.width, $0.height] } ?? []]
            }
            let data = (try? JSONSerialization.data(withJSONObject: ["roots": model.snapshot.roots, "nodes": rows])) ?? Data()
            return String(decoding: data, as: UTF8.self)
        }
        let op = request["op"] as? String ?? ""
        guard op == "tap" || op == "input" else { return model.session.agent(line) }
        let id = request["id"] as? String ?? ""
        guard let node = model.snapshot.nodes.values.first(where: { $0.props["testId"] == id }) else {
            return #"{"error":"no node with testId \#(id)"}"#
        }
        // `input` sends what the field's submit sends: its whole text.
        if op == "input" { model.edit(node.id, text: request["text"] as? String ?? "") } else { model.press(node.id) }
        return #"{"ok":true}"#
    }

    nonisolated private static func serve(_ path: String) {
        unlink(path)
        let listener = socket(AF_UNIX, SOCK_STREAM, 0)
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8.prefix(MemoryLayout.size(ofValue: address.sun_path) - 1))
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: bytes) }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard listener >= 0, bound == 0, listen(listener, 4) == 0 else { return }
        while true {
            let client = accept(listener, nil, nil)
            guard client >= 0 else { continue }
            converse(client)
            close(client)
        }
    }

    nonisolated private static func converse(_ client: Int32) {
        var pending = Data(), chunk = [UInt8](repeating: 0, count: 4096)
        while true {
            let count = read(client, &chunk, chunk.count)
            guard count > 0 else { return }
            pending.append(contentsOf: chunk[0..<count])
            while let newline = pending.firstIndex(of: 10) {
                let line = String(decoding: pending[pending.startIndex..<newline], as: UTF8.self)
                pending.removeSubrange(pending.startIndex...newline)
                let reply = DispatchQueue.main.sync { MainActor.assumeIsolated { answer(line) } } + "\n"
                _ = reply.utf8CString.withUnsafeBufferPointer { write(client, $0.baseAddress, $0.count - 1) }
            }
        }
    }
}
#endif
