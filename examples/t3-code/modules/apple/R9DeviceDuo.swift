#if os(macOS)
import AppKit
import Foundation
import simd

/// Lane r9-device: the iPhone Duo's device-hub protocol (MIT reference, see LICENSE-T3:
/// packages/client-runtime/src/device/stream.ts screenConfigSchema / controlReplySchema,
/// duoControl.ts createDuoControl / createDuoPinch, duoScene.ts duoRawPoint / duoFrameMatches /
/// duoDisplayKey, duoSnap.ts duoViewSnaps, deviceViewSnap.ts nearestDeviceView). serve-sim's screen
/// config (`0x82`) gains the hinge fields; a hinge command is `0x10 {requestId, command}` and its
/// reply `0x90 {requestId, ok, error?}`; an orientation command is the ordinary `0x07` message,
/// acknowledged by the next screen config.
struct R9DuoConfig: Equatable {
    var screenId: Int?
    var supportsHingeAngle = false
    var supportsPhysicalOrientation = false
    var hingeAngle: Double?
    var hingePose: String?
    var tableMode: Bool?
    var tableModeAvailable: Bool?

    static let poses = ["closed", "book", "open", "laptop", "tent"]

    /// The optional keys of screenConfigSchema. `.success(nil)` when none is present; `.failure` when
    /// one is malformed, which rejects the whole config as Schema.decodeUnknownOption does.
    static func parse(_ config: [String: Any]) -> Result<R9DuoConfig?, Failure> {
        let keys = ["screenId", "supportsHingeAngle", "supportsPhysicalOrientation", "hingeAngle", "hingePose", "tableMode", "tableModeAvailable"]
        guard keys.contains(where: { config[$0] != nil }) else { return .success(nil) }
        var duo = R9DuoConfig()
        func bool(_ key: String) -> Result<Bool?, Failure> {
            guard let value = config[key] else { return .success(nil) }
            guard let number = value as? NSNumber, CFGetTypeID(number) == CFBooleanGetTypeID() else { return .failure(Failure()) }
            return .success(number.boolValue)
        }
        if let value = config["screenId"] {
            guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.doubleValue.isFinite else { return .failure(Failure()) }
            duo.screenId = Int(exactly: number.doubleValue) ?? Int(number.doubleValue)
        }
        guard case .success(let hinge) = bool("supportsHingeAngle"), case .success(let physical) = bool("supportsPhysicalOrientation"),
              case .success(let table) = bool("tableMode"), case .success(let tableAvailable) = bool("tableModeAvailable") else { return .failure(Failure()) }
        duo.supportsHingeAngle = hinge ?? false; duo.supportsPhysicalOrientation = physical ?? false
        duo.tableMode = table; duo.tableModeAvailable = tableAvailable
        if let value = config["hingeAngle"] {
            guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.doubleValue.isFinite, (0...180).contains(number.doubleValue) else { return .failure(Failure()) }
            duo.hingeAngle = number.doubleValue
        }
        if let value = config["hingePose"] {
            if value is NSNull { duo.hingePose = nil } else {
                guard let pose = value as? String, poses.contains(pose) else { return .failure(Failure()) }
                duo.hingePose = pose
            }
        }
        return .success(duo)
    }
    struct Failure: Error {}

    var report: [String: Any] {
        ["supported": supportsHingeAngle, "screenId": screenId ?? NSNull(), "hingeAngle": hingeAngle ?? NSNull(), "hingePose": hingePose ?? "", "physical": supportsPhysicalOrientation]
    }
}

