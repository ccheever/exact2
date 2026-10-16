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

    /// The place `view` is in, found each time it is asked (it may be
    /// mounted, or moved, after this): its nearest node's, in that node's
    /// session.
    public init(_ view: ExactTamberView) {
        place = Place(view: view)
    }

    /// The session's root: the window's app tuple.
    public init(session: ExactSession) {
        place = Place(session: session)
    }

    /// `token`'s colour here, dynamic over the traits it is drawn with;
    /// `fallback` where the plan lacks it (said once in the session's log).
    /// After a batch moves what it read, the view the binding was made for
    /// is redrawn and laid out again, and on UIKit its traits change, so a
    /// UIKit property holding the colour, or a `draw` or `layoutSubviews`
    /// that resolved it, reads the new answer; a colour copied out of it (a
    /// layer's `cgColor`) is a value: set it again in `observe`.
    public func color(_ token: ExactTamberToken, fallback: ExactTamberColor = .clear) -> ExactTamberColor {
        let place = place
        // Asked now, on the main thread, so a provider called off it finds
        // the answer kept, with the tint it was asked under.
        if Thread.isMainThread { _ = place.value(token.name) }
        let side = { (dark: Bool, contrast: Bool, elevated: Bool) -> ExactTamberColor? in
            guard let (value, tints) = place.value(token.name), let value else { return nil }
            let tint = tints[(dark ? 1 : 0) | (contrast ? 2 : 0) | (elevated ? 4 : 0)]
            return value.textChannels(dark: dark, contrast: contrast, elevated: elevated, tint: tint).map(TextEngine.color)
        }
        #if os(macOS)
        return NSColor(name: nil) { appearance in
            let match = appearance.bestMatch(from: [.aqua, .darkAqua, .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua])
            let dark = match == .darkAqua || match == .accessibilityHighContrastDarkAqua
            let contrast = match == .accessibilityHighContrastAqua || match == .accessibilityHighContrastDarkAqua
            return side(dark, contrast, false) ?? fallback
        }
        #else
        return UIColor { traits in
            // Read, so UIKit's trait tracking redraws what resolved it when a batch moves it.
            _ = traits[TamberMoves.self]
            #if os(tvOS)
            return side(traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high, false) ?? fallback
            #else
            return side(traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high, traits.userInterfaceLevel == .elevated) ?? fallback
            #endif
        }
        #endif
    }

    /// `body` whenever what this place's tokens read moves (after the batch
    /// that moved it), wherever the view is then: observed before it is
    /// mounted, it hears the first batch after (its colours leave the
    /// fallback); moved to another node, it hears that node's. Stop it with
    /// the observation's `stop()`. Main thread.
    public func observe(_ body: @escaping () -> Void) -> ExactTamberObservation {
        let observation = ExactTamberObservation(body)
        Tambers.observe(place, observation)
        return observation
    }

    /// The tint a row naming the tint resolves by at `node` (none: the
    /// session's root): the view's inherited tint on UIKit, the nearest
    /// authored accent else the theme's on AppKit.
    static func tint(_ node: NodeView?, session: ExactSession) -> PlatformColor? {
        #if os(macOS)
        return node?.nearestAccent ?? ThemeRoles.color("AccentColor", in: session.presenter.root)
        #else
        return (node ?? session.presenter.root).tintColor
        #endif
    }

    /// `color`'s components in extended sRGB: 0–1, past it for a wide colour.
    static func extended(_ color: PlatformColor) -> [Double] {
        #if os(macOS)
        guard let c = color.usingColorSpace(.extendedSRGB) else { return [0, 0, 0, 0] }
        return [c.redComponent, c.greenComponent, c.blueComponent, c.alphaComponent].map(Double.init)
        #else
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        color.getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b, a].map(Double.init)
        #endif
    }

    /// A place, and the view it redraws.
    final class Place {
        private weak var root: ExactSession?
        weak var view: ExactTamberView?
        init(view: ExactTamberView) { self.view = view }
        init(session: ExactSession) { root = session }

        /// Where the view was last found, for a provider called off the main thread.
        private var last: (session: Weak<ExactSession>, view: UInt32)?
        /// Whether a batch that moves its answer redraws its view.
        private var listed = false
        /// The session and node its view was last drawn for (main thread):
        /// one moved to another node is drawn again.
        var drawnAt: (session: ObjectIdentifier, view: UInt32)?

        /// The session and node it is in now: a root's session, or the view's
        /// nearest node's; off the main thread, where it was last found.
        var found: (session: ExactSession, view: UInt32)? {
            if let root { return (root, 0) }
            guard Thread.isMainThread else {
                return Tambers.lock.withLock { last }.flatMap { l in l.session.value.map { ($0, l.view) } }
            }
            var at = view
            while let v = at, !(v is NodeView) { at = v.superview }
            guard let node = at as? NodeView, let session = node.presenter?.session else { return nil }
            Tambers.lock.withLock { last = (Weak(session), node.id) }
            return (session, node.id)
        }

        /// `name`'s value as a row crosses it and the tint it resolves by on
        /// each side, asked once per inputs (`nil`: nothing to answer yet);
        /// off the main thread, what was kept.
        func value(_ name: String) -> (value: BatchValue?, tints: [PlatformColor?])? {
            guard let (session, node) = found else { return nil }
            if Thread.isMainThread, view != nil {
                if !listed { listed = true; Tambers.places.append(Weak(self)) }
                drawnAt = (ObjectIdentifier(session), node)
            }
            let tambers = Tambers.of(session)
            let known = tambers.answer(node)
            if let known, let value = known.colors[name] { return (value, known.tints) }
            guard Thread.isMainThread else { return nil }
            let asked = Array(Set((known?.colors.keys).map(Array.init) ?? []).union([name]))
            guard let answer = Tambers.ask(session, view: node, names: asked) else { return nil }
            tambers.keep(node, answer)
            return (answer.colors[name] ?? nil, answer.tints)
        }
    }
}

