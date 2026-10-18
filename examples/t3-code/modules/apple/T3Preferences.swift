// The versioned preference file, `t3-code.json`, under the module's data directory: the data
// module's selections, drafts and preferences (client.ts writes it whole; `readPreferences`,
// `writePreferences`), which the SnapShot shortcut and the embedded server's one-time carry-over also read.
import Foundation

enum T3Preferences {
    static let maximumBytes = 16 * 1024 * 1024

    static func read(_ url: URL?) throws -> String {
        guard let url else { throw T3Failure(kind: "Persistence", message: "The app data directory is unavailable.") }
        do {
            let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
            guard (attributes[.size] as? NSNumber)?.intValue ?? 0 <= maximumBytes else {
                throw T3Failure(kind: "Limit", message: "Saved drafts and preferences are too large.")
            }
            guard let text = String(data: try Data(contentsOf: url), encoding: .utf8) else {
                throw T3Failure(kind: "Persistence", message: "Saved drafts and preferences are not valid UTF-8.")
            }
            return text
        } catch let error as CocoaError where error.code == .fileReadNoSuchFile { return "" }
        catch let error as T3Failure { throw error }
        catch { throw T3Failure(kind: "Persistence", message: "Could not read saved drafts and preferences: \(error.localizedDescription)") }
    }

    static func write(_ text: String, to url: URL?) throws {
        guard let url else { throw T3Failure(kind: "Persistence", message: "The app data directory is unavailable.") }
        let data = Data(text.utf8)
        guard data.count <= maximumBytes else { throw T3Failure(kind: "Limit", message: "Drafts and preferences are too large to save.") }
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: url, options: .atomic)
        } catch { throw T3Failure(kind: "Persistence", message: "Could not save drafts and preferences: \(error.localizedDescription)") }
    }
}
