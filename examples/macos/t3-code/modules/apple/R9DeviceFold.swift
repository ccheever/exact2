#if os(macOS)
import AppKit
import Foundation
import SceneKit

/// Lane r9-device: an Android foldable (MIT reference, see LICENSE-T3:
/// apps/web/src/components/device/deviceFold.ts, DeviceAndroidFoldControls.tsx;
/// packages/client-runtime/src/device/androidFoldScene.ts, phoneViewer.ts setFoldAngle). serve-emu's
/// `GET /api/fold?device=` reads `{ok, fold: {supported, posture, hingeAngle}}`; `POST` with
/// `{posture: "closed" | "opened"}` changes it and answers the same shape. Capability comes from the
/// emulator, never from its AVD name or screen size.
struct R9FoldState: Equatable {
    var supported: Bool, posture: String?, hingeAngle: Double?
    static let postures = ["closed", "half_opened", "opened", "flipped", "tent"]

    /// parseFold: anything else is "Unexpected Android fold response."
    static func parse(_ data: Data?) -> R9FoldState? {
        guard let data, let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              payload["ok"] as? Bool == true, let fold = payload["fold"] as? [String: Any],
              let supported = fold["supported"] as? NSNumber, CFGetTypeID(supported) == CFBooleanGetTypeID() else { return nil }
        let posture = fold["posture"], angle = fold["hingeAngle"]
        var state = R9FoldState(supported: supported.boolValue, posture: nil, hingeAngle: nil)
        if let posture, !(posture is NSNull) {
            guard let text = posture as? String, postures.contains(text) else { return nil }
            state.posture = text
        } else if posture == nil { return nil }
        if let angle, !(angle is NSNull) {
            guard let number = angle as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.doubleValue.isFinite else { return nil }
            state.hingeAngle = number.doubleValue
        } else if angle == nil { return nil }
        return state
    }

    /// onFoldAngle: the drawn hinge (null while the device does not fold).
    var drawnAngle: Double? { supported ? hingeAngle ?? (posture == "closed" ? 0 : 180) : nil }
}

/// DeviceAndroidFoldControls' reads and writes for one emulator, through the server's hub proxy.
final class R9DeviceFold {
    typealias Access = R7DeviceClient.Access
    private let deviceId: String, hostId: String, access: Access
    private let changed: () -> Void
    private(set) var fold: R9FoldState?
    private(set) var pending = false
    private(set) var error: String?
    /// The angle the 3D phone draws: the read, the press's target while it runs.
    private(set) var angle: Double?
    private var readKey = ""
    private var generation = 0
    private var retry: DispatchWorkItem?
    var session = URLSession(configuration: .ephemeral)
    static var setTimeout: TimeInterval = 12, retryDelay: TimeInterval = 3

    init(deviceId: String, hostId: String, access: @escaping Access, changed: @escaping () -> Void) {
        self.deviceId = deviceId; self.hostId = hostId; self.access = access; self.changed = changed
    }

    var report: [String: Any] {
        guard let fold else { return ["supported": false] }
        return ["supported": fold.supported, "posture": fold.posture ?? "", "hingeAngle": fold.hingeAngle ?? NSNull(), "pending": pending, "error": error ?? ""]
    }

