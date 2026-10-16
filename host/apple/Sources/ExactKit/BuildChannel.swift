// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): a development
// build registers with its dev stream's nonce, reports its effective facts,
// and takes `pin` from the design server's clients; `inspect`, `outline` and
// `hold` are reserved (T2b, stage 8), answered unsupported. The server's half
// is scripts/tamber-builds.mjs; the web page's, host/web/dev.js. A pin sets
// what the agent's `prefer` sets (Agent.swift), by the same means.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// A pin as the server sends it (scripts/tamber-builds.mjs `pinRefused`).
struct BuildPin: Equatable {
    var scheme: String?, contrast: String?, mode: [String: String]?, rootFontSize: Double?

    /// A frame's `pin`: nil for null (hand everything back), or why it is
    /// not one.
    static func parse(_ value: Any?) -> (pin: BuildPin?, refused: String?) {
        if value == nil || value is NSNull { return (nil, nil) }
        guard let o = value as? [String: Any] else { return (nil, "`pin` is an object of facts, or null") }
        var pin = BuildPin()
        for (key, raw) in o {
            switch key {
            case "scheme":
                guard let v = raw as? String, ["light", "dark"].contains(v) else { return (nil, "`scheme` is light or dark") }
                pin.scheme = v
            case "contrast":
                guard let v = raw as? String, ["more", "less", "no-preference"].contains(v) else { return (nil, "`contrast` is more, less or no-preference") }
                pin.contrast = v
            case "mode":
                guard let v = raw as? [String: String], !v.contains(where: { "\($0.key)\($0.value)".contains(where: { $0 == "," || $0 == "=" }) }) else { return (nil, "`mode` is {axis: value}, with no `,` or `=`") }
                pin.mode = v
            case "rootFontSize":
                guard let n = raw as? NSNumber, CFGetTypeID(n) != CFBooleanGetTypeID(), n.doubleValue.isFinite, (4...64).contains(n.doubleValue) else { return (nil, "`rootFontSize` is px, 4 to 64") }
                pin.rootFontSize = n.doubleValue
            default: return (nil, "`pin` names no fact `\(key)`")
            }
        }
        return (pin, nil)
    }
}

/// The channel's state: the process's (one app, D11), as the agent's
/// preferences are.
enum BuildChannel {
    /// The frame kinds dispatched before any generation handling.
    static let commands: Set<String> = ["pin", "inspect", "outline", "hold"]
    #if os(macOS)
    static let host = "macos"
    #else
    static let host = "ios"
    #endif
    /// This process's own id, stable across its reconnects: its pins are
    /// replayed when it registers again.
    static let id = "\(host)-" + String(UInt64.random(in: 1...UInt64.max), radix: 36)
    /// The design digest last applied (a live batch's or snapshot's `design`).
    nonisolated(unsafe) static var design: String?
    /// The last pin frame's revision applied.
    nonisolated(unsafe) static var revision: Int?
    /// The pin in force, re-applied to a session that boots under it.
    nonisolated(unsafe) static var pinned: BuildPin?
    /// What was there before the first pin: undone by the next pin before
    /// it applies, and by null for good.
    nonisolated(unsafe) static var before: (restore: [() -> Void], contrast: String?, mode: [String: String]?)?
}

extension ExactApp {
    private var bootedSessions: [ExactSession] { sessions.filter { $0.state != .destroyed && $0.booted } }

    /// A build-channel frame from the dev stream (`PlanURL`): a hello with a
    /// new nonce registers; a command applies, then reports.
    func channel(_ frame: [String: Any]) {
        guard let connection else { return }
        if frame["hello"] as? Bool == true {
            connection.report(["kind": "register", "id": BuildChannel.id, "host": BuildChannel.host, "capabilities": ["pin"], "design": BuildChannel.design ?? NSNull()])
            reportStatus()
            return
        }
        let kind = frame["kind"] as? String ?? ""
        guard kind == "pin" else { connection.report(["kind": "unsupported", "what": kind]); return }
        let (pin, refused) = BuildPin.parse(frame["pin"])
        if let refused { connection.report(["kind": "unsupported", "what": "pin", "detail": refused]); return }
        self.pin(pin)
        BuildChannel.revision = (frame["revision"] as? NSNumber)?.intValue
        reportStatus()
    }

