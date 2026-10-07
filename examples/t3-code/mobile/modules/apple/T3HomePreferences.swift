// @ref llp/1106.004-home-projection.decision.md#decision
#if os(iOS)
// Upstream 365aa87982 use-thread-list-v2-shelf-preferences; atomic mobile preference file.
// @ref llp/1106.000-mobile-app-layout.decision.md#shared-typescript
import Foundation

final class T3HomePreferences {
    private let path: URL
    private var values: [String: Bool]?
    init(directory: URL) { path = directory.appendingPathComponent("mobile-home.json") }

    func perform(_ request: [String: Any], reply: ExactReply) {
        do {
            if values == nil {
                if FileManager.default.fileExists(atPath: path.path) {
                    values = try JSONDecoder().decode([String: Bool].self, from: Data(contentsOf: path))
                } else { values = [:] }
            }
            if request["op"] as? String == "mobileToggleShelf" {
                guard let section = request["section"] as? String,
                      ["working", "snoozed", "settled"].contains(section) else {
                    throw NSError(domain: "T3Mobile", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unknown thread shelf."])
                }
                let key = section + "Expanded"
                var next = values ?? [:]
                next[key] = !(next[key] ?? false)
                try FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
                try JSONEncoder().encode(next).write(to: path, options: .atomic)
                values = next
            }
            reply.send(["ok": true, "generation": 0, "value": values ?? [:]])
        } catch {
            reply.send(["ok": false, "generation": 0,
                        "error": ["kind": "Persistence", "message": error.localizedDescription, "uncertain": false]])
        }
    }
}
#endif