/// An `observe`'s handle: `stop()` ends it, as dropping the last reference does.
public final class ExactTamberObservation {
    fileprivate var body: (() -> Void)?
    fileprivate init(_ body: @escaping () -> Void) { self.body = body }
    public func stop() { body = nil }
}

#if !os(macOS)
/// A trait each moved binding's view takes a new value of, so UIKit
/// resolves its dynamic colours again (the providers read it).
struct TamberMoves: UITraitDefinition {
    static let defaultValue = 0
    static let affectsColorAppearance = true
}
#endif

/// One session's binding state: each place's last answer. Its answers are
/// read by providers off the main thread, so they are kept under `lock`.
final class Tambers {
    /// A place's answer: its inputs (the tint it resolves by folded in, on
    /// every side, as an accent above moves it), that tint resolved on each
    /// of the eight sides (static colours, which a provider off the main
    /// thread may read), and the colours asked of it since the inputs last
    /// moved.
    struct Answer {
        var inputs: String
        var tints: [PlatformColor?]
        var colors: [String: BatchValue?]
    }
    private var answers: [UInt32: Answer] = [:]
    static let lock = NSLock()
    nonisolated(unsafe) private static var bySession: [ObjectIdentifier: (session: Weak<ExactSession>, tambers: Tambers)] = [:]
    /// The places that read a colour, for redrawing; and the observers,
    /// each with the inputs it last heard. Main thread.
    nonisolated(unsafe) static var places: [Weak<ExactTamber.Place>] = []
    nonisolated(unsafe) private static var observers: [(place: ExactTamber.Place, seen: String?, observation: Weak<ExactTamberObservation>)] = []

    func answer(_ view: UInt32) -> Answer? { Tambers.lock.withLock { answers[view] } }
    func keep(_ view: UInt32, _ answer: Answer?) { Tambers.lock.withLock { answers[view] = answer } }

    static func of(_ session: ExactSession) -> Tambers {
        let key = ObjectIdentifier(session)
        return lock.withLock {
            if let entry = bySession[key], entry.session.value === session { return entry.tambers }
            bySession = bySession.filter { $0.value.session.value != nil }
            let tambers = Tambers()
            bySession[key] = (Weak(session), tambers)
            return tambers
        }
    }

    /// The binding core's answer for `view` (0: the root) in `session`,
    /// with the tint a row naming it resolves by there. Main thread.
    static func ask(_ session: ExactSession, view: UInt32, names: [String]) -> Answer? {
        guard let answer = session.runtime.tamber(view: view, names: names) else { return nil }
        let tints = sides(of: ExactTamber.tint(view == 0 ? nil : session.presenter.views[view], session: session))
        let said = tints.map { $0.map { ExactTamber.extended($0).map { String($0) }.joined(separator: ",") } ?? "" }.joined(separator: " ")
        return Answer(inputs: answer.inputs + " tint " + said, tints: tints, colors: answer.colors)
    }