/// DuoCommand.
enum R9DuoCommand: Equatable {
    case angle(Double), pose(String), table(Bool), physical(String), orientation(String)
    var control: String {
        switch self { case .angle: "angle"; case .pose: "pose"; case .table: "table"; case .physical: "physical"; case .orientation: "orientation" }
    }
    var value: Any {
        switch self { case .angle(let v): v; case .pose(let v): v; case .table(let v): v; case .physical(let v): v; case .orientation(let v): v }
    }
    var json: [String: Any] { ["control": control, "value": value] }
    /// `pose:open`, `angle:120`, `orientation:portrait` (the app's `r6DeviceInput` value).
    static func parse(_ text: String) -> R9DuoCommand? {
        let parts = text.split(separator: ":", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return nil }
        switch parts[0] {
        case "pose": return R9DuoConfig.poses.contains(parts[1]) ? .pose(parts[1]) : nil
        case "angle": return Double(parts[1]).map(R9DuoCommand.angle)
        case "orientation": return R6DeviceWire.orientations.contains(parts[1]) ? .orientation(parts[1]) : nil
        case "physical": return parts[1] == "faceup" || parts[1] == "facedown" ? .physical(parts[1]) : nil
        case "table": return parts[1] == "true" ? .table(true) : parts[1] == "false" ? .table(false) : nil
        default: return nil
        }
    }
}

struct R9DuoControlState: Equatable {
    var pending = false, requested: R9DuoCommand?, error: String?
    var report: [String: Any] {
        ["pending": pending, "requested": requested.map { $0.json } ?? NSNull(), "error": error ?? ""]
    }
}

/// createDuoControl: one in-flight native transaction. Hinge motion coalesces; presets replace
/// queued motion. Nothing replays after reconnect. Main thread.
final class R9DuoControl {
    private let send: (Int, R9DuoCommand) -> Bool
    private let onChange: (R9DuoControlState) -> Void
    private let timeout: TimeInterval
    private var nextId = 1
    private(set) var active: (requestId: Int, command: R9DuoCommand)?
    private var queued: R9DuoCommand?
    private var timer: DispatchWorkItem?
    private(set) var state = R9DuoControlState()

    init(timeout: TimeInterval = 5, send: @escaping (Int, R9DuoCommand) -> Bool, onChange: @escaping (R9DuoControlState) -> Void) {
        self.timeout = timeout; self.send = send; self.onChange = onChange
    }

    private func publish(_ error: String? = nil) {
        state = R9DuoControlState(pending: active != nil, requested: queued ?? active?.command, error: error)
        onChange(state)
    }

    func clear(_ error: String? = nil) {
        timer?.cancel(); timer = nil
        active = nil; queued = nil
        publish(error)
    }

    private func drain() {
        guard active == nil, let command = queued else { return }
        let id = nextId; nextId += 1
        active = (id, command); queued = nil
        let item = DispatchWorkItem { [weak self] in
            guard let self, self.active?.requestId == id else { return }
            self.clear("Device control timed out. Its position is unknown.")
        }
        timer = item
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: item)
        publish()
        if !send(id, command) { clear("Device is disconnected.") }
    }

    func enqueue(_ command: R9DuoCommand) {
        if case .angle(let value) = command, !value.isFinite || value < 0 || value > 180 { return }
        queued = command
        if active != nil { publish() } else { drain() }
    }

    func receive(requestId: Int, ok: Bool, error: String?) {
        guard let current = active, current.requestId == requestId else { return }
        timer?.cancel(); timer = nil
        active = nil
        guard ok else { return clear(error ?? "Device control failed. Its position is unknown.") }
        if queued != nil { drain() } else { publish() }
    }
}

/// createDuoPinch: a pinch keeps its own accumulator across asynchronous native acknowledgements.
final class R9DuoPinch {
    private var angle: Double?
    let current: () -> Double, contains: (Double, Double) -> Bool, change: (Double?) -> Void
    init(angle: @escaping () -> Double, contains: @escaping (Double, Double) -> Bool, change: @escaping (Double?) -> Void) {
        current = angle; self.contains = contains; self.change = change
    }
    var active: Bool { angle != nil }
    func begin(_ x: Double, _ y: Double) -> Bool {
        guard contains(x, y) else { return false }
        angle = max(0, min(180, current()))
        return true
    }
    func move(_ logScale: Double) {
        guard let value = angle, logScale.isFinite else { return }
        let next = max(0, min(180, value + logScale * 120))
        guard next != value else { return }
        angle = next
        change(next)
    }
    func end() {
        guard angle != nil else { return }
        angle = nil
        change(nil)
    }
}

