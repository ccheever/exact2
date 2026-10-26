// @ref LLP 1133.029 D5. `orientationlock`: the runner decides which
// elements hold the session's lock and intersects their types
// (`exact_runner::orientation`); a batch says when that changes, and the
// presenter resolves `natural` and `current` against its scene, bounds the
// types by what the window can take, and has the root controller and every
// controller it presents answer UIKit with the result. On iOS 26 and later
// `current` is `prefersInterfaceOrientationLocked`; before, it is the mask of
// the scene's orientation when the lock began. An iPad applies no type:
// "multitasking apps must support all orientations", so UIKit does not ask
// (`supportedInterfaceOrientations`); `current` is the lock that works
// there. macOS and tvOS have no interface orientation and journal once.
import Foundation
import ExactCore
#if os(iOS)
import UIKit
#endif

/// The session's orientation lock as its presenter holds it.
package struct OrientationLock {
    /// The runner's last word.
    var request = OrientationRequest()
    /// The scene's orientation when a lock with `current` began, as a bit
    /// (`OrientationRequest.types`); 0 when none is captured.
    var captured = 0
    /// What the root and every presented controller answer: the allowed
    /// orientations as bits, 0 for UIKit's own answer.
    var mask = 0
    /// What they answer for `prefersInterfaceOrientationLocked`.
    var locked = false
    /// Why the lock does not hold as asked, for the readback.
    var why: String?
    /// The host's resolution found a conflict.
    var conflict = false
    /// The last line journaled, said once until it changes.
    var said: String?

    /// The four orientations' names, in the bits' order.
    static let names = ["portrait-primary", "portrait-secondary", "landscape-primary", "landscape-secondary"]
    static func named(_ bits: Int) -> [String] { names.indices.filter { bits & (1 << $0) != 0 }.map { names[$0] } }
}

#if os(iOS)
extension OrientationLock {
    /// A `-primary` is the device's own way up, as `phoneOrientations` maps
    /// it into Info.plist (host/apple/devices.mjs): landscape-primary is
    /// UIKit's landscape right.
    private static let masks: [UIInterfaceOrientationMask] = [.portrait, .portraitUpsideDown, .landscapeRight, .landscapeLeft]
    static func mask(_ bits: Int) -> UIInterfaceOrientationMask {
        masks.indices.reduce(into: []) { m, i in if bits & (1 << i) != 0 { m.insert(masks[i]) } }
    }
    static func bits(_ mask: UIInterfaceOrientationMask) -> Int {
        masks.indices.reduce(0) { b, i in mask.contains(masks[i]) ? b | (1 << i) : b }
    }
    static func bit(_ orientation: UIInterfaceOrientation) -> Int {
        switch orientation {
        case .portrait: 1
        case .portraitUpsideDown: 2
        case .landscapeRight: 4
        case .landscapeLeft: 8
        default: 0
        }
    }
    /// What a controller returns from `supportedInterfaceOrientations`.
    package var supported: UIInterfaceOrientationMask? { mask == 0 ? nil : Self.mask(mask) }
}

extension Presenter {
    /// The batch's word on the orientation lock, when it says one.
    func applyOrientationLock(_ batch: Batch) {
        guard let request = batch.orientation else { return }
        // `current` is captured once a hold: a release and a new lock capture again.
        if !request.current { orientationLock.captured = 0 }
        orientationLock.request = request
        resolveOrientationLock()
    }