    private func request(_ posture: String?, timeout: TimeInterval, _ done: @escaping (Result<R9FoldState, Failure>) -> Void) {
        access { [session, deviceId, hostId] origin, ticket in
            guard let origin, let ticket, let url = R6DeviceWire.url(origin: origin, path: "/vendor/serve-emu/api/fold", query: [URLQueryItem(name: "device", value: deviceId)], ticket: ticket, hostId: hostId) else {
                return DispatchQueue.main.async { done(.failure(Failure(message: "Reconnect to the environment and try again.", timedOut: false))) }
            }
            var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: timeout)
            if let posture {
                request.httpMethod = "POST"
                request.setValue("application/json", forHTTPHeaderField: "content-type")
                request.httpBody = try? JSONSerialization.data(withJSONObject: ["posture": posture])
            }
            session.dataTask(with: request) { data, response, failure in
                let code = (response as? HTTPURLResponse)?.statusCode ?? 0
                let result: Result<R9FoldState, Failure>
                if let failure {
                    result = .failure(Failure(message: failure.localizedDescription, timedOut: (failure as? URLError)?.code == .timedOut))
                } else if !(200..<300).contains(code) {
                    let payload = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
                    result = .failure(Failure(message: payload?["error"] as? String ?? "Fold command failed (\(code)).", timedOut: false))
                } else if let state = R9FoldState.parse(data) {
                    result = .success(state)
                } else { result = .failure(Failure(message: "Unexpected Android fold response.", timedOut: false)) }
                DispatchQueue.main.async { done(result) }
            }.resume()
        }
    }
    struct Failure: Error { let message: String; let timedOut: Bool }

    /// The read effect: runs whenever the device is shown, streaming and sized, and again for each
    /// new size or when a press settles; a failed read retries every 3 s.
    func update(visible: Bool, enabled: Bool, width: Double?, height: Double?) {
        let key = visible && enabled && !pending && (width ?? 0) > 0 && (height ?? 0) > 0 ? "\(width!)x\(height!)" : ""
        guard key != readKey else { return }
        readKey = key
        generation += 1; retry?.cancel(); retry = nil
        if !key.isEmpty { read(generation) }
    }

    private func read(_ run: Int) {
        request(nil, timeout: 15) { [weak self] result in
            guard let self, run == self.generation else { return }
            switch result {
            case .success(let next):
                self.fold = next; self.angle = next.drawnAngle; self.changed()
            case .failure:
                let item = DispatchWorkItem { [weak self] in guard let self, run == self.generation else { return }; self.read(run) }
                self.retry = item
                DispatchQueue.main.asyncAfter(deadline: .now() + Self.retryDelay, execute: item)
            }
        }
    }

    /// change: one press at a time; the 3D hinge turns at once and returns if the command fails.
    func set(_ posture: String, enabled: Bool) {
        guard !pending, enabled, let fold, fold.supported, posture == "closed" || posture == "opened" else { return }
        pending = true; error = nil
        generation += 1; retry?.cancel(); retry = nil; readKey = ""
        angle = posture == "closed" ? 0 : 180
        changed()
        request(posture, timeout: Self.setTimeout) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let next): self.fold = next; self.angle = next.hingeAngle ?? (next.posture == "closed" ? 0 : 180)
            case .failure(let failure):
                self.angle = fold.hingeAngle ?? (fold.posture == "closed" ? 0 : 180)
                self.error = failure.timedOut ? "Fold command timed out." : failure.message
            }
            self.pending = false
            self.changed()
        }
    }

    func stop() { generation += 1; retry?.cancel(); retry = nil; readKey = "" }
}

/// createAndroidFoldScene: a procedural book-style foldable. One fixed half, one half rotating
/// around a shared hinge; one continuous inner surface keeps adjacent pixels joined at the crease.
final class R9FoldScene {
    static let height = 2.2, depth = 0.075, inset = 0.026, crease = 0.004, bevel = 0.008
    static let pivotZ = depth / 2 + 0.005, spineRadius = pivotZ + depth / 2 - 0.002
    static let defaultInnerAspect = 2076.0 / 2152.0
    static func isInnerAspect(_ aspect: Double) -> Bool { aspect.isFinite && aspect > 0.75 && aspect < 1.5 }

    let root = SCNNode()
    let halfWidth: Double
    private let left = SCNNode(), right = SCNNode(), leftBody = SCNNode(), spine = SCNNode()
    private let innerLeft = SCNNode(), innerRight = SCNNode(), cover = SCNNode()
    private let surface = SCNNode()
    private let material: SCNMaterial
    private let screenWidth: Double, screenHeight: Double
    private var baseX: [Double] = [], baseY: [Double] = []
    private let columns = 40, rows = 48
    private(set) var angle: Double
    var rotation = 0.0
    private var captured: SCNNode?

    var width: Double { halfWidth * 2 }

