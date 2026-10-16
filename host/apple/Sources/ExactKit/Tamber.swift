// The binding core on the Apple hosts (LLP 1121 D12, stage 5): a design
// token by its name, resolved for a place in one session, for code outside
// the plan. In the process (a host screen, a hatch's object):
// `ExactTamber(view).color(.surfaceCard)`; a native module's library has the
// same API over the hatches' host table (`tamberAnswer`).
//
// A colour is a dynamic provider over the view's traits, so scheme, contrast
// and elevation resolve as LLP 1095 D5 says; what the session answered is
// kept until something it read moves (`inputs`: `setMode`, a pin above, a
// live batch, the scheme, the root font size), which every batch checks, so
// the provider reads the place's app tuple as it is now. `observe` hears it.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import CExact

#if os(macOS)
public typealias ExactTamberView = NSView
public typealias ExactTamberColor = NSColor
#else
public typealias ExactTamberView = UIView
public typealias ExactTamberColor = UIColor
#endif

/// A design token by its name (`contract tamber gen swift` writes each as a
/// static member: `.surfaceCard`).
public struct ExactTamberToken: Hashable, Sendable {
    public let name: String
    public init(_ name: String) { self.name = name }
}

/// A session's tokens for one place: a view of its, or its root.
public struct ExactTamber {
    private let place: Place

    /// The place `view` is in: its nearest node's, in that node's session.
    public init(_ view: ExactTamberView) {
        var at: ExactTamberView? = view
        while let v = at, !(v is NodeView) { at = v.superview }
        let node = at as? NodeView
        place = Place(session: node?.presenter?.session, view: node?.id ?? 0)
    }

    /// The session's root: the window's app tuple.
    public init(session: ExactSession) {
        place = Place(session: session, view: 0)
    }

    /// `token`'s colour here, dynamic over the traits it is drawn with;
    /// `fallback` where the plan lacks it (said once in the session's log).
    public func color(_ token: ExactTamberToken, fallback: ExactTamberColor = .clear) -> ExactTamberColor {
        let place = place
        let side = { (dark: Bool, contrast: Bool, elevated: Bool) -> ExactTamberColor? in
            place.value(token.name)?.textChannels(dark: dark, contrast: contrast, elevated: elevated).map(TextEngine.color)
        }
        #if os(macOS)
        return NSColor(name: nil) { appearance in
            let match = appearance.bestMatch(from: [.aqua, .darkAqua, .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua])
            let dark = match == .darkAqua || match == .accessibilityHighContrastDarkAqua
            let contrast = match == .accessibilityHighContrastAqua || match == .accessibilityHighContrastDarkAqua
            return side(dark, contrast, false) ?? fallback
        }
        #elseif os(tvOS)
        return UIColor { traits in side(traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high, false) ?? fallback }
        #else
        return UIColor { traits in
            side(traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high, traits.userInterfaceLevel == .elevated) ?? fallback
        }
        #endif
    }

    /// `body` whenever what this place's tokens read moves (after the batch
    /// that moved it); stop it with the observation's `stop()`.
    public func observe(_ body: @escaping () -> Void) -> ExactTamberObservation {
        let observation = ExactTamberObservation(body)
        place.observe(observation)
        return observation
    }

    /// A place, and what its session answered for it.
    final class Place {
        weak var session: ExactSession?
        let view: UInt32
        init(session: ExactSession?, view: UInt32) { self.session = session; self.view = view }

        /// `name`'s value as a row crosses it, asked once per inputs.
        func value(_ name: String) -> BatchValue? {
            guard let session else { return nil }
            let known = Tambers.of(session).known[view]
            if let known, known.colors.keys.contains(name) { return known.colors[name] ?? nil }
            let asked = Array(Set((known?.colors.keys).map(Array.init) ?? []).union([name]))
            guard let answer = session.runtime.tamber(view: view, names: asked) else { return nil }
            Tambers.of(session).known[view] = answer
            return answer.colors[name] ?? nil
        }

        func observe(_ observation: ExactTamberObservation) {
            guard let session else { return }
            let tambers = Tambers.of(session)
            tambers.observers.append((view, Weak(observation)))
            if tambers.known[view] == nil, let answer = session.runtime.tamber(view: view, names: []) { tambers.known[view] = answer }
        }
    }
}

/// An `observe`'s handle: `stop()` ends it, as dropping the last reference does.
public final class ExactTamberObservation {
    fileprivate var body: (() -> Void)?
    fileprivate init(_ body: @escaping () -> Void) { self.body = body }
    public func stop() { body = nil }
}

/// One session's binding state: each place's last answer, and the observers.
final class Tambers {
    /// Each place's inputs and the colours asked of it since they last moved.
    var known: [UInt32: (inputs: String, colors: [String: BatchValue?])] = [:]
    var observers: [(view: UInt32, observation: Weak<ExactTamberObservation>)] = []
    nonisolated(unsafe) private static var bySession: [ObjectIdentifier: (session: Weak<ExactSession>, tambers: Tambers)] = [:]

    static func of(_ session: ExactSession) -> Tambers {
        let key = ObjectIdentifier(session)
        if let entry = bySession[key], entry.session.value === session { return entry.tambers }
        bySession = bySession.filter { $0.value.session.value != nil }
        let tambers = Tambers()
        bySession[key] = (Weak(session), tambers)
        return tambers
    }

    /// After `session` applied a batch: each place whose inputs moved is
    /// asked again when next drawn, and its observers hear it.
    static func moved(_ session: ExactSession) {
        // A native module's bindings ask again themselves (ExactTamber.swift).
        session.natives.tamberMoved()
        guard let entry = bySession[ObjectIdentifier(session)], entry.session.value === session else { return }
        let tambers = entry.tambers
        tambers.observers.removeAll { $0.observation.value?.body == nil }
        var moved: Set<UInt32> = []
        for (view, known) in tambers.known {
            guard let now = session.runtime.tamber(view: view, names: []) else { continue }
            if now.inputs != known.inputs { moved.insert(view); tambers.known[view] = now }
        }
        for (view, observation) in tambers.observers where moved.contains(view) { observation.value?.body?() }
    }
}

/// A weak reference, for a table that must not keep what it names alive.
struct Weak<T: AnyObject> {
    weak var value: T?
    init(_ value: T) { self.value = value }
}

extension Runtime {
    /// The binding core's answer for `view` (0: the root) and `names`: what
    /// its inputs are, and each name's colour as a row crosses it (`nil` for
    /// a name the plan lacks); `nil` before boot or for a view that is gone.
    func tamber(view: UInt32, names: [String]) -> (inputs: String, colors: [String: BatchValue?])? {
        let quoted = names.map { String(decoding: (try? JSONEncoder().encode($0)) ?? Data("\"\"".utf8), as: UTF8.self) }
        let request = "{\"view\":\(view),\"names\":[" + quoted.joined(separator: ",") + "]}"
        return on(busy: nil) { () -> (inputs: String, colors: [String: BatchValue?])? in
            let len = exact_tamber(rt, write(request))
            let bytes = UnsafeBufferPointer(start: exact_out(rt), count: Int(len))
            var reader = BatchReader(bytes: bytes)
            var inputs: String?
            var colors: [String: BatchValue?] = [:]
            do {
                try reader.object { r, key in
                    switch key {
                    case "inputs": inputs = try r.string()
                    case "colors": try r.object { r, name in let v = try r.value(); colors[name] = v == .null ? nil : v }
                    default: _ = try r.value()
                    }
                }
            } catch { return nil }
            return inputs.map { ($0, colors) }
        }
    }
}
