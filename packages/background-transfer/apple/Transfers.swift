// @exact/background-transfer on Apple (LLP 1133.017 D3, D4, D10, D17): Background
// Fetch's names over the host's transfer carrier (`context.transfers`,
// ExactKit's Transfers.swift), whose background `URLSession`s finish a
// transfer while the app is suspended or terminated and whose records
// outlive the process.
//
// - A request's `bodyFrom` and `saveTo` are app files (`context.file`); the
//   runner has admitted each against the app's `fs.read` and `fs.write`
//   lines, and its `url` against `net.fetch`. Its headers go to the carrier,
//   never into a record.
// - Every response body is a file. When a registration has ended, each body
//   is moved to its request's `saveTo`, or else to
//   `app:/data/transfers/<id>/<index>`, which `retireBackgroundFetch`
//   deletes with the carrier's record.
// - An ending the host offers (a wake of kind `transfer`) raises
//   `backgroundfetchsuccess`, `backgroundfetchfail` or `backgroundfetchabort`
//   with the registration. The app retires it; until then the host offers it
//   again (D10).
// - Under the agent the carrier runs as in production, against whatever
//   server the drive names: the interface is its own substitute.
import ExactPackage
import Foundation

public final class BackgroundTransferModule: NSObject, ExactPackage {
    /// The extension's name, which the carrier files its registrations under.
    private static let name = "background-transfer"
    private let context: ExactPackageContext
    private var observer: NSObjectProtocol?

    public required init(context: ExactPackageContext) {
        self.context = context
        super.init()
        observer = NotificationCenter.default.addObserver(forName: Notification.Name("exact.transfers.changed"), object: nil, queue: .main) { [weak self] note in
            guard (note.object as? [String: Any])?["package"] as? String == Self.name else { return }
            self?.changed()
        }
    }

    deinit { observer.map(NotificationCenter.default.removeObserver) }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            let id = args["id"] as? String ?? ""
            switch name {
            case "backgroundFetch": reply(start(id, args))
            case "backgroundFetchRegistration": reply(["exact:answer": registration(id) ?? NSNull()])
            case "backgroundFetchIds": reply(["exact:answer": context.transfers.ids])
            case "abortBackgroundFetch": reply(["exact:answer": context.transfers.abort(id: id)])
            case "retireBackgroundFetch":
                context.transfers.retire(id: id)
                if let folder = context.file(Self.kept(id)) { try? FileManager.default.removeItem(at: folder) }
                context.log("background-transfer: \(id) retired")
                changed()
                reply([:])
            case "backgroundFetchStatus": reply(["available": true, "state": "granted"])
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    private func changed() {
        context.changed("\(Self.name).backgroundFetchRegistration")
        context.changed("\(Self.name).backgroundFetchIds")
    }

    /// The registration started, or the error's name.
    private func start(_ id: String, _ args: [String: Any]) -> [String: Any] {
        guard !id.isEmpty, let list = args["requests"] as? [[String: Any]], !list.isEmpty else { return ["error": "TypeError"] }
        var requests: [[String: Any]] = []
        for r in list {
            guard let url = r["url"] as? String else { return ["error": "TypeError"] }
            var out: [String: Any] = ["url": url]
            if let method = r["method"] as? String { out["method"] = method }
            var headers: [String: String] = [:]
            for h in r["headers"] as? [[String: Any]] ?? [] {
                if let n = h["name"] as? String, let v = h["value"] as? String { headers[n] = v }
            }
            out["headers"] = headers
            if let from = r["bodyFrom"] as? String {
                guard let file = context.file(from) else { return ["error": "NotAllowedError"] }
                out["bodyFrom"] = file.path
            }
            if let to = r["saveTo"] as? String {
                guard context.file(to) != nil else { return ["error": "NotAllowedError"] }
                out["saveTo"] = to
            }
            requests.append(out)
        }
        let ceiling = (args["downloadTotal"] as? NSNumber).map { $0.int64Value }
        if let error = context.transfers.start(id: id, requests: requests, downloadTotal: ceiling) { return ["error": error] }
        context.log("background-transfer: \(id) started, \(requests.count) request\(requests.count == 1 ? "" : "s")")
        changed()
        return registration(id) ?? [:]
    }

    /// Where a registration's unsaid response bodies go.
    private static func kept(_ id: String) -> String {
        "app:/data/transfers/" + (id.addingPercentEncoding(withAllowedCharacters: .alphanumerics.union(CharacterSet(charactersIn: "-_"))) ?? id)
    }

    /// The registration in the web's shape; once it has ended, each body
    /// moved to its app file first.
    private func registration(_ id: String) -> [String: Any]? {
        guard let r = context.transfers.record(id: id) else { return nil }
        let ended = !(r["result"] as? String ?? "").isEmpty
        let records = (r["records"] as? [[String: Any]] ?? []).map { x -> [String: Any] in
            var out: [String: Any] = ["index": x["index"] ?? 0, "url": x["url"] ?? "", "status": x["status"] ?? 0,
                                      "contentType": x["contentType"] ?? "", "bodyLost": x["bodyLost"] ?? false]
            if ended, let path = place(id, x) { out["path"] = path }
            return out
        }
        var out: [String: Any] = ["id": id, "result": r["result"] ?? "", "failureReason": r["failureReason"] ?? "", "records": records]
        for k in ["uploadTotal", "uploaded", "downloadTotal", "downloaded"] { out[k] = r[k] ?? 0 }
        if let apple = r["apple"] { out["apple"] = apple }
        return out
    }

    /// Move a record's body from the carrier to its app file: the request's
    /// `saveTo`, else the registration's folder. Nil when it has none.
    private func place(_ id: String, _ x: [String: Any]) -> String? {
        let target = x["saveTo"] as? String ?? "\(Self.kept(id))/\(x["index"] ?? 0)"
        guard let dest = context.file(target) else { return nil }
        let fm = FileManager.default
        if let from = x["path"] as? String, fm.fileExists(atPath: from) {
            try? fm.createDirectory(at: dest.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? fm.removeItem(at: dest)
            do { try fm.moveItem(at: URL(fileURLWithPath: from), to: dest) } catch {
                context.log("background-transfer: \(id): the body was not moved to \(target): \(error.localizedDescription)")
                return nil
            }
        }
        return fm.fileExists(atPath: dest.path) ? target : nil
    }

    /// The host offers registrations that have ended: each raises its event.
    public func woke(_ wake: ExactWake) {
        DispatchQueue.main.async { [self] in
            guard wake.kind == "transfer" else { return wake.done(success: true) }
            let ended = wake.keys.compactMap { registration($0) }
            guard !ended.isEmpty else { return wake.done(success: true) }
            var left = ended.count
            for r in ended {
                let reason = r["failureReason"] as? String ?? ""
                let event = r["result"] as? String == "success" ? "backgroundfetchsuccess" : reason == "aborted" ? "backgroundfetchabort" : "backgroundfetchfail"
                context.log("background-transfer: \(r["id"] ?? ""): \(event)")
                wake.raise(event, [r]) { _ in
                    left -= 1
                    if left == 0 { wake.done(success: true) }
                }
            }
        }
    }
}