    init(material: SCNMaterial, angle: Double, innerAspect: Double = R9FoldScene.defaultInnerAspect) {
        self.material = material
        let height = Self.height, depth = Self.depth, inset = Self.inset
        screenHeight = height - 2 * inset
        halfWidth = innerAspect * screenHeight / 2 + inset
        screenWidth = 2 * (halfWidth - inset)
        self.angle = angle
        let frameMetal = R7DevicePhoneView.material(0xa3abb2, metal: 0.9, rough: 0.28)
        let polished = R7DevicePhoneView.material(0xc4cad0, metal: 0.95, rough: 0.16)
        let bezel = R7DevicePhoneView.material(0x0b0d10, metal: 0.1, rough: 0.2, clearcoat: 1)
        let backGlass = R7DevicePhoneView.material(0x2c3237, metal: 0.35, rough: 0.52, clearcoat: 0.4)
        let island = R7DevicePhoneView.material(0x1a1e22, metal: 0.55, rough: 0.3, clearcoat: 1)
        let lens = R7DevicePhoneView.material(0x061022, metal: 0.6, rough: 0.1, clearcoat: 1)
        let flash = SCNMaterial(); flash.lightingModel = .constant; flash.diffuse.contents = R7DevicePhoneView.color(0xf2ead6)
        let hit = SCNMaterial(); hit.colorBufferWriteMask = []; hit.writesToDepthBuffer = false; hit.isDoubleSided = true
        root.addChildNode(left); root.addChildNode(right)
        // Each half's meshes live in body coordinates; `left` pivots them around the hinge axis.
        left.position.z = CGFloat(Self.pivotZ)
        leftBody.position.z = CGFloat(-Self.pivotZ)
        left.addChildNode(leftBody)
        func shape(_ path: NSBezierPath, _ extrusion: Double, _ material: SCNMaterial, chamfer: Double = 0) -> SCNNode {
            path.flatness = 0.002
            let geometry = SCNShape(path: path, extrusionDepth: CGFloat(extrusion))
            geometry.chamferRadius = CGFloat(chamfer)
            geometry.materials = [material]
            return SCNNode(geometry: geometry)
        }
        let half = halfWidth
        func build(_ group: SCNNode, _ side: String, _ back: SCNMaterial, _ display: SCNNode) {
            let body = shape(Self.panel(half, side, inset: 0, radius: 0.1 + Self.bevel, hinge: 0), depth, frameMetal, chamfer: Self.bevel)
            group.addChildNode(body)
            let frame = shape(Self.panel(half, side, inset: 0.01, radius: 0.095, hinge: 0.002), 0, bezel)
            frame.position.z = CGFloat(depth / 2 + 0.001)
            group.addChildNode(frame)
            // A back-facing shape mirrors X, so it is drawn from the opposite side's outline.
            let rear = shape(Self.panel(half, side == "left" ? "right" : "left", inset: 0.01, radius: 0.095, hinge: 0.002), 0, back)
            rear.name = "\(side)-back"; rear.eulerAngles.y = .pi
            rear.position.z = CGFloat(-depth / 2 - 0.001)
            group.addChildNode(rear)
            display.geometry = shape(Self.panel(half, side, inset: inset, radius: 0.076, hinge: 0), 0, hit).geometry
            display.name = "\(side)-inner-screen"
            display.position.z = CGFloat(depth / 2 + 0.003)
            group.addChildNode(display)
        }
        build(leftBody, "left", bezel, innerLeft)
        build(right, "right", backGlass, innerRight)
        surface.name = "continuous-inner-screen"
        root.addChildNode(surface)
        for i in 0...rows { for j in 0...columns {
            let y = screenHeight / 2 - Double(i) / Double(rows) * screenHeight
            let x0 = -screenWidth / 2 + Double(j) / Double(columns) * screenWidth
            let radius = 0.076, cornerY = max(0, abs(y) - (screenHeight / 2 - radius))
            let limit = screenWidth / 2 - radius + sqrt(max(0, radius * radius - cornerY * cornerY))
            baseX.append(max(-limit, min(limit, x0))); baseY.append(y)
        } }
        // The outer half of the hinge housing: tucked behind the back glass when open, the spine when closed.
        spine.geometry = Self.partialCylinder(radius: Self.spineRadius, height: height - 0.012, start: .pi / 2 + 0.15, length: .pi - 0.3, material: polished)
        spine.name = "hinge-spine"
        spine.position.z = CGFloat(Self.pivotZ)
        root.addChildNode(spine)
        cover.geometry = Self.roundedPlane(width: half - inset * 2 - 0.02, height: height - inset * 2 - 0.04, radius: 0.07, material: material)
        cover.name = "cover-screen"
        cover.position = SCNVector3(-half / 2, 0, -depth / 2 - 0.003)
        cover.eulerAngles.y = .pi
        leftBody.addChildNode(cover)
        // Rear components use back-surface coordinates, with outward positive Z.
        let camera = SCNNode(); camera.name = "rear-camera"
        let islandWidth = 0.46, islandHeight = 0.2
        camera.position = SCNVector3(half - 0.07 - islandWidth / 2, height / 2 - 0.08 - islandHeight / 2, -depth / 2 - 0.002)
        camera.eulerAngles.y = .pi
        right.addChildNode(camera)
        let plateDepth = 0.02
        let plate = shape(NSBezierPath(roundedRect: NSRect(x: -islandWidth / 2 - 0.008, y: -islandHeight / 2 - 0.008, width: islandWidth + 0.016, height: islandHeight + 0.016), xRadius: 0.078, yRadius: 0.078), plateDepth + 0.012, island, chamfer: 0.006)
        plate.name = "camera-plate"; plate.position.z = CGFloat(plateDepth / 2)
        camera.addChildNode(plate)
        let plateFront = plateDepth + 0.006
        for (x, radius) in [(-0.14, 0.05), (-0.01, 0.05), (0.105, 0.036)] {
            let ringGeometry = SCNCylinder(radius: CGFloat(radius + 0.012), height: 0.012); ringGeometry.radialSegmentCount = 32; ringGeometry.materials = [frameMetal]
            let ring = SCNNode(geometry: ringGeometry); ring.eulerAngles.x = .pi / 2; ring.position = SCNVector3(x, 0, plateFront + 0.004)
            camera.addChildNode(ring)
            let glass = shape(NSBezierPath(ovalIn: NSRect(x: -radius, y: -radius, width: radius * 2, height: radius * 2)), 0, lens)
            glass.name = "camera-lens"; glass.position = SCNVector3(x, 0, plateFront + 0.0105)
            camera.addChildNode(glass)
        }
        let light = shape(NSBezierPath(ovalIn: NSRect(x: -0.018, y: -0.018, width: 0.036, height: 0.036)), 0, flash)
        light.position = SCNVector3(0.185, 0.045, plateFront + 0.0005)
        camera.addChildNode(light)
        // Power and volume keys sit on the fixed half's outer edge.
        for (y, length) in [(0.52, 0.16), (0.2, 0.3)] {
            let box = SCNBox(width: 0.02, height: CGFloat(length), length: CGFloat(depth * 0.45), chamferRadius: 0.004); box.materials = [frameMetal]
            let key = SCNNode(geometry: box); key.position = SCNVector3(half + 0.008, y, 0)
            right.addChildNode(key)
        }
        setAngle(angle)
    }

