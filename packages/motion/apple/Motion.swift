// @exact/motion on Apple (LLP 1133.013 D2, D3, D4, D5, D8): Core Motion
// through one `CMMotionManager`, answered in the interface's shapes. The
// app's composed module calls it outside the agent, through `ExactPackage`'s
// shape; under the agent the runner's substitute answers and no manager is
// made, unless the drive binds it (`--bind motion`).
//
// - One manager serves every reader (D4), with four streams: the raw
//   accelerometer, gyroscope and magnetometer, and device motion for the
//   rest. Each runs at the fastest frequency its readers ask (60 Hz when one
//   names none; `deviceMotion` always 60 Hz, as WebKit) and device motion in
//   the strongest reference frame one needs (magnetic north for an absolute
//   orientation, else `xArbitraryZVertical`). Each reader group (a name with
//   one `frequency`, `referenceFrame` and `history`) keeps a ring of its own
//   thinned samples, numbered, answered from the cursor the runner passes as
//   `after` (D5).
// - Units and sign are the W3C's (D2): Core Motion's accelerations are in g
//   with gravity pointing down (face up reads z = −1), so each is multiplied
//   by −9.80665; rotation rates are rad/s and fields µT as Core Motion gives
//   them. An absolute orientation is turned from Core Motion's x-north frame
//   to the W3C's Earth frame (x east, y north, z up).
// - `referenceFrame: "screen"` turns x and y by the window scene's interface
//   orientation, as the web's `screen.orientation.angle` (Accelerometer §7.1).
// - The streams stop while the app is in the background and start again in
//   the foreground; each reader's next answer reads `interrupted` (D8).
// - A canvas input needs the context's surface input, which this host does
//   not give an implementation yet: `startSensorInput` is `NotSupportedError`.
// - Tier 3 (D10): `MotionModule.hatch` is given the manager once, when made.
// - The Mac has none of this (`CMMotionManager` is iOS's); its module answers
//   the status unavailable.
import CoreMotion
import ExactPackage
import Foundation
#if os(iOS)
import UIKit
#endif

#if os(iOS)
public final class MotionModule: ExactPackage {
    /// The app's access hatch (D10): given the manager once, when it is made.
    public static var hatch: ((CMMotionManager) -> Void)?

    private static let g = 9.80665
    private static let most = 1024

    /// The Core Motion stream a name reads.
    private enum Stream: CaseIterable { case accelerometer, gyroscope, magnetometer, deviceMotion }
    private static func stream(_ name: String, _ args: [String: Any]) -> Stream? {
        switch name {
        case "accelerometer": return .accelerometer
        case "gyroscope": return (args["apple"] as? [String: Any])?["biasCorrected"] as? Bool == true ? .deviceMotion : .gyroscope
        case "magnetometer": return .magnetometer
        case "linearAccelerationSensor", "gravitySensor", "absoluteOrientationSensor", "relativeOrientationSensor",
             "deviceMotion", "deviceOrientation": return .deviceMotion
        default: return nil
        }
    }

    private final class Group {
        let name: String, args: [String: Any], stream: Stream, period: Double
        var next = -Double.infinity
        var ring: [(sequence: Int, fields: [String: Any])] = []
        var sequence = 0
        var pauses: [Int] = []
        var waiting: [() -> Void] = []
        init(name: String, args: [String: Any], stream: Stream) {
            self.name = name; self.args = args; self.stream = stream
            let f = (args["frequency"] as? Double) ?? 0
            period = f > 0 ? 1000 / f : 0
        }
        var frequency: Double { name == "deviceMotion" ? 60 : (args["frequency"] as? Double).flatMap { $0 > 0 ? $0 : nil } ?? 60 }
        var history: Int { min(MotionModule.most, Int(args["history"] as? Double ?? 0)) }
    }

    private let log: (String) -> Void
    private let now: () -> Double
    private lazy var manager: CMMotionManager = {
        let m = CMMotionManager()
        Self.hatch?(m)
        return m
    }()
    /// Reader groups by name and key.
    private var groups: [String: Group] = [:]
    /// What each running stream was started with.
    private var running: [Stream: (frequency: Double, frame: CMAttitudeReferenceFrame?)] = [:]
    private var shown = true

