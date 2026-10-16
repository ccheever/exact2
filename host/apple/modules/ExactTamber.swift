// The binding core in a native module's library (LLP 1121 D12, stage 5):
// `ExactTamber(view).color(.surfaceCard)` on a view the module made or a
// hatched node's view, as ExactKit's own `ExactTamber` is in the process.
// The host answers over the hatches' host table (`tamber`, at 96), resolving
// each colour for the eight sides a view draws in, under the view's tint, as
// extended sRGB (the library cannot read a row's value); the colour is a
// dynamic provider over the view's traits. After each batch the host calls
// the module's `tamber` entry (at 216): every place asks whether what it
// read moved (the inputs fold in the tint), and each that moved asks again
// for its names, redraws its view and tells its observers.
#if os(macOS)
import AppKit
public typealias ExactTamberColor = NSColor
#else
import UIKit
public typealias ExactTamberColor = UIColor
#endif

/// A design token by its name (`contract tamber gen swift` writes each as a
/// static member: `.surfaceCard`).
public struct ExactTamberToken: Hashable, Sendable {
    public let name: String
    public init(_ name: String) { self.name = name }
}

/// The design's tokens where `view` is: its element's place in its session.
public struct ExactTamber {
    private let place: TamberPlace?

    /// The place of `view`, a view this module made or one inside it.
    public init(_ view: ExactPlatformView) {
        var at: ExactPlatformView? = view
        while let v = at, Tambers.made[ObjectIdentifier(v)]?.view.value !== v { at = v.superview }
        place = at.flatMap { v in Tambers.made[ObjectIdentifier(v)]?.module.value.map { Tambers.place(v, module: $0) } }
    }

    /// `token`'s colour here, dynamic over the traits it is drawn with;
    /// `fallback` where the session's plan lacks it, or before the host can
    /// answer (its hatches connect after the first frame). After a batch
    /// moves what it read, the module's view is redrawn and laid out again,
    /// and on UIKit its traits change, so a UIKit property holding the
    /// colour, or a `draw` or `layoutSubviews` that resolved it, reads the
    /// new answer; a colour copied out of it (a layer's `cgColor`) is a
    /// value: set it again in `observe`.
    public func color(_ token: ExactTamberToken, fallback: ExactTamberColor = .clear) -> ExactTamberColor {
        let place = place
        // Asked now, on the main thread, so a provider called off it (a
        // renderer's) finds the answer kept: the host answers on the main
        // thread only.
        if Thread.isMainThread { _ = place?.sides(token.name) }
        let side = { (side: Int) -> ExactTamberColor? in
            guard let c = place?.sides(token.name)?[side], c.count == 4 else { return nil }
            #if os(macOS)
            return NSColor(colorSpace: .extendedSRGB, components: c.map { CGFloat($0) }, count: 4)
            #else
            return UIColor(red: c[0], green: c[1], blue: c[2], alpha: c[3])
            #endif
        }
        #if os(macOS)
        return NSColor(name: nil) { appearance in
            let match = appearance.bestMatch(from: [.aqua, .darkAqua, .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua])
            let dark = match == .darkAqua || match == .accessibilityHighContrastDarkAqua
            let contrast = match == .accessibilityHighContrastAqua || match == .accessibilityHighContrastDarkAqua
            return side((dark ? 1 : 0) | (contrast ? 2 : 0)) ?? fallback
        }
        #else
        return UIColor { traits in
            // Read, so UIKit's trait tracking redraws what resolved it when a batch moves it.
            _ = traits[TamberMoves.self]
            var at = (traits.userInterfaceStyle == .dark ? 1 : 0) | (traits.accessibilityContrast == .high ? 2 : 0)
            #if !os(tvOS)
            at |= traits.userInterfaceLevel == .elevated ? 4 : 0
            #endif
            return side(at) ?? fallback
        }
        #endif
    }

    /// `body` after each batch that moved what this place's tokens read (a
    /// mode, a pin above, a live edit, the scheme, the root font size); stop
    /// it with the observation's `stop()`. Main thread.
    public func observe(_ body: @escaping () -> Void) -> ExactTamberObservation {
        let observation = ExactTamberObservation(body)
        if let place {
            _ = place.sides(nil)
            Tambers.observers.append((place, Weak(observation)))
        }
        return observation
    }
}

/// An `observe`'s handle: `stop()` ends it, as dropping the last reference does.
public final class ExactTamberObservation {
    fileprivate var body: (() -> Void)?
    fileprivate init(_ body: @escaping () -> Void) { self.body = body }
    public func stop() { body = nil }
}

/// A view the module made, and what its session last answered for it: one
/// per view, which every binding on it shares. Its answer is read by
/// providers off the main thread, so it is kept under `Tambers.lock`.
final class TamberPlace {
    weak var view: ExactPlatformView?
    weak var module: ExactModule?
    /// The inputs the answer read, and each name's eight sides.
    private var known: (inputs: String, colors: [String: [[Double]]?])?
    /// The host's batch count this place last asked its inputs at.
    private var checked = -1
    init(view: ExactPlatformView, module: ExactModule) { self.view = view; self.module = module }

