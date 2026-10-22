// An `app:/` path in a module call (LLP 1133.006 D8 item 3): the runner has
// admitted it under the calling source's `fs.read` grant; the host maps it
// to the file in the app's roots, as storage configured them (also under
// the agent, where data code's files are), and the implementation reads
// that file, never checking a grant itself. `{"$path": "app:/tmp/a.png"}`
// becomes `{"$file": "/…/tmp/a.png"}`.
//
// A component that is a symbolic link is refused, as storage's directory
// handles never follow one out of the root; so are a missing file, a
// folder, and a file over 64 MiB (the bound of `exactBodyFrom`).
import Foundation

enum ModulePaths {
    static let maxBytes: UInt64 = 64 << 20

    /// `body`, a call of an extension's name, with each admitted path resolved, or the module's error answer
    /// (`{"error", "message"}`) when one does not resolve.
    enum Resolved { case body(Data), answer(Data) }

    static func resolve(_ body: Data, roots: [String: String]) -> Resolved {
        guard body.range(of: Data("\"$path\"".utf8)) != nil,
              let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any],
              json["extension"] is String else { return .body(body) }
        do {
            let resolved = try walk(json, roots)
            return .body((try? JSONSerialization.data(withJSONObject: resolved)) ?? body)
        } catch let refused as Refused {
            let answer: [String: Any] = ["error": refused.name, "message": refused.message]
            return .answer((try? JSONSerialization.data(withJSONObject: answer)) ?? Data())
        } catch {
            return .body(body)
        }
    }

    struct Refused: Error { let name: String; let message: String }

    private static func walk(_ value: Any, _ roots: [String: String]) throws -> Any {
        if let object = value as? [String: Any] {
            if object.count == 1, let path = object["$path"] as? String {
                return ["$file": try file(path, roots)]
            }
            return try object.mapValues { try walk($0, roots) }
        }
        if let list = value as? [Any] { return try list.map { try walk($0, roots) } }
        return value
    }

    /// The file `path` names under `roots` (`data`, `cache`, `tmp`).
    static func file(_ path: String, _ roots: [String: String]) throws -> String {
        guard path.hasPrefix("app:/") else { throw Refused(name: "DataError", message: "\(path): an app:/ path") }
        let parts = path.dropFirst(5).split(separator: "/", omittingEmptySubsequences: false).map(String.init)
        guard let root = parts.first, let base = roots[root] else {
            throw Refused(name: "NotFoundError", message: "\(path): no app files at app:/\(parts.first ?? "") here")
        }
        let rest = parts.dropFirst()
        guard !rest.isEmpty, !rest.contains(where: { $0.isEmpty || $0 == "." || $0 == ".." || $0.contains("\0") }) else {
            throw Refused(name: "DataError", message: "\(path): an app:/ path has no empty, . or .. segment")
        }
        var at = base
        for component in rest {
            at = (at as NSString).appendingPathComponent(component)
            var info = stat()
            guard lstat(at, &info) == 0 else { throw Refused(name: "NotFoundError", message: "\(path): no such file") }
            if info.st_mode & S_IFMT == S_IFLNK { throw Refused(name: "NotAllowedError", message: "\(path): a symbolic link") }
            if component == rest.last {
                guard info.st_mode & S_IFMT == S_IFREG else { throw Refused(name: "DataError", message: "\(path): not a file") }
                guard UInt64(info.st_size) <= maxBytes else {
                    throw Refused(name: "DataError", message: "\(path): \(info.st_size) bytes is more than the 64 MiB a module reads")
                }
            }
        }
        return at
    }
}
