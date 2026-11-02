// T3 Code 365aa87982, packages/contracts/src/git.ts GitCommandError.message (MIT, LICENSE-T3).
import Foundation

enum T3GitCommandMessage {
    static func message(explicit: String, operation: String, cwd: String, detail: String, reason: String = "") -> String {
        let text = { (value: String) in value.trimmingCharacters(in: .whitespacesAndNewlines) }
        let supplied = text(explicit)
        if !supplied.isEmpty { return supplied }
        let reason = text(reason)
        let suffix = reason.isEmpty ? "" : " (\(reason))"
        return "Git command failed in \(operation) (\(cwd)): \(detail)\(suffix)"
    }
}
