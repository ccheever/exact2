// `copyText` (Session.swift). Under the agent nothing reaches the person's
// pasteboard (LLP 1133.006 §7): it writes a private named pasteboard, the
// one `@exact/clipboard`'s implementation writes as its own substitute,
// unless `EXACT_DEVICE=real` asks for the machine's (LLP 1069.007 D8). One
// per process, released when the process exits, since a named pasteboard
// outlives its writer.
#if os(macOS)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

enum AgentPasteboard {
    static let substituted = ExactEnv.agentMode && ExactEnv.environment["EXACT_DEVICE"] != "real" && {
        #if os(macOS)
        atexit { NSPasteboard(name: NSPasteboard.Name("dev.exact.agent.\(getpid())")).releaseGlobally() }
        #endif
        return true
    }()
    static var name: String { "dev.exact.agent.\(getpid())" }

    /// `copyText(text)`: the general pasteboard, or under the agent the
    /// private one. tvOS has no pasteboard.
    static func copy(_ text: String, log: (String) -> Void) {
        #if os(tvOS)
        fputs("exact: copyText: no pasteboard\n", stderr)
        #else
        #if canImport(UIKit)
        let pasteboard = substituted ? (UIPasteboard(name: UIPasteboard.Name(name), create: true) ?? .general) : .general
        pasteboard.string = text
        #else
        let pasteboard = substituted ? NSPasteboard(name: NSPasteboard.Name(name)) : .general
        pasteboard.clearContents()
        if !pasteboard.setString(text, forType: .string) { fputs("exact: copyText failed\n", stderr) }
        #endif
        if substituted { log("copyText: \(text.count) characters to the drive's pasteboard") }
        #endif
    }
}