enum R9Duo {
    /// duoRawPoint: hardware mounting is independent of app orientation and the model's orbit.
    static func rawPoint(panel: Int, x: Double, y: Double) -> CGPoint { panel == 1 ? CGPoint(x: x, y: y) : CGPoint(x: y, y: 1 - x) }

    static func frameMatches(width: Double, height: Double, screen: R7DeviceClient.Screen) -> Bool {
        width > 0 && height > 0 && abs(width / height - screen.width / screen.height) <= 1 / height + 1 / screen.height
    }

    static func displayKey(_ screen: R7DeviceClient.Screen?) -> String {
        guard let screen else { return "" }
        let id = screen.duo?.screenId.map(String.init) ?? "undefined"
        return "\(id):\(Self.number(screen.width)):\(Self.number(screen.height)):\(screen.orientation)"
    }
    private static func number(_ value: Double) -> String { value.rounded() == value ? String(Int(value)) : String(value) }

    /// The viewer's panel surfaces: the cover 784 × 1140, the inner 1600 × 1125, painted #080a10.
    static func surfaceSize(_ panel: Int) -> CGSize { panel == 1 ? CGSize(width: 784, height: 1140) : CGSize(width: 1600, height: 1125) }

    /// frameUpdated's draw: the frame fitted into its surface; the inner panel is mounted a quarter turn.
    static func paint(_ image: CGImage, panel: Int) -> CGImage? {
        let size = surfaceSize(panel)
        guard let context = CGContext(data: nil, width: Int(size.width), height: Int(size.height), bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
        context.setFillColor(CGColor(srgbRed: 8 / 255, green: 10 / 255, blue: 16 / 255, alpha: 1))
        context.fill(CGRect(origin: .zero, size: size))
        // The canvas's clockwise quarter turn (y-down) is a negative angle in Core Graphics' y-up space.
        context.translateBy(x: size.width / 2, y: size.height / 2)
        let rotate = panel == 3
        if rotate { context.rotate(by: -.pi / 2) }
        let width = Double(image.width), height = Double(image.height)
        let scale = min(Double(size.width) / (rotate ? height : width), Double(size.height) / (rotate ? width : height))
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: -width * scale / 2, y: -height * scale / 2, width: width * scale, height: height * scale))
        return context.makeImage()
    }

    /// The activation probe: an 8 × 8 reduction with no channel above 3 is a native shutdown blank.
    static func isBlank(_ image: CGImage) -> Bool {
        var pixels = [UInt8](repeating: 0, count: 8 * 8 * 4)
        guard let context = CGContext(data: &pixels, width: 8, height: 8, bitsPerComponent: 8, bytesPerRow: 32, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        context.draw(image, in: CGRect(x: 0, y: 0, width: 8, height: 8))
        return !pixels.enumerated().contains { $0.offset % 4 != 3 && $0.element > 3 }
    }
}

/// duoSnap.ts: views built from the actual hinged display planes (three.js conventions, see R7Quat).
struct R9DuoRestFrame { var face: String; var normal: SIMD3<Double>; var up: SIMD3<Double>; var center: SIMD3<Double> }
struct R9DuoViewSnap { var rotation: simd_quatd; var face: String; var orientation: String; var center: SIMD3<Double>; var yawLimit: Double }

enum R9DuoSnaps {
    static func axis(_ v: SIMD3<Double>, _ angle: Double) -> simd_quatd { simd_quatd(angle: angle, axis: simd_normalize(v)) }

    /// Matrix4.makeBasis(right, up, normal) → Quaternion.setFromRotationMatrix.
    static func basis(_ x: SIMD3<Double>, _ y: SIMD3<Double>, _ z: SIMD3<Double>) -> simd_quatd {
        simd_quatd(simd_double3x3(columns: (x, y, z)))
    }

