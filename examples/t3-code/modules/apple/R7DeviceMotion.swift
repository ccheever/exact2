#if os(macOS)
import Foundation
import simd

/// Lane r7-device: the 3D phone's rotation and camera dynamics (MIT reference, see LICENSE-T3:
/// packages/client-runtime/src/device/deviceMotion.ts, deviceFraming.ts, deviceViewSnap.ts),
/// ported with three.js's quaternion conventions (`a.multiply(b)` = a·b, `premultiply` = b·a).
enum R7Quat {
    static let identity = simd_quatd(ix: 0, iy: 0, iz: 0, r: 1)
    /// rotationVector: the shortest rotation vector (q and -q are one orientation).
    static func log(_ rotation: simd_quatd) -> SIMD3<Double> {
        var q = rotation.normalized
        if q.real < 0 { q = simd_quatd(ix: -q.imag.x, iy: -q.imag.y, iz: -q.imag.z, r: -q.real) }
        let length = simd_length(q.imag)
        return length < 1e-8 ? .zero : q.imag * (2 * atan2(length, q.real) / length)
    }
    static func exp(_ vector: SIMD3<Double>) -> simd_quatd {
        let angle = simd_length(vector)
        return angle < 1e-8 ? identity : simd_quatd(angle: angle, axis: vector / angle)
    }
    static func angle(_ a: simd_quatd, _ b: simd_quatd) -> Double { 2 * acos(min(1, abs(simd_dot(a.vector, b.vector)))) }
    static func clamp(_ v: SIMD3<Double>, _ limit: Double) -> SIMD3<Double> {
        let length = simd_length(v)
        return length > limit ? v * (limit / length) : v
    }
    /// three.js Quaternion.slerp (shortest path).
    static func slerp(_ a: simd_quatd, _ b: simd_quatd, _ t: Double) -> simd_quatd { simd_slerp(a, b, t) }

    /// nearestDeviceView with the 3D view's one snap: rest, yawing up to ±60°.
    static func nearestView(_ rotation: simd_quatd, yawLimit: Double = .pi / 3) -> simd_quatd {
        let turn = 2 * atan2(rotation.imag.y, rotation.real)
        let yaw = max(-yawLimit, min(yawLimit, atan2(sin(turn), cos(turn))))
        return simd_quatd(angle: yaw, axis: SIMD3(0, 1, 0))
    }
}

/// createDeviceMotion: a spring target follows the gesture; release latches the nearest view.
final class R7DeviceMotion {
    private static let gain = 0.006, response = 0.5, damping = 0.78
    private static let frequency = 2 * Double.pi / response, decay = frequency * damping, stepSeconds = 1.0 / 120
    private static let flickThreshold = 4.5, flickLimit = 22.0, spinDecay = 1.1, settleFrequency = 2.0

    private(set) var rotation = R7Quat.identity
    private var target = R7Quat.identity
    private var velocity = SIMD3<Double>.zero
    private var gestureVelocity = SIMD3<Double>.zero
    private var spring: (at: Double, rotation: simd_quatd, velocity: SIMD3<Double>, steps: Int)?
    private var spin: (at: Double, rotation: simd_quatd, velocity: SIMD3<Double>, correction: SIMD3<Double>)?
    private var drag: (start: simd_quatd, rest: simd_quatd, x: Double, y: Double, error: SIMD3<Double>, at: Double)?
    private var held = false, interruptedDrag = false
    private var lastInput = -Double.infinity
    let choose: (simd_quatd) -> simd_quatd

    init(choose: @escaping (simd_quatd) -> simd_quatd = { R7Quat.nearestView($0) }) { self.choose = choose }

    private func beginSpring(_ next: simd_quatd, _ now: Double) {
        target = next.normalized
        spring = (now, rotation, velocity, 0)
    }

    private func release(_ now: Double) {
        guard let current = drag else { return }
        if now - lastInput > 100 { gestureVelocity = .zero }
        let prediction = R7Quat.exp(gestureVelocity * 0.085) * target
        velocity = R7Quat.clamp(velocity * 0.2 + gestureVelocity * 0.8, Self.flickLimit)
        let moved = current.x != 0 || current.y != 0
        drag = nil
        if moved, simd_length(velocity) >= Self.flickThreshold {
            let projected = R7Quat.exp(velocity * (1 / Self.spinDecay)) * rotation
            target = choose(projected).normalized
            spin = (now, rotation, velocity, R7Quat.log(target * projected.inverse))
            return
        }
        beginSpring(moved ? choose(prediction) : current.rest, now)
    }