    /// `name`'s eight sides (`nil`: none, or nothing to answer yet). On the
    /// main thread, after a batch it first asks whether its inputs moved;
    /// off it, what was kept.
    func sides(_ name: String?) -> [[Double]]? {
        guard Thread.isMainThread else { return name.flatMap { n in Tambers.lock.withLock { known?.colors[n] ?? nil } } }
        if checked != Tambers.batches { _ = refresh() }
        guard let name else { return nil }
        let kept = Tambers.lock.withLock { known }
        if let kept, let colors = kept.colors[name] { return colors }
        guard let answer = ask(Array(Set(kept?.colors.keys.map { $0 } ?? []).union([name]))) else { return nil }
        Tambers.lock.withLock { known = answer }
        return answer.colors[name] ?? nil
    }

    /// Main thread: asks whether the inputs moved since the answer kept, and
    /// if so asks again for every name kept, so a provider off the main
    /// thread never finds the answer emptied; whether they moved.
    func refresh() -> Bool {
        checked = Tambers.batches
        guard let now = ask([]) else { return false }
        let kept = Tambers.lock.withLock { known }
        guard let kept else { Tambers.lock.withLock { known = now }; return false }
        guard kept.inputs != now.inputs else { return false }
        let again = kept.colors.isEmpty ? now : ask(Array(kept.colors.keys)) ?? now
        Tambers.lock.withLock { known = again }
        return true
    }

    private func ask(_ names: [String]) -> (inputs: String, colors: [String: [[Double]]?])? {
        guard let view, let hatches = module?.hatches, let tamber = hatches.tamberFn,
              let request = try? JSONSerialization.data(withJSONObject: ["names": names]) else { return nil }
        let raw = Unmanaged.passUnretained(view).toOpaque()
        let needed = request.withUnsafeBytes { r in
            tamber(hatches.host, raw, r.bindMemory(to: UInt8.self).baseAddress, UInt32(request.count), nil, 0)
        }
        guard needed > 0 else { return nil }
        var out = [UInt8](repeating: 0, count: Int(needed))
        let written = request.withUnsafeBytes { r in
            out.withUnsafeMutableBufferPointer { o in
                tamber(hatches.host, raw, r.bindMemory(to: UInt8.self).baseAddress, UInt32(request.count), o.baseAddress, needed)
            }
        }
        guard written == needed, let json = try? JSONSerialization.jsonObject(with: Data(out)) as? [String: Any],
              let inputs = json["inputs"] as? String else { return nil }
        let colors = (json["colors"] as? [String: Any] ?? [:]).mapValues { $0 as? [[Double]] }
        return (inputs, colors)
    }
}

#if !os(macOS)
/// A trait each moved place's view takes a new value of, so UIKit resolves
/// its dynamic colours again (the providers read it).
struct TamberMoves: UITraitDefinition {
    static let defaultValue = 0
}
#endif

/// The library's binding state: each view the module made, its place, and
/// the observers. Main thread, but for `lock`.
enum Tambers {
    static let lock = NSLock()
    /// The batches the host has said were applied: a place asks again when it moves.
    nonisolated(unsafe) static var batches = 0
    nonisolated(unsafe) static var made: [ObjectIdentifier: (view: Weak<ExactPlatformView>, module: Weak<ExactModule>)] = [:]
    nonisolated(unsafe) static var places: [ObjectIdentifier: TamberPlace] = [:]
    nonisolated(unsafe) static var observers: [(place: TamberPlace, observation: Weak<ExactTamberObservation>)] = []

    /// `view`, just made by `module` for an element: a place a binding finds.
    static func made(_ view: ExactPlatformView, by module: ExactModule) {
        made = made.filter { $0.value.view.value != nil }
        made[ObjectIdentifier(view)] = (Weak(view), Weak(module))
    }

    /// The place of `view`, a view `module` made, shared by its bindings.
    static func place(_ view: ExactPlatformView, module: ExactModule) -> TamberPlace {
        let id = ObjectIdentifier(view)
        if let place = places[id], place.view === view { return place }
        places = places.filter { $0.value.view != nil }
        let place = TamberPlace(view: view, module: module)
        places[id] = place
        return place
    }

    /// The host's `tamber` entry, after each batch: each place asks whether
    /// its inputs moved; each that moved asks again for what it kept, its
    /// view is redrawn, and its observers hear.
    static func moved() {
        batches += 1
        places = places.filter { $0.value.view != nil }
        observers.removeAll { $0.observation.value?.body == nil || $0.place.view == nil }
        var moved: Set<ObjectIdentifier> = []
        for (id, place) in places where place.refresh() {
            moved.insert(id)
            if let view = place.view { redraw(view) }
        }
        for (place, observation) in observers where place.view.map({ moved.contains(ObjectIdentifier($0)) }) ?? false {
            observation.value?.body?()
        }
    }

    /// `view` and what it holds, drawn and laid out again; on UIKit their
    /// traits move too, which resolves their dynamic colours again.
    static func redraw(_ view: ExactPlatformView) {
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