    /// panelPath: one half's outline; its hinge edge stays square.
    static func panel(_ halfWidth: Double, _ side: String, inset: Double, radius: Double, hinge: Double) -> NSBezierPath {
        let leftEdge = side == "left" ? -halfWidth + inset : crease / 2 + hinge
        let rightEdge = side == "left" ? -crease / 2 - hinge : halfWidth - inset
        let bottom = -height / 2 + inset, top = height / 2 - inset
        let path = NSBezierPath()
        func quad(_ control: CGPoint, _ end: CGPoint) {
            let start = path.currentPoint
            path.curve(to: end, controlPoint1: CGPoint(x: start.x + 2 / 3 * (control.x - start.x), y: start.y + 2 / 3 * (control.y - start.y)),
                       controlPoint2: CGPoint(x: end.x + 2 / 3 * (control.x - end.x), y: end.y + 2 / 3 * (control.y - end.y)))
        }
        if side == "left" {
            path.move(to: CGPoint(x: leftEdge + radius, y: bottom))
            path.line(to: CGPoint(x: rightEdge, y: bottom)); path.line(to: CGPoint(x: rightEdge, y: top))
            path.line(to: CGPoint(x: leftEdge + radius, y: top)); quad(CGPoint(x: leftEdge, y: top), CGPoint(x: leftEdge, y: top - radius))
            path.line(to: CGPoint(x: leftEdge, y: bottom + radius)); quad(CGPoint(x: leftEdge, y: bottom), CGPoint(x: leftEdge + radius, y: bottom))
        } else {
            path.move(to: CGPoint(x: leftEdge, y: bottom))
            path.line(to: CGPoint(x: rightEdge - radius, y: bottom)); quad(CGPoint(x: rightEdge, y: bottom), CGPoint(x: rightEdge, y: bottom + radius))
            path.line(to: CGPoint(x: rightEdge, y: top - radius)); quad(CGPoint(x: rightEdge, y: top), CGPoint(x: rightEdge - radius, y: top))
            path.line(to: CGPoint(x: leftEdge, y: top))
        }
        path.close()
        return path
    }