    @discardableResult
    func advance(_ now: Double, reduced: Bool = false) -> Bool {
        if held || !now.isFinite { return false }
        if let spin {
            let seconds = max(0, (now - spin.at) / 1000)
            let coast = Foundation.exp(-Self.spinDecay * seconds), settle = Foundation.exp(-Self.settleFrequency * seconds)
            let correction = R7Quat.exp(spin.correction * (1 - (1 + Self.settleFrequency * seconds) * settle))
            rotation = (correction * (R7Quat.exp(spin.velocity * ((1 - coast) / Self.spinDecay)) * spin.rotation)).normalized
            velocity = correction.act(spin.velocity * coast) + spin.correction * (Self.settleFrequency * Self.settleFrequency * seconds * settle)
            if reduced || (R7Quat.angle(rotation, target) < 0.01 && simd_length(velocity) < 0.08) {
                self.spin = nil; rotation = target; velocity = .zero
            }
            return true
        }
        if var current = drag {
            let seconds = min(0.05, max(0, (now - current.at) / 1000))
            current.at = now
            let steps = Int(ceil(seconds * 120)), dt = steps > 0 ? seconds / Double(steps) : 0
            for _ in 0..<steps {
                var error = R7Quat.log(target * rotation.inverse)
                if simd_length_squared(error) > 1e-10 {
                    let axis = simd_normalize(error)
                    let turns = ((simd_dot(current.error, axis) - simd_length(error)) / (2 * .pi)).rounded()
                    error += axis * (turns * 2 * .pi)
                }
                current.error = error
                let omega = 2 * Double.pi / 0.68
                let acceleration = R7Quat.clamp(error, 1.2) * (omega * omega) - velocity * (2 * 1.12 * omega)
                velocity = R7Quat.clamp(velocity + acceleration * dt, 9)
                rotation = (R7Quat.exp(velocity * dt) * rotation).normalized
            }
            drag = current
            if reduced { rotation = target; velocity = .zero }
            return steps > 0 || reduced
        }
        guard var current = spring else { return false }
        let seconds = max(0, (now - current.at) / 1000)
        let steps = min(240, Int(floor(seconds / Self.stepSeconds)))
        func integrate(_ q: inout simd_quatd, _ speed: inout SIMD3<Double>, _ dt: Double) {
            let error = R7Quat.log(target * q.inverse)
            let acceleration = error * (Self.frequency * Self.frequency) - speed * (2 * Self.decay)
            speed = R7Quat.clamp(speed + acceleration * dt, 9)
            q = (R7Quat.exp(speed * dt) * q).normalized
        }
        while current.steps < steps { integrate(&current.rotation, &current.velocity, Self.stepSeconds); current.steps += 1 }
        var nextRotation = current.rotation, nextVelocity = current.velocity
        integrate(&nextRotation, &nextVelocity, Self.stepSeconds)
        let fraction = min(1, (seconds - Double(steps) * Self.stepSeconds) / Self.stepSeconds)
        rotation = R7Quat.slerp(current.rotation, nextRotation, fraction)
        velocity = current.velocity + (nextVelocity - current.velocity) * fraction
        spring = current
        if reduced || steps == 240 || (R7Quat.angle(rotation, target) < 0.0005 && simd_length(velocity) < 0.005) {
            rotation = target; velocity = .zero; spring = nil
        }
        return true
    }

    private func beginDrag(_ now: Double) {
        if held || drag != nil { return }
        advance(now)
        drag = (rotation, target, 0, 0, .zero, now)
        target = rotation
        gestureVelocity = .zero
        lastInput = -.infinity
        spring = nil; spin = nil
    }

    func setPose(_ next: simd_quatd, _ now: Double, immediate: Bool = false) {
        advance(now)
        drag = nil; spin = nil
        beginSpring(next, now)
        if immediate { rotation = target; velocity = .zero; spring = nil }
    }

    func dragActive(_ active: Bool, _ now: Double) {
        if active {
            if drag != nil || spin != nil { advance(now); drag = nil; spin = nil }
            beginDrag(now)
        } else { advance(now); release(now) }
    }