    /// Resolve the request against the scene, have every controller answer
    /// it, turn the scene when it is outside, and report what was asked.
    func resolveOrientationLock() {
        var lock = orientationLock
        let request = lock.request
        let window = viewport.window, scene = window?.windowScene
        var mask = 0, locked = false, line: String?, why: String?, conflict = false
        if request.types == 0 {
            lock.captured = 0
        } else if session?.hatchesOwnWindow != true {
            (line, why) = ("orientationlock: the window is the embedder's", "the window is the embedder's")
        } else if let window, let scene {
            let now = OrientationLock.bit(scene.effectiveGeometry.interfaceOrientation)
            if request.current, lock.captured == 0 { lock.captured = now }
            let captured = request.current ? lock.captured : 15
            if scene.traitCollection.userInterfaceIdiom == .pad {
                // iPadOS asks no controller for a type; `current` alone holds, in full screen.
                if request.current, #available(iOS 26, *) { locked = true }
                else { why = request.current ? "an iPad before iPadOS 26 has no lock" : "an iPad applies no type" }
            } else {
                let allowed = OrientationLock.bits(UIApplication.shared.supportedInterfaceOrientations(for: window))
                let types = request.types & (request.natural ? 1 : 15)
                if types & captured == 0 {
                    conflict = true
                    line = "orientationlock: conflict: \(request.holders.joined(separator: ", ")); none held"
                    why = "conflict"
                } else if types & captured & allowed == 0 {
                    line = "orientationlock: \(request.holders.joined(separator: ", ")): this device does not show it"
                    why = "this device does not show it"
                } else if #available(iOS 26, *), request.current {
                    locked = true
                    mask = types & allowed == allowed ? 0 : types & allowed
                } else {
                    mask = types & captured & allowed == allowed ? 0 : types & captured & allowed
                }
            }
        } else {
            why = "no window"
        }
        let changed = mask != lock.mask || locked != lock.locked
        (lock.mask, lock.locked, lock.why, lock.conflict) = (mask, locked, why, conflict)
        if line != lock.said, let line { session?.log(line) }
        lock.said = line
        orientationLock = lock
        if changed, let root = window?.rootViewController {
            var controller: UIViewController? = root
            while let c = controller {
                c.setNeedsUpdateOfSupportedInterfaceOrientations()
                if #available(iOS 26, *) { c.setNeedsUpdateOfPrefersInterfaceOrientationLocked() }
                controller = c.presentedViewController
            }
        }
        if mask != 0, let scene, OrientationLock.bit(scene.effectiveGeometry.interfaceOrientation) & mask == 0 {
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: OrientationLock.mask(mask))) { [weak self] error in
                self?.session?.log("orientationlock: the turn was refused: \(error.localizedDescription)")
            }
        }
        reportOrientationLock()
    }

    /// `state.orientationlock.applied` (LLP 1133.029 §8): the orientations
    /// asked for, whether `current` asked the system to lock, whether it
    /// holds (iOS 26 and later), the scene's orientation, and why not.
    func reportOrientationLock() {
        let lock = orientationLock
        let scene = viewport.window?.windowScene
        let now = scene.map { OrientationLock.bit($0.effectiveGeometry.interfaceOrientation) } ?? 0
        var system: Bool?
        if #available(iOS 26, *) { system = scene?.effectiveGeometry.isInterfaceOrientationLocked }
        let held = lock.request.types != 0 && lock.why == nil
            && (lock.mask == 0 || lock.mask & now != 0) && (!lock.locked || system == true)
        let applied: [String: Any] = [
            "asked": lock.mask == 0 ? NSNull() : OrientationLock.named(lock.mask) as Any,
            "current": lock.locked, "locked": system.map { $0 as Any } ?? NSNull(),
            "orientation": OrientationLock.named(now).first ?? "unknown", "held": held,
            "why": lock.why.map { $0 as Any } ?? NSNull()]
        guard let data = try? JSONSerialization.data(withJSONObject: applied, options: .sortedKeys) else { return }
        session?.runtime.orientationApplied(String(decoding: data, as: UTF8.self), conflict: lock.conflict)
    }

    /// A restart lets the lock go; the new host's first batch says it again.
    func releaseOrientationLock() {
        guard orientationLock.request.types != 0 else { return }
        orientationLock.request = OrientationRequest()
        resolveOrientationLock()
    }
}
#else
extension Presenter {
    /// This host has no interface orientation: the first lock held is journaled.
    func applyOrientationLock(_ batch: Batch) {
        guard let request = batch.orientation else { return }
        orientationLock.request = request
        guard request.types != 0, orientationLock.said == nil else { return }
        orientationLock.said = "orientationlock: this host has no screen orientation"
        session?.log(orientationLock.said!)
    }
    func reportOrientationLock() {}
    func releaseOrientationLock() { orientationLock.request = OrientationRequest() }
}
#endif
