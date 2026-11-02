// The probe's Apple implementation: `probeUpload` writes its text to a
// file and starts one upload of it through the host's transfer carrier;
// when the host offers the registration's ending (a wake of kind
// `transfer`), each is raised as `probed(id, status, body, bodyLost)` and,
// once its handler completed, retired.
import ExactPackage
import Foundation

public final class ProbeModule: NSObject, ExactPackage {
    private let context: ExactPackageContext

    public required init(context: ExactPackageContext) {
        self.context = context
        super.init()
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            guard name == "probeUpload" else { return reply(["error": "NotSupportedError"]) }
            let id = args["id"] as? String ?? "", url = args["url"] as? String ?? ""
            let file = FileManager.default.temporaryDirectory.appendingPathComponent("probe-\(UUID().uuidString)")
            try? Data((args["text"] as? String ?? "").utf8).write(to: file)
            let request: [String: Any] = ["url": url, "method": "POST", "bodyFrom": file.path, "headers": ["Content-Type": "text/plain"]]
            let error = context.transfers.start(id: id, requests: [request])
            try? FileManager.default.removeItem(at: file)
            reply(["id": id, "error": error ?? ""])
        }
    }

    public func woke(_ wake: ExactWake) {
        DispatchQueue.main.async { [self] in
            guard wake.kind == "transfer", !wake.keys.isEmpty else { return wake.done(success: true) }
            var left = wake.keys.count
            for id in wake.keys {
                let r = context.transfers.record(id: id)
                let first = (r?["records"] as? [[String: Any]])?.first
                let body = (first?["path"] as? String).flatMap { try? String(contentsOfFile: $0, encoding: .utf8) } ?? ""
                let status = first?["status"] as? Int ?? 0, lost = first?["bodyLost"] as? Bool ?? false
                context.log("probe: \(id) \(r?["result"] as? String ?? "?") \(status), body \(body.utf8.count) bytes\(lost ? ", lost" : "")")
                wake.raise("probed", [id, status, body, lost]) { [self] outcome in
                    if outcome == "completed" { context.transfers.retire(id: id) }
                    left -= 1
                    if left == 0 { wake.done(success: true) }
                }
            }
        }
    }
}