    /// `tint` resolved for each of the eight sides a view draws in (bit 0
    /// dark, bit 1 more contrast, bit 2 elevated), on the main thread: a
    /// dynamic tint reads its node's style, which a provider off it must not.
    static func sides(of tint: PlatformColor?) -> [PlatformColor?] {
        guard let tint else { return Array(repeating: nil, count: 8) }
        return (0..<8).map { side in
            #if os(macOS)
            let names: [NSAppearance.Name] = [.aqua, .darkAqua, .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua]
            var out: NSColor?
            NSAppearance(named: names[side & 3])?.performAsCurrentDrawingAppearance { out = tint.usingColorSpace(.extendedSRGB) }
            return out
            #else
            let traits = UITraitCollection { t in
                t.userInterfaceStyle = side & 1 != 0 ? .dark : .light
                t.accessibilityContrast = side & 2 != 0 ? .high : .normal
                #if !os(tvOS)
                t.userInterfaceLevel = side & 4 != 0 ? .elevated : .base
                #endif
            }
            return tint.resolvedColor(with: traits)
            #endif
        }
    }

    static func observe(_ place: ExactTamber.Place, _ observation: ExactTamberObservation) {
        observers.append((place, heard(place), Weak(observation)))
    }

    /// The inputs `place` reads now, asked when not kept; `nil` unmounted.
    private static func heard(_ place: ExactTamber.Place) -> String? {
        guard let (session, view) = place.found else { return nil }
        let tambers = of(session)
        if let known = tambers.answer(view) { return known.inputs }
        guard let answer = ask(session, view: view, names: []) else { return nil }
        tambers.keep(view, answer)
        return answer.inputs
    }

    /// After `session` applied a batch: each place whose inputs moved is
    /// asked again for every name it kept, its views are redrawn, and the
    /// observers of a place whose inputs moved, here or by a move of the
    /// view, hear it.
    static func moved(_ session: ExactSession) {
        // A native module's bindings ask again themselves (ExactTamber.swift).
        session.natives.tamberMoved()
        // A session with no answers kept may still have observers (first
        // registered unmounted), which are asked below.
        let tambers = lock.withLock({ bySession[ObjectIdentifier(session)] }).flatMap({ $0.session.value === session ? $0.tambers : nil })
        var moved: Set<UInt32> = []
        if let tambers {
            for (view, known) in lock.withLock({ tambers.answers }) {
                // A place gone (its node retired) is forgotten.
                guard let now = ask(session, view: view, names: []) else { tambers.keep(view, nil); continue }
                guard now.inputs != known.inputs else { continue }
                moved.insert(view)
                tambers.keep(view, known.colors.isEmpty ? now : ask(session, view: view, names: Array(known.colors.keys)) ?? now)
            }
        }
        places.removeAll { $0.value?.view == nil }
        for case let place? in places.map(\.value) {
            guard let (s, view) = place.found, s === session, let v = place.view else { continue }
            let at = (ObjectIdentifier(s), view)
            // Its node's answer moved, or it was moved to another node.
            guard moved.contains(view) || place.drawnAt.map({ $0 != at }) ?? true else { continue }
            place.drawnAt = at
            redraw(v)
        }
        observers.removeAll { $0.observation.value?.body == nil }
        for i in observers.indices {
            guard let (s, _) = observers[i].place.found, s === session else { continue }
            let now = heard(observers[i].place)
            defer { observers[i].seen = now }
            // Observed unmounted (`seen` none), the first batch it is found
            // in moves it off the fallback.
            if let now, observers[i].seen != now { observers[i].observation.value?.body?() }
        }
    }

    /// `view` and what it holds, drawn and laid out again; on UIKit their
    /// traits move too, which resolves their dynamic colours again.
    static func redraw(_ view: ExactTamberView) {
        #if os(macOS)
        func walk(_ v: NSView) { v.needsDisplay = true; v.needsLayout = true; v.subviews.forEach(walk) }
        walk(view)
        #else
        view.traitOverrides[TamberMoves.self] = view.traitCollection[TamberMoves.self] &+ 1
        func walk(_ v: UIView) { v.setNeedsDisplay(); v.setNeedsLayout(); v.subviews.forEach(walk) }
        walk(view)
        #endif
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