    public init(context: ExactPackageContext) {
        log = context.log
        now = context.now
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            // Core Motion asks no permission for these sensors (D11).
            case "motionStatus", "requestMotionPermission": reply(status())
            case _ where Self.stream(name, args) != nil: read(name, args, reply)
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "motionStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    private func status() -> [String: Any] {
        let motion = manager.isDeviceMotionAvailable
        let north = motion && CMMotionManager.availableAttitudeReferenceFrames().contains(.xMagneticNorthZVertical)
        let sensors: [String: Any] = [
            "accelerometer": manager.isAccelerometerAvailable, "linearAcceleration": motion, "gravity": motion,
            "gyroscope": manager.isGyroAvailable, "magnetometer": manager.isMagnetometerAvailable,
            "absoluteOrientation": north, "relativeOrientation": motion, "deviceMotion": motion, "deviceOrientation": motion,
        ]
        return ["available": motion || manager.isAccelerometerAvailable, "state": "granted", "sensors": sensors,
                "apple": NSNull(), "android": NSNull()]
    }

    private func available(_ stream: Stream) -> Bool {
        switch stream {
        case .accelerometer: return manager.isAccelerometerAvailable
        case .gyroscope: return manager.isGyroAvailable
        case .magnetometer: return manager.isMagnetometerAvailable
        case .deviceMotion: return manager.isDeviceMotionAvailable
        }
    }

    // MARK: readers

    /// A reader's arguments without its cursor, and the key of its group.
    private static func asked(_ args: [String: Any]) -> ([String: Any], String) {
        var a = args
        a["after"] = nil
        let data = (try? JSONSerialization.data(withJSONObject: a, options: [.sortedKeys])) ?? Data()
        return (a, String(decoding: data, as: UTF8.self))
    }

    private func read(_ name: String, _ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let (asked, key) = Self.asked(args)
        guard let stream = Self.stream(name, asked), available(stream),
              name != "absoluteOrientationSensor" || status()["sensors"].flatMap({ ($0 as? [String: Any])?["absoluteOrientation"] as? Bool }) == true
        else { return reply(["error": "NotSupportedError"]) }
        let group = groups["\(name) \(key)"] ?? Group(name: name, args: asked, stream: stream)
        groups["\(name) \(key)"] = group
        restart()
        guard group.ring.isEmpty else { return reply(answer(group, args)) }
        guard shown else { return reply(["error": "NotReadableError"]) }
        // The first answer waits for the stream's first sample, a second at most.
        var answered = false
        let once: () -> Void = { [weak self] in
            guard !answered, let self else { return }
            answered = true
            reply(group.ring.isEmpty ? ["error": "NotReadableError"] : self.answer(group, args))
        }
        group.waiting.append(once)
        DispatchQueue.main.asyncAfter(deadline: .now() + 1, execute: once)
    }

    /// A reader's answer from its group's ring after `after` (D5).
    private func answer(_ group: Group, _ args: [String: Any]) -> [String: Any] {
        guard let newest = group.ring.last else { return ["error": "NotReadableError"] }
        var out = newest.fields
        out["sequence"] = newest.sequence
        out["samples"] = [[String: Any]]()
        out["dropped"] = 0
        out["interrupted"] = false
        if let after = (args["after"] as? Double).map(Int.init) {
            let fresh = group.ring.filter { $0.sequence > after }
            let kept = group.history > 0 ? Array(fresh.suffix(group.history)) : []
            out["samples"] = kept.map(\.fields)
            out["dropped"] = group.history > 0 ? max(0, newest.sequence - after - kept.count) : 0
            out["interrupted"] = group.pauses.contains { $0 >= after }
        }
        return out
    }

    public func release(_ name: String) {
        groups = groups.filter { $0.value.name != name }
        restart()
    }

    public func release(_ name: String, args: [String: Any]) {
        groups["\(name) \(Self.asked(args).1)"] = nil
        restart()
    }

    public func foreground(_ shown: Bool) {
        guard shown != self.shown else { return }
        self.shown = shown
        if !shown { for g in groups.values { g.pauses.append(g.sequence) } }
        restart()
    }

    // MARK: the streams

