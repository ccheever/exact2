// The build channel (LLP 1122 D4, LLP 1121 stage 4 commit 5): a development
// build registers with its dev stream's nonce, reports its effective facts
// and the digest of the plan it runs (tamber/1.9), and takes `pin` from the
// design server's clients, and `inspect` and `outline` where inspect mode is
// compiled in (T2c, Inspect.swift); `hold` is reserved (stage 8), answered
// unsupported. The server's half
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
    /// A production build (`compat.json`'s `inputs.trust`) neither registers
    /// nor takes a pin, whatever its environment.
    static let enabled = !ExactEnv.productionBake
    /// The frame kinds dispatched before any generation handling.
    static let commands: Set<String> = ["pin", "inspect", "outline", "hold"]
    /// What this build speaks: inspect where it is compiled in.
    #if EXACT_INSPECT && !os(tvOS)
    static let capabilities = ["pin", "inspect", "hold"]
    #else
    static let capabilities = ["pin"]
    #endif
    /// The SHA-256 of the plan the app runs, which its source map names: the
    /// runner's own (its `sites` op), the one source `selected` reports use
    /// too, asked once per generation.
    static func planDigest(_ app: ExactApp) -> String? {
        #if EXACT_INSPECT
        guard let s = app.sessions.first(where: \.booted) else { return nil }
        if let known = Inspect.digest, known.session == ObjectIdentifier(s), known.generation == s.generation { return known.digest }
        let digest = Inspect.sites(s).plan
        Inspect.digest = (ObjectIdentifier(s), s.generation, digest)
        return digest
        #else
        return nil
        #endif
    }
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
    /// The pin in force, applied whole to a session that boots under it.
    nonisolated(unsafe) static var pinned: BuildPin?
    /// What was there before the first pin, until null hands it back.
    nonisolated(unsafe) static var held: BuildHeld?
    /// A status is owed this run-loop turn; the last one sent, on its nonce.
    nonisolated(unsafe) static var statusOwed = false
    nonisolated(unsafe) static var lastStatus: (nonce: String, body: NSDictionary)?

    /// A session's modes under a pin: its own before the first pin, the
    /// pin's over them.
    static func modes(before: [String: String], pin: BuildPin?) -> [String: String] {
        before.merging(pin?.mode ?? [:]) { $1 }
    }
}

/// What was there before the first pin: the contrast setting and the system
/// appearance (the Mac's, the process's; each iOS scene's own), and each
/// session's modes, taken when a pin first reaches it.
final class BuildHeld {
    final class Session { weak var session: ExactSession?; let modes: [String: String]; init(_ s: ExactSession, _ m: [String: String]) { session = s; modes = m } }
    let contrast = DisplayPreferences.agentContrast
    private(set) var sessions: [Session] = []
    #if os(macOS)
    let appearance = (system: Agent.systemAppearance, app: NSApp.appearance)
    #else
    final class Scene { weak var scene: UIWindowScene?; let style: UIUserInterfaceStyle?, contrast: UIAccessibilityContrast?
        init(_ s: UIWindowScene) {
            scene = s
            style = s.traitOverrides.contains(UITraitUserInterfaceStyle.self) ? s.traitOverrides.userInterfaceStyle : nil
            contrast = s.traitOverrides.contains(UITraitAccessibilityContrast.self) ? s.traitOverrides.accessibilityContrast : nil
        }
    }
    private(set) var scenes: [Scene] = []
    #endif

    /// A session's (and its scene's) before, unless already taken.
    func take(_ s: ExactSession) {
        sessions.removeAll { $0.session == nil }
        if !sessions.contains(where: { $0.session === s }) { sessions.append(Session(s, s.runtime.setModes([:], check: true).modes ?? [:])) }
        #if !os(macOS)
        scenes.removeAll { $0.scene == nil }
        if let scene = s.presenter.viewport.window?.windowScene, !scenes.contains(where: { $0.scene === scene }) { scenes.append(Scene(scene)) }
        #endif
    }
    func modes(of s: ExactSession) -> [String: String] { sessions.first { $0.session === s }?.modes ?? [:] }

    /// The system appearance as it was: a pin's are set again over it.
    func restoreAppearance() {
        #if os(macOS)
        let following = NSApp.appearance == nil || NSApp.appearance === Agent.systemAppearance
        Agent.systemAppearance = appearance.system
        if following { NSApp.appearance = appearance.app }
        DisplayPreferences.changed()
        #else
        for held in scenes {
            guard let scene = held.scene else { continue }
            if let style = held.style { scene.traitOverrides.userInterfaceStyle = style } else { scene.traitOverrides.remove(UITraitUserInterfaceStyle.self) }
            if let contrast = held.contrast { scene.traitOverrides.accessibilityContrast = contrast } else { scene.traitOverrides.remove(UITraitAccessibilityContrast.self) }
            scene.windows.forEach { $0.updateTraitsIfNeeded() }
        }
        for held in sessions { held.session?.view?.updateTraitsIfNeeded() }
        #endif
    }
}

extension ExactApp {
    private var bootedSessions: [ExactSession] { sessions.filter { $0.state != .destroyed && $0.booted } }