    /// The effective facts, as the server's `build-status` shows them.
    func reportStatus() {
        guard let connection else { return }
        let first = bootedSessions.first
        #if os(macOS)
        let dark = NSApp.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        #else
        let dark = (first?.view?.traitCollection ?? first?.presenter.viewport.window?.windowScene?.traitCollection)?.userInterfaceStyle == .dark
        #endif
        let modes = first.map { $0.runtime.setModes([:], check: true).modes ?? [:] }
        connection.report(["kind": "status", "scheme": dark ? "dark" : "light", "contrast": DisplayPreferences.contrast,
                           "mode": modes ?? NSNull(), "rootFontSize": PageFacts.rootFontSize,
                           "design": BuildChannel.design ?? NSNull(), "revision": BuildChannel.revision ?? NSNull()])
    }

    /// A pin on every booted session, as `prefer` sets each fact: the system
    /// scheme beneath the app's `setScheme`, the contrast setting, the
    /// window's modes, the root font size. A fact it does not name is what
    /// was there before the first pin; null hands every one back.
    private func pin(_ pin: BuildPin?) {
        let booted = bootedSessions
        if BuildChannel.before == nil {
            guard pin != nil else { return }
            var restore: [() -> Void] = []
            #if os(macOS)
            let (system, app) = (Agent.systemAppearance, NSApp.appearance)
            restore.append {
                let following = NSApp.appearance == nil || NSApp.appearance === Agent.systemAppearance
                Agent.systemAppearance = system
                if following { NSApp.appearance = app }
                DisplayPreferences.changed()
            }
            #else
            for session in booted {
                guard let scene = session.presenter.viewport.window?.windowScene else { continue }
                let style = scene.traitOverrides.contains(UITraitUserInterfaceStyle.self) ? scene.traitOverrides.userInterfaceStyle : nil
                let contrast = scene.traitOverrides.contains(UITraitAccessibilityContrast.self) ? scene.traitOverrides.accessibilityContrast : nil
                restore.append { [weak scene, weak session] in
                    guard let scene else { return }
                    if let style { scene.traitOverrides.userInterfaceStyle = style } else { scene.traitOverrides.remove(UITraitUserInterfaceStyle.self) }
                    if let contrast { scene.traitOverrides.accessibilityContrast = contrast } else { scene.traitOverrides.remove(UITraitAccessibilityContrast.self) }
                    scene.windows.forEach { $0.updateTraitsIfNeeded() }
                    session?.view?.updateTraitsIfNeeded()
                }
            }
            #endif
            BuildChannel.before = (restore, DisplayPreferences.agentContrast, booted.first?.runtime.setModes([:], check: true).modes)
        }
        guard let before = BuildChannel.before else { return }
        before.restore.forEach { $0() }
        DisplayPreferences.agentContrast = pin?.contrast ?? before.contrast
        for session in booted where pin?.scheme != nil || pin?.contrast != nil {
            let agent = session.agentInstance
            if let scheme = pin?.scheme { agent.systemScheme(dark: scheme == "dark") }
            agent.systemContrast(more: DisplayPreferences.contrast == "more")
        }
        PageFacts.pinnedRootFontSize = pin?.rootFontSize
        for session in booted { session.apply(session.runtime.setRootFontSize(PageFacts.rootFontSize)) }
        BuildChannel.pinned = pin
        let mode = (before.mode ?? [:]).merging(pin?.mode ?? [:]) { $1 }
        if !mode.isEmpty { pinModes(mode, booted) }
        if pin == nil { BuildChannel.before = nil }
    }

    /// The window's modes, checked before any session takes them (as
    /// `prefer`'s mode group); a refusal is reported, not applied.
    private func pinModes(_ mode: [String: String], _ sessions: [ExactSession]) {
        guard let first = sessions.first else { return }
        if let error = first.runtime.setModes(mode, check: true).error {
            connection?.report(["kind": "unsupported", "what": "pin.mode", "detail": error])
            return
        }
        for session in sessions { session.apply(session.runtime.setModes(mode)) }
    }

    /// A session booted under a pin takes its modes once the boot's batch is
    /// in (a generation's runner starts from the plan's).
    func repinBooted() {
        guard let mode = BuildChannel.pinned?.mode, !mode.isEmpty else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, BuildChannel.pinned?.mode == mode else { return }
            self.pinModes((BuildChannel.before?.mode ?? [:]).merging(mode) { $1 }, self.bootedSessions)
            self.reportStatus()
        }
    }
}