    /// Each stream its readers need, at their fastest frequency; the rest stopped.
    private func restart() {
        for stream in Stream.allCases {
            let readers = shown ? groups.values.filter { $0.stream == stream } : []
            guard let frequency = readers.map(\.frequency).max() else {
                if running.removeValue(forKey: stream) != nil { stop(stream) }
                continue
            }
            let frame: CMAttitudeReferenceFrame? = stream != .deviceMotion ? nil
                : readers.contains { $0.name == "absoluteOrientationSensor" || ($0.name == "deviceOrientation" && $0.args["absolute"] as? Bool == true) }
                ? .xMagneticNorthZVertical : .xArbitraryZVertical
            if let r = running[stream], r.frequency == frequency, r.frame == frame { continue }
            stop(stream)
            running[stream] = (frequency, frame)
            start(stream, frequency, frame)
            log("motion: \(stream) at \(frequency) Hz")
        }
    }

    private func stop(_ stream: Stream) {
        switch stream {
        case .accelerometer: manager.stopAccelerometerUpdates()
        case .gyroscope: manager.stopGyroUpdates()
        case .magnetometer: manager.stopMagnetometerUpdates()
        case .deviceMotion: manager.stopDeviceMotionUpdates()
        }
    }

    private func start(_ stream: Stream, _ frequency: Double, _ frame: CMAttitudeReferenceFrame?) {
        let interval = 1 / frequency
        switch stream {
        case .accelerometer:
            manager.accelerometerUpdateInterval = interval
            manager.startAccelerometerUpdates(to: .main) { [weak self] d, _ in
                guard let d else { return }
                let a = d.acceleration
                self?.take(.accelerometer, d.timestamp) { _ in [a.x, a.y, a.z].map { $0 * -Self.g } }
            }
        case .gyroscope:
            manager.gyroUpdateInterval = interval
            manager.startGyroUpdates(to: .main) { [weak self] d, _ in
                guard let d else { return }
                let r = d.rotationRate
                self?.take(.gyroscope, d.timestamp) { _ in [r.x, r.y, r.z] }
            }
        case .magnetometer:
            manager.magnetometerUpdateInterval = interval
            manager.startMagnetometerUpdates(to: .main) { [weak self] d, _ in
                guard let d else { return }
                let f = d.magneticField
                self?.take(.magnetometer, d.timestamp) { _ in [f.x, f.y, f.z] }
            }
        case .deviceMotion:
            manager.deviceMotionUpdateInterval = interval
            // Core Motion's own calibration display, as a hand-built compass sets it (D3).
            manager.showsDeviceMovementDisplay = frame == .xMagneticNorthZVertical
            manager.startDeviceMotionUpdates(using: frame ?? .xArbitraryZVertical, to: .main) { [weak self] m, _ in
                guard let m else { return }
                self?.take(.deviceMotion, m.timestamp) { name in Self.values(name, m) }
            }
        }
    }

    /// The session's clock at a Core Motion timestamp (seconds of system uptime).
    private func stamp(_ t: TimeInterval) -> Double {
        now() - (ProcessInfo.processInfo.systemUptime - t) * 1000
    }

    /// A sample of `stream` for each of its groups that takes it (D4's
    /// thinning), `values(name)` giving a name's values in the device frame.
    /// No reader is asked again for it: the runner polls each at its
    /// `interval`, and the samples between are the next answer's batch.
    private func take(_ stream: Stream, _ timestamp: TimeInterval, _ values: (String) -> Any) {
        let at = stamp(timestamp), deg = angle()
        for group in groups.values where group.stream == stream {
            if group.period > 0, at < group.next - group.period / 2 { continue }
            group.next = max(group.next + group.period, at + group.period / 2)
            group.sequence += 1
            let turn = group.args["referenceFrame"] as? String == "screen" ? deg : 0
            group.ring.append((group.sequence, Self.fields(group.name, values(group.name), at: at, screen: turn)))
            let bound = max(1, group.history)
            if group.ring.count > bound { group.ring.removeFirst(group.ring.count - bound) }
            let waiting = group.waiting
            group.waiting = []
            waiting.forEach { $0() }
        }
    }

    /// The screen's angle as the web's `screen.orientation.angle`.
    private func angle() -> Double {
        #if os(iOS)
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        switch scene?.effectiveGeometry.interfaceOrientation {
        case .landscapeRight: return 90
        case .portraitUpsideDown: return 180
        case .landscapeLeft: return 270
        default: return 0
        }
        #else
        return 0
        #endif
    }