    /// CylinderGeometry with a theta range (x = r·sin θ, z = r·cos θ) and its two pie caps.
    static func partialCylinder(radius: Double, height: Double, start: Double, length: Double, material: SCNMaterial) -> SCNGeometry {
        var positions: [SCNVector3] = [], normals: [SCNVector3] = [], indices: [UInt16] = []
        let segments = 32
        for i in 0...segments {
            let theta = start + Double(i) / Double(segments) * length
            let normal = SCNVector3(sin(theta), 0, cos(theta))
            positions += [SCNVector3(radius * sin(theta), height / 2, radius * cos(theta)), SCNVector3(radius * sin(theta), -height / 2, radius * cos(theta))]
            normals += [normal, normal]
        }
        for i in 0..<segments { let a = UInt16(i * 2); indices += [a, a + 1, a + 2, a + 2, a + 1, a + 3] }
        for (y, sign) in [(height / 2, 1.0), (-height / 2, -1.0)] {
            let center = UInt16(positions.count)
            positions.append(SCNVector3(0, y, 0)); normals.append(SCNVector3(0, sign, 0))
            for i in 0...segments { let theta = start + Double(i) / Double(segments) * length; positions.append(SCNVector3(radius * sin(theta), y, radius * cos(theta))); normals.append(SCNVector3(0, sign, 0)) }
            for i in 0..<segments { let a = center + 1 + UInt16(i); indices += sign > 0 ? [center, a, a + 1] : [center, a + 1, a] }
        }
        let geometry = SCNGeometry(sources: [SCNGeometrySource(vertices: positions), SCNGeometrySource(normals: normals)], elements: [SCNGeometryElement(indices: indices, primitiveType: .triangles)])
        geometry.materials = [material]; material.isDoubleSided = true
        return geometry
    }

    /// A rounded rectangle as a triangle fan with UVs spanning its bounds (setDisplay's cover UVs;
    /// SceneKit's t runs down where three.js's v runs up).
    static func roundedPlane(width: Double, height: Double, radius: Double, material: SCNMaterial) -> SCNGeometry {
        var points: [SIMD2<Double>] = []
        let corners: [(SIMD2<Double>, Double)] = [(SIMD2(width / 2 - radius, -height / 2 + radius), -.pi / 2), (SIMD2(width / 2 - radius, height / 2 - radius), 0),
                                                   (SIMD2(-width / 2 + radius, height / 2 - radius), .pi / 2), (SIMD2(-width / 2 + radius, -height / 2 + radius), .pi)]
        for (center, start) in corners { for step in 0...12 { let a = start + Double(step) / 12 * (.pi / 2); points.append(center + SIMD2(cos(a), sin(a)) * radius) } }
        let positions = [SCNVector3(0, 0, 0)] + points.map { SCNVector3($0.x, $0.y, 0) }
        var indices: [UInt16] = []
        for i in 1..<positions.count { indices += [0, UInt16(i), UInt16(i == positions.count - 1 ? 1 : i + 1)] }
        let uvs = positions.map { SIMD2<Float>(Float((Double($0.x) + width / 2) / width), Float(1 - (Double($0.y) + height / 2) / height)) }
        let uvData = uvs.withUnsafeBufferPointer { Data(buffer: $0) }
        let geometry = SCNGeometry(sources: [SCNGeometrySource(vertices: positions), SCNGeometrySource(normals: positions.map { _ in SCNVector3(0, 0, 1) }),
                                             SCNGeometrySource(data: uvData, semantic: .texcoord, vectorCount: uvs.count, usesFloatComponents: true, componentsPerVector: 2, bytesPerComponent: 4, dataOffset: 0, dataStride: 8)],
                                   elements: [SCNGeometryElement(indices: indices, primitiveType: .triangles)])
        geometry.materials = [material]
        return geometry
    }

