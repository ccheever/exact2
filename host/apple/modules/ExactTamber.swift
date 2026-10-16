// The binding core in a native module's library (LLP 1121 D12, stage 5):
// `ExactTamber(view).color(.surfaceCard)` on a view the module made, as
// ExactKit's own `ExactTamber` is in the process. The host answers over the
// hatches' host table (`tamber`, at 96), resolving each colour for the
// eight sides a view draws in, as the library cannot read a row's value; the
// colour is a dynamic provider over the view's traits. After a batch that
// moved what a binding reads, the host calls the module's `tamber` entry
// (at 216) and each observed place is asked again.
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
        place = at.flatMap { v in Tambers.made[ObjectIdentifier(v)]?.module.value.map { TamberPlace(view: v, module: $0) } }
    }

    /// `token`'s colour here, dynamic over the traits it is drawn with;
    /// `fallback` where the session's plan lacks it, or before the host can
    /// answer (its hatches connect after the first frame).
    public func color(_ token: ExactTamberToken, fallback: ExactTamberColor = .clear) -> ExactTamberColor {
        let place = place
        let side = { (side: Int) -> ExactTamberColor? in
            guard let c = place?.sides(token.name)?[side], c.count == 4 else { return nil }
            return ExactTamberColor(red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
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
            _ = place.inputs()
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

/// A view the module made, and what its session last answered for it.
final class TamberPlace {
    weak var view: ExactPlatformView?
    weak var module: ExactModule?
    /// The inputs the answer read, and each name's eight sides.
    private var known: (inputs: String, colors: [String: [[Double]]?])?
    init(view: ExactPlatformView, module: ExactModule) { self.view = view; self.module = module }

    func sides(_ name: String) -> [[Double]]? {
        if let known, let colors = known.colors[name] { return colors }
        let names = Array(Set(known?.colors.keys.map { $0 } ?? []).union([name]))
        guard let answer = ask(names) else { return nil }
        known = answer
        return answer.colors[name] ?? nil
    }

    /// The inputs now, keeping what was asked when they have not moved;
    /// whether they moved.
    func inputs() -> Bool {
        guard let answer = ask([]) else { return false }
        let moved = known.map { $0.inputs != answer.inputs } ?? false
        if known == nil || moved { known = (answer.inputs, [:]) }
        return moved
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

/// The library's binding state: each view the module made, and the observers.
enum Tambers {
    nonisolated(unsafe) static var made: [ObjectIdentifier: (view: Weak<ExactPlatformView>, module: Weak<ExactModule>)] = [:]
    nonisolated(unsafe) static var observers: [(place: TamberPlace, observation: Weak<ExactTamberObservation>)] = []

    /// `view`, just made by `module` for an element: a place a binding finds.
    static func made(_ view: ExactPlatformView, by module: ExactModule) {
        made = made.filter { $0.value.view.value != nil }
        made[ObjectIdentifier(view)] = (Weak(view), Weak(module))
    }

    /// The host's `tamber` entry: each observed place whose inputs moved
    /// tells its observers.
    static func moved() {
        observers.removeAll { $0.observation.value?.body == nil || $0.place.view == nil }
        for (place, observation) in observers where place.inputs() { observation.value?.body?() }
    }
}

/// A weak reference, for a table that must not keep what it names alive.
struct Weak<T: AnyObject> {
    weak var value: T?
    init(_ value: T) { self.value = value }
}
