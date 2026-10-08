#if os(iOS)
// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
// T3 Code 365aa87982 cliRelease.ts: public GitHub release pages, 100 entries.
import Foundation

final class T3ReleasePages {
    private struct Pending {
        let task: URLSessionDataTask
        let reply: ExactReply
        let generation: Int
    }
    private let lock = NSLock()
    private var pending: [UUID: Pending] = [:]
    private var alive = true
    private let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.httpCookieStorage = nil
        configuration.urlCredentialStorage = nil
        configuration.timeoutIntervalForRequest = 20
        configuration.timeoutIntervalForResource = 20
        return URLSession(configuration: configuration)
    }()

    func perform(_ request: [String: Any], reply: ExactReply) {
        let generation = request["generation"] as? Int ?? 0
        guard let page = request["page"] as? Int, (1...100).contains(page) else {
            reply.send(Self.failure("The release page must be between 1 and 100.", generation)); return
        }
        // Exact's data source has no ambient wall clock. Keep the entire paginated
        // lookup's deadline here and pass its opaque value back through TypeScript.
        let now = Date().timeIntervalSince1970 * 1000
        let deadline = page == 1 ? now + 20_000 : request["deadlineMs"] as? Double ?? 0
        let milliseconds = min(20_000, deadline - now)
        guard deadline.isFinite, milliseconds.isFinite, milliseconds > 0 else {
            reply.send(Self.failure("The release check timed out. Try again.", generation)); return
        }
        let url = URL(string: "https://api.github.com/repos/pingdotgg/t3code/releases?per_page=100&page=\(page)")!
        var query = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: milliseconds / 1000)
        query.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        query.setValue("T3CodeMobile", forHTTPHeaderField: "User-Agent")
        let id = UUID()
        let task = session.dataTask(with: query) { [weak self] data, response, error in
            guard let self else { return }
            self.lock.lock(); let entry = self.pending.removeValue(forKey: id); self.lock.unlock()
            guard let entry else { return }
            if let error {
                entry.reply.send(Self.failure(error.localizedDescription, entry.generation)); return
            }
            guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
                let status = (response as? HTTPURLResponse)?.statusCode ?? 0
                entry.reply.send(Self.failure("Could not check releases (\(status)). Try again.", entry.generation)); return
            }
            guard let data, let releases = (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]] else {
                entry.reply.send(Self.failure("The release response is invalid.", entry.generation)); return
            }
            // Only these release-index fields are consumed; avoid transferring asset descriptions.
            guard releases.allSatisfy({ $0["tag_name"] is String && ($0["draft"] == nil || $0["draft"] is Bool) }) else {
                entry.reply.send(Self.failure("The release response is invalid.", entry.generation)); return
            }
            let index = releases.map { release -> [String: Any] in
                var item: [String: Any] = ["tag_name": release["tag_name"]!]
                if let draft = release["draft"] { item["draft"] = draft }
                return item
            }
            entry.reply.send(["ok": true, "generation": entry.generation, "value": ["releases": index, "deadlineMs": deadline]])
        }
        lock.lock()
        guard alive else {
            lock.unlock(); task.cancel()
            reply.send(Self.failure("The mobile session was closed.", generation)); return
        }
        pending[id] = Pending(task: task, reply: reply, generation: generation)
        lock.unlock()
        task.resume()
    }

    func destroy() {
        lock.lock(); alive = false; let entries = Array(pending.values); pending.removeAll(); lock.unlock()
        for entry in entries {
            entry.task.cancel()
            entry.reply.send(Self.failure("The mobile session was closed.", entry.generation))
        }
        session.invalidateAndCancel()
    }

    private static func failure(_ message: String, _ generation: Int) -> [String: Any] {
        ["ok": false, "generation": generation, "error": ["kind": "release", "message": message]]
    }
}
#endif