    /// Deltas are points, independent of panel size and event partitioning.
    func orbit(_ x: Double, _ y: Double, _ now: Double) {
        if held || ![x, y, now].allSatisfy(\.isFinite) || (x == 0 && y == 0) { return }
        advance(now)
        beginDrag(now)
        guard var current = drag else { return }
        let previous = target
        current.x += x; current.y += y
        drag = current
        target = (R7Quat.exp(SIMD3(current.y * Self.gain, current.x * Self.gain, 0)) * current.start).normalized
        let seconds = (now - lastInput) / 1000
        gestureVelocity = R7Quat.log(target * previous.inverse)
        if seconds > 0 && seconds < 0.1 { gestureVelocity = R7Quat.clamp(gestureVelocity / seconds, Self.flickLimit) } else { gestureVelocity = .zero }
        lastInput = now
    }

    /// Captured device touches freeze projection, independently of orbit dragging.
    func hold(_ active: Bool, _ now: Double) {
        if held == active { return }
        if active {
            interruptedDrag = drag != nil || spin != nil
            drag = nil; spin = nil; spring = nil; velocity = .zero
        }
        held = active
        if !active {
            if interruptedDrag { beginSpring(choose(rotation), now) } else if R7Quat.angle(rotation, target) > 0.0005 { beginSpring(target, now) }
            interruptedDrag = false
        }
    }

    var needsFrame: Bool {
        !held && (spring != nil || spin != nil || (drag != nil && (R7Quat.angle(rotation, target) > 0.0005 || simd_length(velocity) > 0.005)))
    }

    func reset(_ next: simd_quatd, _ now: Double) { advance(now); drag = nil; spin = nil; beginSpring(next, now) }
}

/// createDeviceFraming: fit the assembly with its own critically damped centre and distance.
final class R7DeviceFraming {
    private(set) var center = SIMD3<Double>.zero
    private var target = SIMD3<Double>.zero, velocity = SIMD3<Double>.zero
    private var bounds: (min: SIMD3<Double>, max: SIMD3<Double>) = (.zero, .zero)
    private var distanceValue = 1.0, targetDistance = 1.0, distanceVelocity = 0.0
    private var tanX = 1.0, tanY = 1.0, at = 0.0
    private var initialized = false, moving = false, held = false

    @discardableResult
    func advance(_ now: Double, immediate: Bool = false) -> Bool {
        if held { return false }
        let dt = max(0, (now - at) / 1000)
        at = now
        if !moving { return false }
        let omega = 14.0, decay = Foundation.exp(-omega * dt)
        let error = center - target
        let coefficient = velocity + error * omega
        center = (error + coefficient * dt) * decay + target
        velocity = (velocity - coefficient * (omega * dt)) * decay
        let distanceError = distanceValue - targetDistance
        let distanceCoefficient = distanceVelocity + omega * distanceError
        distanceValue = targetDistance + (distanceError + distanceCoefficient * dt) * decay
        distanceVelocity = (distanceVelocity - omega * distanceCoefficient * dt) * decay
        moving = simd_distance(center, target) > 0.0005 || simd_length(velocity) > 0.005 || abs(distanceValue - targetDistance) > 0.0005 || abs(distanceVelocity) > 0.005
        if immediate || !moving { center = target; distanceValue = targetDistance; velocity = .zero; distanceVelocity = 0; moving = false }
        return true
    }

    /// `vertical` is half the vertical field of view in radians.
    func setBounds(min lower: SIMD3<Double>, max upper: SIMD3<Double>, vertical: Double, aspect: Double, now: Double, immediate: Bool = false) {
        guard upper.x >= lower.x, upper.y >= lower.y else { return }
        advance(now)
        bounds = (lower, upper)
        tanY = tan(vertical); tanX = tanY * aspect
        target = (lower + upper) / 2; target.z = 0
        let size = upper - lower
        targetDistance = Swift.max(1, Swift.max(size.x / tanX, size.y / tanY) * 0.565 + upper.z)
        moving = true
        advance(now, immediate: immediate || !initialized)
        initialized = true
    }

    /// Spring lag must never clip the fitted assembly.
    var distance: Double {
        let clearance = [abs(bounds.min.x - center.x) / tanX, abs(bounds.max.x - center.x) / tanX, abs(bounds.min.y - center.y) / tanY, abs(bounds.max.y - center.y) / tanY].max() ?? 0
        return Swift.max(distanceValue, clearance * 1.02 + bounds.max.z, 1)
    }

    func hold(_ active: Bool, _ now: Double) { held = active; at = now }
    var needsFrame: Bool { moving && !held }
}
#endif
