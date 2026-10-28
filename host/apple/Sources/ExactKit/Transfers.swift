// The transfer carrier (LLP 1133.017 D8, D10): two background
// `URLSession`s, `<bundle id>.transfers` and
// `<bundle id>.transfers.discretionary`, their delegate, and a durable
// record per registration. A session the system relaunches the app for must
// be made again with the same identifier before its events arrive, which is
// before a package's Swift loads, so the carrier is the host's, and a
// package reaches it through `context.transfers`.
//
// The module side reaches it through `NotificationCenter`
// (`exact.transfers`, an `NSMutableDictionary` whose `reply` the carrier
// sets): `{op: start, package, id, requests, discretionary, downloadTotal}`,
// `{op: abort, package, id}`, `{op: retire, package, id}`, `{op: record,
// package, id}`, `{op: ids, package}`. It posts `exact.transfers.changed`
// `{package, id}` as a registration's bytes move, at most once a second,
// and when it ends.
//
// The record is the truth and the event a notice of it (D10): a
// registration whose requests have all ended is offered to its package as
// a wake of kind `transfer`, its key the registration's id, at once while
// the app runs; after a relaunch for the session's events, at
// `urlSessionDidFinishEvents`, whose completion handler is called when the
// wake is done; and again at each later launch, once a module is ready,
// until the package retires it.
import Foundation

public enum TransferCarrier {
    struct Record: Codable {
        var id: String, package: String, discretionary: Bool
        var requests: [Item]
        var result = "", failureReason = ""
        var cancelledReason: Int?
        /// The wakes this registration's ending was offered in.
        var delivered = 0
        /// The web's ceiling on the bytes downloaded, past which it fails.
        var downloadTotal: Int64?
        var ended: Bool { requests.allSatisfy(\.ended) }
    }

    struct Item: Codable {
        var index: Int, method: String
        /// Without its query or fragment, which may carry a credential.
        var url: String
        var upload: Bool
        var ended = false
        var status: Int?, contentType: String?, location: String?, error: String?
        var length: Int64?
        var bodyLost = false
        /// The app file the response body goes to (`saveTo`), which the
        /// package moves it to; the carrier keeps it as given.
        var saveTo: String?
        /// The request body's size, and the bytes sent and received so far;
        /// optional, so a record an earlier build wrote still reads.
        var size: Int64?, sent: Int64?, received: Int64?
    }

    private static var records: [String: Record] = [:]
    private static var sessions: [Bool: URLSession] = [:]
    /// The completion handler of a relaunch for each session's events.
    private static var relaunched: [String: () -> Void] = [:]
    private static let delegate = Delegate()
    private static var installed = false
    /// Registrations in a wake now, not offered again until it is done.
    private static var offering: Set<String> = []
    /// When each registration's progress was last told.
    private static var told: [String: Date] = [:]

    private static var bundle: String { Bundle.main.bundleIdentifier ?? "app" }
    private static func identifier(_ discretionary: Bool) -> String { bundle + (discretionary ? ".transfers.discretionary" : ".transfers") }

