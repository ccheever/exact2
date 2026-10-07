// The app and window scopes (@ref LLP 1075.003.000.001 §2.1): above the
// containers and the nodes, the module hears of the app (its facts, when it
// connects, when one changes, when the session ends or reloads) and of the
// window the session presents into (built, a size or safe-area change, the
// session leaving it).
//
// A session rarely knows whether the window is its own (§2.1.1), so the
// embedder says: `hatchesOwnWindow` hands a hatch the window itself, and
// `hatchesOwnProcess` the application object, for process-wide appearance.
// Neither is inferred. Without the first a hatch gets the window's frame and
// moments and no window; without the second, no application.
//
// The module's entries (NativeModule.swift reads them from its table):
//
//   184  app(module, event, application, json, len)
//          event 0 built, 1 changed, 2 ended; json {"facts": {…}, "processOwner"}
//   192  window(module, event, window, scene, json, len)
//          event 0 built, 1 changed, 2 ended; json {"frame", "safeArea", "exclusive"}
//
// Built runs as the hatches connect, after first pixel; changed and ended
// never inside a batch.
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

typealias HatchAppFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void
typealias HatchWindowFn = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt32) -> Void

public extension ExactSession {
    /// The embedder's word that this session's window is its own (LLP
    /// 1075.003.000.001 §2.1.1): a window hatch is then handed the window.
    /// Set before the session's first frame. False unless said.
    var hatchesOwnWindow: Bool {
        get { natives.scopes.windowExclusive }
        set { natives.scopes.windowExclusive = newValue }
    }
    /// The embedder's word that this session may set process-wide state
    /// (appearance proxies, the application object): at most one session in a
    /// process. False unless said.
    var hatchesOwnProcess: Bool {
        get { natives.scopes.processOwner }
        set { natives.scopes.processOwner = newValue }
    }
}

/// What a session's app and window hatches have been told, so a change is told once.
final class HatchScopes {
    var windowExclusive = false, processOwner = false
    var appCall: HatchAppFn?, windowCall: HatchWindowFn?
    var appTold: String?, windowTold: (window: ObjectIdentifier, said: String)?
    var preferenceObservers: [NSObjectProtocol] = [], pageObservers: [NSObjectProtocol] = [], pending = false
}

extension NativeViews {
    /// The app's facts as a hatch reads them, by source (§2.1): the page's
    /// (LLP 1069.000 D2) and the display preferences `prefer` sets.
    private var appFacts: [String: Any] {
        #if os(macOS)
        let dark = DisplayPreferences.systemDark
        #else
        // The scene's, as the session tells Contract (Session.swift `preferenceBits`).
        let dark = session?.view?.window?.windowScene?.traitCollection.userInterfaceStyle == .dark
        #endif
        return ["visibilityState": PageFacts.hidden ? "hidden" : "visible", "onLine": PageFacts.onLine,
                "prefersColorScheme": dark ? "dark" : "light", "prefersContrast": DisplayPreferences.contrast,
                "prefersReducedMotion": DisplayPreferences.reducedMotion, "prefersReducedTransparency": DisplayPreferences.reducedTransparency]
    }

    private func encoded(_ object: [String: Any]) -> Data {
        (try? JSONSerialization.data(withJSONObject: object, options: .sortedKeys)) ?? Data("{}".utf8)
    }