    /// setAngle: the moving half turns about the hinge; the inner surface's left columns fold with it.
    func setAngle(_ next: Double) {
        angle = max(0, min(180, next))
        let radians = Double.pi * (1 - angle / 180)
        left.eulerAngles.y = CGFloat(radians)
        spine.eulerAngles.y = CGFloat(radians / 2)
        let cosine = cos(radians), sine = sin(radians), frontZ = Self.depth / 2 + 0.004 - Self.pivotZ
        var positions: [SCNVector3] = [], uvs: [SIMD2<Float>] = [], indices: [UInt16] = []
        for (index, x) in baseX.enumerated() {
            let y = baseY[index]
            positions.append(x < 0 ? SCNVector3(x * cosine + frontZ * sine, y, -x * sine + frontZ * cosine + Self.pivotZ) : SCNVector3(x, y, frontZ + Self.pivotZ))
            uvs.append(SIMD2(Float(x / screenWidth + 0.5), Float(1 - (y / screenHeight + 0.5))))
        }
        for i in 0..<rows { for j in 0..<columns {
            let a = UInt16(i * (columns + 1) + j), b = a + 1, c = a + UInt16(columns + 1), d = c + 1
            indices += [a, c, b, b, c, d]
        } }
        let uvData = uvs.withUnsafeBufferPointer { Data(buffer: $0) }
        let geometry = SCNGeometry(sources: [SCNGeometrySource(vertices: positions), SCNGeometrySource(normals: positions.map { _ in SCNVector3(0, 0, 1) }),
                                             SCNGeometrySource(data: uvData, semantic: .texcoord, vectorCount: uvs.count, usesFloatComponents: true, componentsPerVector: 2, bytesPerComponent: 4, dataOffset: 0, dataStride: 8)],
                                   elements: [SCNGeometryElement(indices: indices, primitiveType: .triangles)])
        geometry.materials = [material]
        surface.geometry = geometry
        let innerActive = angle >= 90
        surface.isHidden = !innerActive; innerLeft.isHidden = !innerActive; innerRight.isHidden = !innerActive
        cover.isHidden = innerActive
    }

    /// screenPoint: the visible display under the pointer (cover or either inner half), or a captured
    /// drag's plane; the inner halves share one framebuffer side by side.
    func screenPoint(view: SCNView, at point: CGPoint, captured isCaptured: Bool) -> CGPoint? {
        let screens = cover.isHidden ? [innerLeft, innerRight] : [cover]
        let hits = view.hitTest(point, options: [.searchMode: SCNHitTestSearchMode.all.rawValue, .backFaceCulling: false, .rootNode: root])
        let hit = hits.first { result in screens.contains { $0 === result.node } }
        if hit == nil && !isCaptured { captured = nil; return nil }
        let display = hit?.node ?? (captured.map { !$0.isHidden ? $0 : screens[0] } ?? screens[0])
        if hit != nil { captured = display }
        var local: SIMD3<Float>
        if let hit { local = display.simdConvertPosition(hit.simdWorldCoordinates, from: nil) } else {
            let near = view.unprojectPoint(SCNVector3(point.x, point.y, 0)), far = view.unprojectPoint(SCNVector3(point.x, point.y, 1))
            let a = display.simdConvertPosition(SIMD3(Float(near.x), Float(near.y), Float(near.z)), from: nil)
            let b = display.simdConvertPosition(SIMD3(Float(far.x), Float(far.y), Float(far.z)), from: nil)
            guard abs(b.z - a.z) > 1e-9 else { return nil }
            local = a + (b - a) * (-a.z / (b.z - a.z))
        }
        let (lower, upper) = display.boundingBox
        let u = max(0, min(1, Double((local.x - Float(lower.x)) / Float(upper.x - lower.x))))
        let v = max(0, min(1, Double((Float(upper.y) - local.y) / Float(upper.y - lower.y))))
        let across = display === cover ? u : (display === innerLeft ? u : 1 + u) / 2
        return abs(rotation - .pi) < 1e-6 ? CGPoint(x: 1 - across, y: 1 - v) : CGPoint(x: across, y: v)
    }
}
#endif