    /// Under the app's support directory, not purgeable; on the Mac under its bundle id.
    static let directory: URL = {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        #if os(macOS)
        let dir = support.appendingPathComponent(Bundle.main.bundleIdentifier ?? "app").appendingPathComponent("transfers")
        #else
        let dir = support.appendingPathComponent("exact-transfers")
        #endif
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()

    private static func folder(_ id: String) -> URL { directory.appendingPathComponent(safe(id)) }
    private static func body(_ id: String, _ index: Int) -> URL { folder(id).appendingPathComponent("\(index)") }
    private static func safe(_ id: String) -> String {
        id.addingPercentEncoding(withAllowedCharacters: .alphanumerics.union(CharacterSet(charactersIn: "-_"))) ?? id
    }

    /// At launch: the records read back, and the sessions made again if a
    /// transfer is in flight, so its events are heard.
    static func install() {
        guard !installed else { return }
        installed = true
        for name in (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? [] {
            let file = directory.appendingPathComponent(name).appendingPathComponent("record.json")
            if let data = try? Data(contentsOf: file), let r = try? JSONDecoder().decode(Record.self, from: data) { records[r.id] = r }
        }
        NotificationCenter.default.addObserver(forName: Notification.Name("exact.transfers"), object: nil, queue: nil) { note in
            guard let box = note.object as? NSMutableDictionary else { return }
            let run = { box["reply"] = handle(box as? [String: Any] ?? [:]) }
            if Thread.isMainThread { run() } else { DispatchQueue.main.sync(execute: run) }
        }
        if records.values.contains(where: { !$0.ended }) {
            DispatchQueue.main.async { for d in Set(records.values.filter { !$0.ended }.map(\.discretionary)) { _ = session(d) } }
        }
    }

    private static func session(_ discretionary: Bool) -> URLSession {
        if let s = sessions[discretionary] { return s }
        let c = URLSessionConfiguration.background(withIdentifier: identifier(discretionary))
        c.isDiscretionary = discretionary
        #if os(iOS)
        c.sessionSendsLaunchEvents = true
        #endif
        let s = URLSession(configuration: c, delegate: delegate, delegateQueue: .main)
        sessions[discretionary] = s
        return s
    }

    /// iOS relaunched the app for `identifier`'s events: the session made
    /// again before this returns, `completion` called when the wake its
    /// endings raise is done.
    static func events(_ identifier: String, _ completion: @escaping () -> Void) {
        install()
        guard identifier == self.identifier(false) || identifier == self.identifier(true) else { return completion() }
        relaunched[identifier] = completion
        _ = session(identifier == self.identifier(true))
        WakeStore.log("transfers: relaunched for \(identifier)")
    }

    private static func keep(_ r: Record) {
        records[r.id] = r
        try? FileManager.default.createDirectory(at: folder(r.id), withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(r) { try? data.write(to: folder(r.id).appendingPathComponent("record.json"), options: .atomic) }
    }

    private static func handle(_ box: [String: Any]) -> Any? {
        guard let package = box["package"] as? String else { return nil }
        let id = box["id"] as? String ?? ""
        switch box["op"] as? String {
        case "start":
            let total = (box["downloadTotal"] as? NSNumber)?.int64Value
            return start(package, id, box["requests"] as? [[String: Any]] ?? [], discretionary: box["discretionary"] as? Bool ?? false, downloadTotal: total)
        case "abort":
            guard let r = records[id], r.package == package, !r.ended else { return false }
            session(r.discretionary).getAllTasks { tasks in
                for t in tasks where t.taskDescription?.hasPrefix(id + " ") == true { t.cancel() }
            }
            return true
        case "retire":
            guard records[id]?.package == package else { return nil }
            records[id] = nil
            try? FileManager.default.removeItem(at: folder(id))
            return nil
        case "record": return records[id].flatMap { $0.package == package ? shown($0) : nil }
        case "ids": return records.values.filter { $0.package == package }.map(\.id).sorted()
        default: return nil
        }
    }

    /// Each request `{url, method, headers, bodyFrom, saveTo}`; `bodyFrom` a
    /// file, copied into the record's folder first, since a background
    /// session uploads only from a file. Nil, or the error's name.
    private static func start(_ package: String, _ id: String, _ requests: [[String: Any]], discretionary: Bool, downloadTotal: Int64? = nil) -> String? {
        guard !id.isEmpty, !requests.isEmpty, records[id].map({ $0.ended }) ?? true else { return "TypeError" }
        try? FileManager.default.removeItem(at: folder(id))
        try? FileManager.default.createDirectory(at: folder(id), withIntermediateDirectories: true)
        var items: [Item] = [], tasks: [URLSessionTask] = []
        let s = session(discretionary)
        for (index, r) in requests.enumerated() {
            guard let text = r["url"] as? String, let url = URL(string: text), ["http", "https"].contains(url.scheme ?? "") else { return "TypeError" }
            var request = URLRequest(url: url)
            request.httpMethod = r["method"] as? String ?? (r["bodyFrom"] == nil ? "GET" : "POST")
            for (k, v) in r["headers"] as? [String: String] ?? [:] { request.setValue(v, forHTTPHeaderField: k) }
            var shownURL = URLComponents(url: url, resolvingAgainstBaseURL: false)
            shownURL?.query = nil
            shownURL?.fragment = nil
            let task: URLSessionTask
            var size: Int64?
            if let from = r["bodyFrom"] as? String {
                let copy = folder(id).appendingPathComponent("upload-\(index)")
                do { try FileManager.default.copyItem(at: URL(fileURLWithPath: from), to: copy) } catch { return "NotAllowedError" }
                size = try? FileManager.default.attributesOfItem(atPath: copy.path)[.size] as? Int64
                task = s.uploadTask(with: request, fromFile: copy)
            } else {
                task = s.downloadTask(with: request)
            }
            task.taskDescription = "\(id) \(index)"
            var item = Item(index: index, method: request.httpMethod ?? "GET", url: shownURL?.string ?? text, upload: r["bodyFrom"] != nil)
            item.saveTo = r["saveTo"] as? String
            item.size = size
            items.append(item)
            tasks.append(task)
        }
        var record = Record(id: id, package: package, discretionary: discretionary, requests: items)
        record.downloadTotal = downloadTotal
        keep(record)
        tasks.forEach { $0.resume() }
        return nil
    }

    /// What a package reads of a record: never a header or a query.
    private static func shown(_ r: Record) -> [String: Any] {
        var out: [String: Any] = ["id": r.id, "result": r.result, "failureReason": r.failureReason,
                                  "uploadTotal": r.requests.reduce(0) { $0 + ($1.size ?? 0) }, "uploaded": r.requests.reduce(0) { $0 + ($1.sent ?? 0) },
                                  "downloadTotal": r.downloadTotal ?? 0, "downloaded": r.requests.reduce(0) { $0 + ($1.received ?? 0) }]
        if let c = r.cancelledReason { out["apple"] = ["cancelledReason": c] }
        out["records"] = r.requests.map { i -> [String: Any] in
            let file = body(r.id, i.index)
            var o: [String: Any] = ["index": i.index, "url": i.url, "status": i.status ?? 0, "contentType": i.contentType ?? "", "bodyLost": i.bodyLost]
            if FileManager.default.fileExists(atPath: file.path) { o["path"] = file.path }
            if let l = i.location { o["location"] = l }
            if let t = i.saveTo { o["saveTo"] = t }
            return o
        }
        return out
    }

    // MARK: endings

    fileprivate static func received(_ task: URLSessionTask, _ data: Data) {
        guard let (id, index) = which(task), records[id] != nil else { return }
        let file = body(id, index)
        if let h = try? FileHandle(forWritingTo: file) { h.seekToEndOfFile(); h.write(data); h.closeFile() } else { try? data.write(to: file) }
        progressed(task) { $0.received = ($0.received ?? 0) + Int64(data.count) }
    }

    /// A request's bytes moved: kept in memory, written with the ending, and
    /// told at most once a second. A download past `downloadTotal` is
    /// cancelled and fails `download-total-exceeded`.
    fileprivate static func progressed(_ task: URLSessionTask, _ change: (inout Item) -> Void) {
        guard let (id, index) = which(task), var r = records[id], index < r.requests.count else { return }
        change(&r.requests[index])
        if let ceiling = r.downloadTotal, ceiling > 0, r.requests.reduce(0, { $0 + ($1.received ?? 0) }) > ceiling, r.failureReason.isEmpty {
            r.failureReason = "download-total-exceeded"
            task.cancel()
        }
        records[id] = r
        if told[id].map({ Date().timeIntervalSince($0) >= 1 }) ?? true {
            told[id] = Date()
            tell(r)
        }
    }

    private static func tell(_ r: Record) {
        let body: [String: Any] = ["package": r.package, "id": r.id]
        NotificationCenter.default.post(name: Notification.Name("exact.transfers.changed"), object: body as NSDictionary)
    }

    fileprivate static func downloaded(_ task: URLSessionTask, to location: URL) {
        guard let (id, index) = which(task), records[id] != nil else { return }
        let file = body(id, index)
        try? FileManager.default.removeItem(at: file)
        try? FileManager.default.moveItem(at: location, to: file)
    }

    fileprivate static func completed(_ task: URLSessionTask, _ error: Error?) {
        guard let (id, index) = which(task), var r = records[id], index < r.requests.count else { return }
        var i = r.requests[index]
        let http = task.response as? HTTPURLResponse
        i.ended = true
        i.status = http?.statusCode
        i.contentType = http?.value(forHTTPHeaderField: "Content-Type")
        i.location = http?.value(forHTTPHeaderField: "Location")
        i.length = http.map { $0.expectedContentLength }
        // A small body's progress may never be told; the task's counts are final here.
        i.sent = max(i.sent ?? 0, task.countOfBytesSent)
        i.received = max(i.received ?? 0, task.countOfBytesReceived)
        let have = (try? FileManager.default.attributesOfItem(atPath: body(id, index).path)[.size] as? Int64) ?? 0
        i.bodyLost = error == nil && (i.length ?? 0) > have
        if let e = error as NSError? {
            i.error = e.localizedDescription
            if e.code == NSURLErrorCancelled && r.failureReason == "download-total-exceeded" {
                // Cancelled by the carrier at the ceiling, not by the app.
            } else if e.code == NSURLErrorCancelled {
                r.failureReason = "aborted"
                r.cancelledReason = e.userInfo[NSURLErrorBackgroundTaskCancelledReasonKey] as? Int
            } else if r.failureReason.isEmpty {
                r.failureReason = "fetch-error"
            }
        } else if let s = i.status, !(200..<300).contains(s), r.failureReason.isEmpty {
            r.failureReason = "bad-status"
        }
        r.requests[index] = i
        if r.ended { r.result = r.failureReason.isEmpty ? "success" : "failure" }
        keep(r)
        tell(r)
        WakeStore.log("transfers: \(id) \(index) ended \(i.status.map(String.init) ?? i.error ?? "?")\(i.bodyLost ? ", its body lost" : "")")
        // While the app runs and no relaunch waits on these events, the ending is offered now.
        if r.ended, relaunched[identifier(r.discretionary)] == nil { offer([id]) }
    }

    fileprivate static func finished(_ session: URLSession) {
        guard let identifier = session.configuration.identifier, let completion = relaunched.removeValue(forKey: identifier) else { return }
        let discretionary = identifier == self.identifier(true)
        let ended = records.values.filter { $0.discretionary == discretionary && $0.ended && $0.delivered < 3 }.map(\.id)
        WakeStore.log("transfers: \(identifier)'s events are in: \(ended.count) ended")
        offer(ended, completion: completion)
    }

    /// The ended registrations not yet retired, once a module is ready at a launch (D10).
    static func ready() {
        let held = records.values.filter { $0.ended && !offering.contains($0.id) }.map(\.id)
        if !held.isEmpty { offer(held) }
    }

    private static func offer(_ ids: [String], completion: (() -> Void)? = nil) {
        guard !ids.isEmpty else { completion?(); return }
        offering.formUnion(ids)
        for id in ids { records[id]?.delivered += 1; records[id].map(keep) }
        WakeStore.transfer(ids.compactMap { id in records[id].map { ($0.package, id) } }) { _ in
            offering.subtract(ids)
            completion?()
        }
    }

    private static func which(_ task: URLSessionTask) -> (String, Int)? {
        guard let d = task.taskDescription, let space = d.lastIndex(of: " "), let index = Int(d[d.index(after: space)...]) else { return nil }
        return (String(d[..<space]), index)
    }

    private final class Delegate: NSObject, URLSessionDataDelegate, URLSessionDownloadDelegate {
        func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) { received(dataTask, data) }
        func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
            downloaded(downloadTask, to: location)
        }
        func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) { completed(task, error) }
        func urlSession(_ session: URLSession, task: URLSessionTask, didSendBodyData bytesSent: Int64, totalBytesSent: Int64, totalBytesExpectedToSend: Int64) {
            progressed(task) { $0.sent = totalBytesSent }
        }
        func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
            progressed(downloadTask) { $0.received = totalBytesWritten }
        }
        func urlSessionDidFinishEvents(forBackgroundURLSession session: URLSession) { finished(session) }
    }
}

extension ExactApp {
    /// `application(_:handleEventsForBackgroundURLSession:completionHandler:)`:
    /// the carrier's session made again before it returns (LLP 1133.017 D10).
    public func handleTransferEvents(_ identifier: String, completion: @escaping () -> Void) {
        TransferCarrier.events(identifier, completion)
    }
}