    /// A device-motion name's values in the device frame: x, y, z; a quaternion; or the event's fields.
    private static func values(_ name: String, _ m: CMDeviceMotion) -> Any {
        let gravity = [m.gravity.x, m.gravity.y, m.gravity.z].map { $0 * -g }
        let user = [m.userAcceleration.x, m.userAcceleration.y, m.userAcceleration.z].map { $0 * -g }
        let rate = [m.rotationRate.x, m.rotationRate.y, m.rotationRate.z]
        let q = m.attitude.quaternion, quaternion = [q.x, q.y, q.z, q.w]
        let deg = 180 / Double.pi
        switch name {
        case "linearAccelerationSensor": return user
        case "gravitySensor": return gravity
        case "gyroscope": return rate
        case "absoluteOrientationSensor": return northUp(quaternion)
        case "relativeOrientationSensor": return quaternion
        case "deviceMotion":
            let v3: ([Double]) -> [String: Any] = { ["x": $0[0], "y": $0[1], "z": $0[2]] }
            return ["acceleration": v3(user), "accelerationIncludingGravity": v3(zip(gravity, user).map(+)),
                    "rotationRate": ["alpha": rate[2] * deg, "beta": rate[0] * deg, "gamma": rate[1] * deg],
                    "interval": 1000 / 60.0] as [String: Any]
        default:
            // Device Orientation's Z-X'-Y'' angles from Core Motion's attitude.
            let a = m.attitude
            return ["alpha": a.yaw * deg, "beta": a.pitch * deg, "gamma": a.roll * deg, "absolute": false, "apple": NSNull()] as [String: Any]
        }
    }

    /// A name's answer fields from its values, at `at` on the session clock, turned into the screen's frame at `deg`.
    private static func fields(_ name: String, _ values: Any, at: Double, screen deg: Double) -> [String: Any] {
        if var event = values as? [String: Any] {
            event["timeStamp"] = at
            return event
        }
        guard let v = values as? [Double] else { return [:] }
        if v.count == 4 {
            return ["quaternion": deg != 0 ? turned(v, deg) : v, "timestamp": at, "apple": NSNull()]
        }
        let s = deg != 0 ? screen(v, deg) : v
        return ["x": s[0], "y": s[1], "z": s[2], "timestamp": at]
    }

    /// `v`'s screen components at `deg` (Accelerometer §7.1).
    private static func screen(_ v: [Double], _ deg: Double) -> [Double] {
        let r = deg * .pi / 180, c = cos(r).rounded(), s = sin(r).rounded()
        return [v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]]
    }

    /// `q ⊗ (0, 0, −sin(θ/2), cos(θ/2))`: a vector's screen components are its device components turned by θ about z.
    private static func turned(_ q: [Double], _ deg: Double) -> [Double] {
        let h = deg * .pi / 360, bz = -sin(h), bw = cos(h)
        return [q[0] * bw + q[1] * bz, q[1] * bw - q[0] * bz, q[2] * bw + q[3] * bz, q[3] * bw - q[2] * bz]
    }

    /// `(0, 0, sin 45°, cos 45°) ⊗ q`: from Core Motion's x-north frame to the W3C's x-east, y-north one.
    private static func northUp(_ q: [Double]) -> [Double] {
        let s = 0.5.squareRoot()
        return [s * (q[0] - q[1]), s * (q[1] + q[0]), s * (q[2] + q[3]), s * (q[3] - q[2])]
    }
}
#else
/// The Mac has no device motion (`CMMotionManager` is iOS's): the status
/// reads unavailable and every sensor `NotSupportedError` (D14).
public final class MotionModule: ExactPackage {
    private static let status: [String: Any] = [
        "available": false, "state": "unknown", "apple": NSNull(), "android": NSNull(),
        "sensors": Dictionary(uniqueKeysWithValues: ["accelerometer", "linearAcceleration", "gravity", "gyroscope", "magnetometer",
                                                     "absoluteOrientation", "relativeOrientation", "deviceMotion", "deviceOrientation"].map { ($0, false) }),
    ]

    public init(context: ExactPackageContext) {}

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        reply(name == "motionStatus" || name == "requestMotionPermission" ? Self.status : ["error": "NotSupportedError"])
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "motionStatusSync" ? Self.status : ["error": "NotSupportedError"]
    }
}
#endif