    /// A build-channel frame from the dev stream (`PlanURL`): a hello with a
    /// new nonce registers; a command applies, then reports.
    func channel(_ frame: [String: Any]) {
        guard BuildChannel.enabled, let connection else { return }
        if frame["hello"] as? Bool == true {
            connection.report(["kind": "register", "id": BuildChannel.id, "host": BuildChannel.host, "capabilities": BuildChannel.capabilities, "planDigest": BuildChannel.planDigest(self) ?? NSNull(), "design": BuildChannel.design ?? NSNull()])
            BuildChannel.lastStatus = nil
            scheduleStatus()
            return
        }
        let kind = frame["kind"] as? String ?? ""
        #if EXACT_INSPECT && !os(tvOS)
        if kind == "inspect" || kind == "outline" { inspectCommand(frame); return }
        if kind == "hold" { holdCommand(frame); return }
        #endif
        guard kind == "pin" else { connection.report(["kind": "unsupported", "what": kind]); return }
        let (pin, refused) = BuildPin.parse(frame["pin"])
        // A refusal names the pin's revision (1.3): the server binds it to that pin.
        let revision = (frame["revision"] as? NSNumber)?.intValue
        if let refused { connection.report(["kind": "unsupported", "what": "pin", "detail": refused, "revision": revision.map { $0 as Any } ?? NSNull()]); return }
        BuildChannel.revision = revision
        self.pin(pin)
        scheduleStatus()
    }

    /// A status at the end of this run-loop turn, one however many facts
    /// changed in it (a pin, a design, a generation, the scheme, the system's
    /// or the app's `setScheme`, the contrast, the app's `setMode` or
    /// `setRootFontSize`: a batch's `facts`), and none that says what the
    /// last did.
    func scheduleStatus() {
        guard BuildChannel.enabled, connection?.nonce != nil, !BuildChannel.statusOwed else { return }
        BuildChannel.statusOwed = true
        DispatchQueue.main.async { [weak self] in
            BuildChannel.statusOwed = false
            self?.reportStatus()
        }
    }

    /// The effective facts, as the server's `build-status` shows them: the
    /// scheme the app shows, the contrast setting, the window's modes, the
    /// root font size the runner lays out by (after the app's own
    /// `setRootFontSize`), the generation and design it runs.
    private func reportStatus() {
        guard let connection, let nonce = connection.nonce else { return }
        let first = bootedSessions.first
        #if os(macOS)
        let dark = (first?.view?.effectiveAppearance ?? NSApp.effectiveAppearance).bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        #else
        let dark = (first?.view?.traitCollection ?? first?.presenter.viewport.window?.windowScene?.traitCollection)?.userInterfaceStyle == .dark
        #endif
        let modes = first.map { $0.runtime.setModes([:], check: true).modes ?? [:] }
        let body: [String: Any] = ["kind": "status", "scheme": dark ? "dark" : "light", "contrast": DisplayPreferences.contrast,
                                   "mode": modes ?? NSNull(), "rootFontSize": first.flatMap(renderedRootFontSize) ?? PageFacts.rootFontSize,
                                   "plan": connection.plan ?? NSNull(), "planDigest": BuildChannel.planDigest(self) ?? NSNull(), "design": BuildChannel.design ?? NSNull(), "revision": BuildChannel.revision.map { $0 as Any } ?? NSNull()]
        if let last = BuildChannel.lastStatus, last.nonce == nonce, last.body.isEqual(to: body) { return }
        BuildChannel.lastStatus = (nonce, body as NSDictionary)
        connection.report(body)
    }

    /// The runner's own root font size (the agent's `state`, `device`).
    private func renderedRootFontSize(_ s: ExactSession) -> Double? {
        guard let data = s.agent("{\"op\":\"state\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return ((o["device"] as? [String: Any])?["rootFontSize"] as? NSNumber)?.doubleValue
    }

    /// A pin on every booted session, as `prefer` sets each fact: the system
    /// scheme beneath the app's `setScheme`, the contrast setting, the
    /// window's modes, the root font size. A fact it does not name is what
    /// was there before the first pin; null hands every one back.
    private func pin(_ pin: BuildPin?) {
        let booted = bootedSessions
        if BuildChannel.held == nil {
            guard pin != nil else { return }
            BuildChannel.held = BuildHeld()
        }
        guard let held = BuildChannel.held else { return }
        booted.forEach(held.take)
        held.restoreAppearance()
        BuildChannel.pinned = pin
        apply(pin, to: booted, held)
        if pin == nil { BuildChannel.held = nil }
    }

    /// `pin` set on `sessions`, over what `held` keeps. A session whose app
    /// refuses the pin's modes runs its own again, and it is reported.
    private func apply(_ pin: BuildPin?, to sessions: [ExactSession], _ held: BuildHeld) {
        DisplayPreferences.agentContrast = pin?.contrast ?? held.contrast
        for session in sessions where pin?.scheme != nil || pin?.contrast != nil {
            let agent = session.agentInstance
            if let scheme = pin?.scheme { agent.systemScheme(dark: scheme == "dark") }
            agent.systemContrast(more: DisplayPreferences.contrast == "more")
        }
        PageFacts.pinnedRootFontSize = pin?.rootFontSize
        for session in sessions { session.apply(session.runtime.setRootFontSize(PageFacts.rootFontSize)) }
        var refused: String?
        for session in sessions {
            let before = held.modes(of: session), mode = BuildChannel.modes(before: before, pin: pin)
            guard !mode.isEmpty else { continue }
            if let error = session.runtime.setModes(mode, check: true).error {
                refused = refused ?? error
                if !before.isEmpty { session.apply(session.runtime.setModes(before)) }
                continue
            }
            session.apply(session.runtime.setModes(mode))
        }
        if let refused { connection?.report(["kind": "unsupported", "what": "pin.mode", "detail": refused, "revision": BuildChannel.revision.map { $0 as Any } ?? NSNull()]) }
    }

    /// A session (or scene) that boots under a pin takes the whole of it,
    /// once the boot's batch is in (a generation's runner starts from the
    /// plan's); either way the generation it runs is reported.
    func repinBooted() {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            if BuildChannel.pinned != nil, let held = BuildChannel.held {
                let booted = self.bootedSessions
                booted.forEach(held.take)
                self.apply(BuildChannel.pinned, to: booted, held)
            }
            self.scheduleStatus()
        }
    }
}