    static func snaps(_ frames: [R9DuoRestFrame], panel: Int) -> [R9DuoViewSnap] {
        var out: [R9DuoViewSnap] = []
        let orientations = R6DeviceWire.orientations
        for frame in frames {
            let normal = simd_normalize(frame.normal)
            guard simd_length_squared(normal) >= 0.5 else { continue }
            let right = simd_normalize(simd_cross(frame.up, normal))
            let up = simd_normalize(simd_cross(normal, right))
            let faceRotation = basis(right, up, normal).inverse
            for index in 0..<orientations.count {
                if frame.face == "right" && index != 0 { continue }
                if frame.face == "left" && index != 2 { continue }
                let roll = (panel == 3 ? Double.pi / 2 : 0) - Double(index) * .pi / 2
                var rotation = axis(SIMD3(0, 0, 1), roll) * faceRotation
                if frame.face == "left" || frame.face == "right" { rotation = axis(SIMD3(1, 0, 0), .pi / 12) * rotation }
                let leaves = frames.filter { $0.face == "left" || $0.face == "right" }
                if panel == 3, leaves.contains(where: { rotation.act($0.normal).z < 0.04 }) { continue }
                var yawLimit = frame.face == "cover" ? Double.pi / 9 : Double.pi / 3
                if panel == 3 {
                    for sign in [-1.0, 1.0] {
                        for step in 1...60 {
                            let turn = axis(SIMD3(0, 1, 0), sign * Double(step) * .pi / 180)
                            if leaves.contains(where: { turn.act(rotation.act($0.normal)).z < 0.04 }) {
                                yawLimit = min(yawLimit, Double(step - 1) * .pi / 180)
                                break
                            }
                        }
                    }
                }
                out.append(R9DuoViewSnap(rotation: rotation, face: frame.face, orientation: orientations[index], center: frame.center, yawLimit: yawLimit))
            }
        }
        return out
    }

    /// nearestDeviceView: the closest member of each family, then the closest family.
    static func nearest(_ rotation: simd_quatd, _ snaps: [R9DuoViewSnap]) -> R9DuoViewSnap? {
        var closest: R9DuoViewSnap?, distance = Double.infinity
        for snap in snaps {
            let relative = rotation * snap.rotation.inverse
            let turn = 2 * atan2(relative.imag.y, relative.real)
            let yaw = max(-snap.yawLimit, min(snap.yawLimit, atan2(sin(turn), cos(turn))))
            let candidate = axis(SIMD3(0, 1, 0), yaw) * snap.rotation
            let next = R7Quat.angle(candidate, rotation)
            if next < distance - 1e-8 { distance = next; var found = snap; found.rotation = candidate; closest = found }
        }
        return closest
    }

    /// The physical presentation a pose rests in (createDuoViewer setScreen), before any snap.
    static func presentation(pose: String, angle: Double, screen: R7DeviceClient.Screen?) -> simd_quatd {
        let fold = (180 - angle) * .pi / 360
        var roll = pose == "closed" ? 0 : Double.pi / 2
        if let screen, screen.width < screen.height {
            if screen.orientation == "landscape_left" { roll -= .pi / 2 }
            if screen.orientation == "landscape_right" { roll += .pi / 2 }
        }
        if screen?.orientation == "portrait_upside_down" { roll -= .pi }
        let euler: SIMD3<Double> = pose == "laptop" ? SIMD3(-fold + .pi / 9, -.pi / 9, .pi / 2)
            : pose == "tent" ? SIMD3(.pi / 2 + .pi / 18, -.pi / 9, -.pi / 2)
            : SIMD3(0, Double.pi / 2 * pow(1 - angle / 180, 3), 0)
        // Euler order "YXZ": q = qY · qX · qZ.
        var q = axis(SIMD3(0, 1, 0), euler.y) * axis(SIMD3(1, 0, 0), euler.x) * axis(SIMD3(0, 0, 1), euler.z)
        if pose != "laptop" && pose != "tent" { q = axis(SIMD3(0, 0, 1), roll) * q }
        return q
    }
}
#endif
