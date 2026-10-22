// @fixture/tray on Apple: a module outside exact2's tree, composed into the
// app's module as any package's implementation is. A put keeps what each
// item carried as the seam handed it over (a string, bytes as base64, the
// file the host resolved an admitted path to) and reports it as `$state`.
// It is its own substitute, so under the agent it runs too, saying so.
import Foundation

public final class TrayModule {
    private let agent: Bool
    private var newest = 0

    public init(changed: @escaping (String) -> Void, agent: Bool, log: @escaping (String) -> Void) {
        self.agent = agent
    }

    public func later(_ name: String, _ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let seq = (args["$sequence"] as? [String: Any])?["seq"] as? Int ?? 0
        let items: [[String: Any]]
        switch name {
        case "put": items = args["items"] as? [[String: Any]] ?? []
        case "putText": items = [["text/plain": args["text"] ?? ""]]
        default: return reply(["error": "NotSupportedError"])
        }
        if seq == 1 { newest = 0 }
        guard seq > newest else { return reply(["error": "AbortError", "message": "put \(seq): a later put superseded it"]) }
        newest = seq
        let last = items.map { item in item.mapValues(describe) }
        reply(["$state": ["last": last, "substituting": agent]])
    }

    private func describe(_ value: Any) -> String {
        if let text = value as? String { return "text \(text)" }
        if let object = value as? [String: Any] {
            if let b64 = object["$bytes"] as? String { return "bytes \(Data(base64Encoded: b64)?.count ?? -1)" }
            if let file = object["$file"] as? String {
                let size = (try? FileManager.default.attributesOfItem(atPath: file)[.size] as? Int) ?? -1
                return "file \(size)"
            }
        }
        return "other"
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] { ["error": "NotSupportedError"] }
    public func release(_ name: String) {}
}