    /// `app`: built, changed (a fact moved) or ended.
    func appHatch(_ event: UInt32) {
        guard hatchesConnected, let instance, let call = scopes.appCall else { return }
        let json = encoded(["facts": appFacts, "processOwner": scopes.processOwner])
        scopes.appTold = event == 2 ? nil : String(decoding: json, as: UTF8.self)
        let moment = ["built", "changed", "ended"][Int(min(event, 2))]
        session?.log("hatch app: \(moment)")
        #if os(macOS)
        let application: AnyObject? = scopes.processOwner ? NSApp : nil
        #else
        let application: AnyObject? = scopes.processOwner ? UIApplication.shared : nil
        #endif
        timedHatch("app", moment) {
            json.withUnsafeBytes { j in
                call(instance, event, application.map { Unmanaged.passUnretained($0).toOpaque() }, j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count))
            }
        }
        if event == 2 { session?.presenter.elements.regions.ended(scope: "app") }
    }

    private func windowJSON(_ view: ExactView, _ window: FocusWindow) -> Data {
        let frame = view.convert(view.bounds, to: nil)
        let r = { (x: CGFloat) in (Double(x) * 100).rounded() / 100 }
        let insets = view.safeAreaInsets, safe = [r(insets.top), r(insets.right), r(insets.bottom), r(insets.left)]
        return encoded(["frame": [r(frame.minX), r(frame.minY), r(frame.width), r(frame.height)], "safeArea": safe, "exclusive": scopes.windowExclusive])
    }

    /// `window` for the surface the session presents into: built (0), a
    /// size or safe-area change (1), or the session leaving it (2).
    func windowHatch(_ event: UInt32, window given: FocusWindow? = nil) {
        guard hatchesConnected, let instance, let call = scopes.windowCall, let view = session?.view, let window = given ?? view.window else { return }
        let json = windowJSON(view, window)
        scopes.windowTold = event == 2 ? nil : (ObjectIdentifier(window), String(decoding: json, as: UTF8.self))
        let moment = ["built", "changed", "ended"][Int(min(event, 2))]
        session?.log("hatch window: \(moment)\(scopes.windowExclusive ? "" : " (not exclusive: no window is handed over)")")
        #if os(macOS)
        let scene: AnyObject? = nil
        #else
        let scene: AnyObject? = scopes.windowExclusive ? window.windowScene : nil
        #endif
        let raw = { (o: AnyObject?) in o.map { Unmanaged.passUnretained($0).toOpaque() } }
        timedHatch("window", moment) {
            json.withUnsafeBytes { j in
                call(instance, event, raw(scopes.windowExclusive ? window : nil), raw(scene), j.bindMemory(to: UInt8.self).baseAddress, UInt32(json.count))
            }
        }
        if event == 2 { session?.presenter.elements.regions.ended(scope: "window") }
    }

    /// As the hatches connect: `app`, then `window` for the surface there
    /// is, and the facts watched from now on.
    func connectScopes(app: HatchAppFn?, window: HatchWindowFn?) {
        scopes.appCall = app
        scopes.windowCall = window
        guard app != nil || window != nil else { return }
        appHatch(0)
        windowHatch(0)
        guard scopes.preferenceObservers.isEmpty, scopes.pageObservers.isEmpty else { return }
        scopes.preferenceObservers = DisplayPreferences.observe { [weak self] in self?.scopesChanged() }
        scopes.pageObservers = PageFacts.observe { [weak self] in self?.scopesChanged() }
    }

    /// A fact, the window's size or its safe area may have moved: what is
    /// now different from what was told is told, on the next turn, so never
    /// inside a batch and once however many moved.
    func scopesChanged() {
        guard hatchesConnected, !scopes.pending, scopes.appCall != nil || scopes.windowCall != nil else { return }
        scopes.pending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.scopes.pending = false
            guard self.hatchesConnected else { return }
            if let told = self.scopes.appTold, told != String(decoding: self.encoded(["facts": self.appFacts, "processOwner": self.scopes.processOwner]), as: UTF8.self) { self.appHatch(1) }
            guard let view = self.session?.view else { return }
            switch (self.scopes.windowTold, view.window) {
            case (nil, .some): self.windowHatch(0)
            case (.some(let told), .some(let window)) where told.window == ObjectIdentifier(window):
                if told.said != String(decoding: self.windowJSON(view, window), as: UTF8.self) { self.windowHatch(1) }
            default: break
            }
        }
    }

    /// The session is leaving `window` (for another, or for none): its
    /// window hatch ends while the window is still there.
    func windowLeaving(_ window: FocusWindow?) {
        guard let window, let told = scopes.windowTold, told.window == ObjectIdentifier(window) else { return }
        windowHatch(2, window: window)
    }

    /// A reload or the session's end: `window` then `app` end, inside the
    /// reset, so a hatch takes back what it added to a window that survives.
    /// After a reload both are built again, on the next turn, with new handles.
    func scopesReset() {
        // What the old incarnation registered on the clock goes with it (§2.1.2).
        hatchClock.reset()
        guard hatchesConnected, scopes.appTold != nil || scopes.windowTold != nil else { return }
        if scopes.windowTold != nil { windowHatch(2) }
        if scopes.appTold != nil { appHatch(2) }
        DispatchQueue.main.async { [weak self] in
            guard let self, self.hatchesConnected, self.scopes.appTold == nil else { return }
            self.appHatch(0)
            self.windowHatch(0)
        }
    }

    /// The session's end: `window` then `app` end, if a reset has not ended
    /// them already, and the facts are no longer watched.
    func endScopes() {
        if hatchesConnected {
            if scopes.windowTold != nil { windowHatch(2) }
            if scopes.appTold != nil { appHatch(2) }
        }
        DisplayPreferences.forget(scopes.preferenceObservers)
        PageFacts.forget(scopes.pageObservers)
        scopes.preferenceObservers = []; scopes.pageObservers = []
    }
}
