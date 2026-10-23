#if os(macOS)
import Foundation

/// Account terminal input outlives a Contract answer. Keep its serial RPC queue beside the native
/// view, like drawer writes, rather than retaining a promise owned by a superseded JS mutation.
final class T3TerminalAuth {
    typealias Send = ([String: Any], @escaping (T3Failure?) -> Void) -> Void
    private let send: Send
    private let failure: (Bool) -> Void
    private var instance = "", flow = "", interaction = "", readOnly = true
    private var queue: [[String: Any]] = []
    private var epoch: UInt64 = 0
    private var sending = false, failed = false

    init(send: @escaping Send, failure: @escaping (Bool) -> Void) {
        self.send = send; self.failure = failure
    }

    func configure(instance: String, identity: String, readOnly: Bool) {
        let parts = identity.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        let flow = parts.count == 2 ? String(parts[0]) : ""
        let interaction = parts.count == 2 ? String(parts[1]) : ""
        if self.instance != instance || self.flow != flow || self.interaction != interaction || self.readOnly != readOnly { clear() }
        self.instance = instance; self.flow = flow; self.interaction = interaction; self.readOnly = readOnly
    }

    func clear() { epoch &+= 1; queue = []; sending = false; failed = false }

    func input(_ data: String, size: (cols: Int, rows: Int)? = nil) {
        guard !readOnly, !instance.isEmpty, !flow.isEmpty, !interaction.isEmpty else { return }
        let units = Array(data.utf16)
        var index = 0
        repeat {
            var end = min(index + 4096, units.count)
            // Keep a surrogate pair together while staying below the server's UTF-16 slice limit.
            if end < units.count, end > index, (0xD800...0xDBFF).contains(units[end - 1]), (0xDC00...0xDFFF).contains(units[end]) { end -= 1 }
            var response: [String: Any] = ["type": "terminal", "data": String(decoding: units[index..<end], as: UTF16.self)]
            if let size { response["size"] = ["cols": max(1, min(500, size.cols)), "rows": max(1, min(200, size.rows))] }
            queue.append(response); index = end
        } while index < units.count
        drain()
    }

    private func drain() {
        guard !sending, !queue.isEmpty else { return }
        let response = queue.removeFirst(), current = epoch
        sending = true
        send(["instanceId": instance, "flowId": flow, "interactionId": interaction, "response": response]) { [weak self] error in
            guard let self, current == self.epoch else { return }
            self.sending = false
            if error != nil {
                self.queue = []; self.failed = true; self.failure(true)
            } else {
                if self.failed { self.failed = false; self.failure(false) }
                self.drain()
            }
        }
    }
}
#endif
