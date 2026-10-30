// The pasteboard a drive writes instead of the person's (LLP 1133.006 §7):
// `copyText`'s, and @exact/clipboard's implementation's by the same rule and
// name (packages/clipboard/apple/Clipboard.swift). Under the agent, unless the
// drive binds the clipboard as in production (`--bind clipboard`,
// `EXACT_AGENT_BIND`, which the implementation hears as `context.agent`) or
// asks for the real device (`EXACT_DEVICE=real`, LLP 1069.007 D8). Named for
// the session, and removed when it ends or the process exits, since a named
// pasteboard outlives its writer. Where it cannot be made nothing is written.
import Foundation
#if os(macOS)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

enum AgentPasteboard {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var named: Set<String> = []
    private static let removedAtExit: Void = { atexit { AgentPasteboard.removeAll() } }()

    /// Session `owner`'s private pasteboard, registered for removal at exit, or nil where writes go to the person's.
    static func name(_ owner: String) -> String? {
        guard ExactEnv.agentMode, !ExactEnv.agentBind.contains("clipboard"), ExactEnv.environment["EXACT_DEVICE"] != "real" else { return nil }
        let name = "exact.agent.clipboard.\(owner)"
        _ = removedAtExit
        lock.lock(); named.insert(name); lock.unlock()
        return name
    }

    /// `copyText(text)`: the person's pasteboard, or `name`'s. tvOS has none.
    static func copy(_ text: String, to name: String?, log: (String) -> Void) {
        #if os(tvOS)
        fputs("exact: copyText: no pasteboard\n", stderr)
        #elseif canImport(UIKit)
        let board = name.map { UIPasteboard(name: UIPasteboard.Name($0), create: true) }
        if case .some(nil) = board { return log("copyText: refused, the drive's private pasteboard could not be made") }
        (board.flatMap { $0 } ?? .general).string = text
        #else
        let board = name.map { NSPasteboard(name: NSPasteboard.Name($0)) } ?? .general
        board.clearContents()
        if !board.setString(text, forType: .string) { fputs("exact: copyText failed\n", stderr) }
        #endif
    }

    /// The session's private pasteboard goes with it.
    static func remove(_ name: String) {
        lock.lock(); named.remove(name); lock.unlock()
        release(name)
    }

    private static func removeAll() {
        lock.lock(); let all = named; named = []; lock.unlock()
        all.forEach(release)
    }

    private static func release(_ name: String) {
        #if os(macOS)
        NSPasteboard(name: NSPasteboard.Name(name)).releaseGlobally()
        #elseif os(iOS)
        UIPasteboard.remove(withName: UIPasteboard.Name(name))
        #endif
    }
}
