#if os(macOS)
import AppKit
import Foundation
import SceneKit

/// Lane r7-device: DeviceStreamView's 3D phone (MIT reference, see LICENSE-T3:
/// apps/web/src/components/device/DevicePhoneViewport.tsx, deviceFrameLayout.ts, phoneTrackpad.ts;
/// packages/client-runtime/src/device/phoneViewer.ts, phoneScene.ts, modelScene.ts,
/// shapeProfile.ts, phoneInteraction.ts) in SceneKit where the reference uses three.js. The live
/// picture is the screen's texture; a drag that starts on the screen is a touch, anywhere else
/// (or with ⌥) it turns the device, as does a two-finger trackpad swipe.
enum R7DeviceGeometry {
    /// fitDeviceFrame.
    static func fit(aspect: Double, width: Double, height: Double, rightInset: Double = 0) -> CGSize {
        let available = max(0, width - rightInset)
        guard available > 0, height > 0, aspect > 0 else { return .zero }
        return height * aspect <= available ? CGSize(width: height * aspect, height: height) : CGSize(width: available, height: available / aspect)
    }

    /// phoneDisplayLayout.
    struct Layout: Equatable { var aspect: Double; var rotation: Double; var landscape: Bool; var rawLandscape: Bool }
    static func layout(screen: R7DeviceClient.Screen?, rawWidth: Double, rawHeight: Double) -> Layout {
        let width = rawWidth > 0 ? rawWidth : screen?.width ?? 900, height = rawHeight > 0 ? rawHeight : screen?.height ?? 1950
        let orientation = screen?.orientation
        let rotation = orientation == "landscape_left" ? -Double.pi / 2 : orientation == "landscape_right" ? Double.pi / 2 : orientation == "portrait_upside_down" ? Double.pi : 0
        return Layout(aspect: min(width, height) / max(width, height), rotation: rotation, landscape: orientation == "landscape_left" || orientation == "landscape_right", rawLandscape: width > height)
    }

    /// updateDisplayUv: canonical portrait geometry → raw framebuffer coordinates (three.js's
    /// bottom-up v, which SceneKit's top-down t flips).
    static func uv(x: Double, y: Double, width: Double, height: Double, layout displayLayout: Layout) -> SIMD2<Float> {
        let u = (x + width / 2) / width, v = (y + height / 2) / height
        let s = displayLayout.rawLandscape ? (displayLayout.rotation > 0 ? 1 - v : v) : u
        let t = displayLayout.rawLandscape ? (displayLayout.rotation > 0 ? u : 1 - u) : v
        return SIMD2(Float(s), Float(1 - t))
    }

    /// createDisplayProjection's last step: a point on the portrait display → the displayed screen.
    static func screenPoint(u: Double, v: Double, rotation: Double) -> CGPoint {
        if abs(rotation + .pi / 2) < 1e-6 { return CGPoint(x: v, y: u) }
        if abs(rotation - .pi / 2) < 1e-6 { return CGPoint(x: 1 - v, y: 1 - u) }
        if abs(rotation - .pi) < 1e-6 { return CGPoint(x: 1 - u, y: v) }
        return CGPoint(x: u, y: 1 - v)
    }
}

/// shapeProfile.ts: original family silhouettes for devices without a model.
struct R7ShapeProfile: Equatable {
    struct Button: Equatable { let edge: String; let offset: Double; let length: Double }
    let id: String, bezel: Double, bodyRadius: Double, screenRadius: Double, depth: Double, backColor: UInt32
    let buttons: [Button]
    let cameraWidth: Double, cameraHeight: Double, insetX: Double, insetY: Double, lensRadius: Double
    let lenses: [SIMD2<Double>], flash: SIMD2<Double>?

    static let iosPhone = R7ShapeProfile(id: "ios-phone", bezel: 0.055, bodyRadius: 0.15, screenRadius: 0.105, depth: 0.085, backColor: 0x424b5d,
                                         buttons: [.init(edge: "right", offset: 0.35, length: 0.3), .init(edge: "left", offset: 0.48, length: 0.18), .init(edge: "left", offset: 0.22, length: 0.18)],
                                         cameraWidth: 0.39, cameraHeight: 0.44, insetX: 0.25, insetY: 0.29, lensRadius: 0.068, lenses: [SIMD2(-0.08, 0.095), SIMD2(0.08, -0.095)], flash: SIMD2(0.085, 0.11))
    static let iosTablet = R7ShapeProfile(id: "ios-tablet", bezel: 0.065, bodyRadius: 0.105, screenRadius: 0.045, depth: 0.055, backColor: 0x9ca5af,
                                          buttons: [.init(edge: "top", offset: 0.5, length: 0.15), .init(edge: "right", offset: 0.78, length: 0.13), .init(edge: "right", offset: 0.58, length: 0.13)],
                                          cameraWidth: 0.19, cameraHeight: 0.19, insetX: 0.15, insetY: 0.15, lensRadius: 0.045, lenses: [SIMD2(0, 0)], flash: nil)
    static let androidPhone = R7ShapeProfile(id: "android-phone", bezel: 0.035, bodyRadius: 0.115, screenRadius: 0.08, depth: 0.085, backColor: 0x344449,
                                             buttons: [.init(edge: "right", offset: 0.2, length: 0.24), .init(edge: "right", offset: 0.65, length: 0.32)],
                                             cameraWidth: 0.24, cameraHeight: 0.47, insetX: 0.18, insetY: 0.29, lensRadius: 0.056, lenses: [SIMD2(0, 0.11), SIMD2(0, -0.11)], flash: SIMD2(0.1, 0))
    static let androidTablet = R7ShapeProfile(id: "android-tablet", bezel: 0.065, bodyRadius: 0.105, screenRadius: 0.045, depth: 0.055, backColor: 0x697b80,
                                              buttons: iosTablet.buttons, cameraWidth: 0.19, cameraHeight: 0.19, insetX: 0.15, insetY: 0.15, lensRadius: 0.045, lenses: [SIMD2(0, 0)], flash: nil)

    /// resolveDeviceShape.
    static func resolve(platform: String, name: String, portraitAspect: Double) -> R7ShapeProfile {
        let lower = name.lowercased()
        func word(_ w: String) -> Bool { lower.range(of: "\\b\(w)\\b", options: .regularExpression) != nil }
        let tablet = word("ipad") || word("tablet") || (!(word("iphone") || word("phone")) && portraitAspect >= 0.6)
        return platform == "ios" ? (tablet ? iosTablet : iosPhone) : (tablet ? androidTablet : androidPhone)
    }
}

/// model.ts resolveDeviceModelId, deviceModels.ts deviceKeyboard, and the server's model files.
enum R7DeviceModels {
    static func id(platform: String, name: String) -> String? {
        guard platform == "ios" else { return nil }
        switch name.lowercased() {
        case "iphone duo": return "iphone-duo"
        case "iphone 18 pro max": return "iphone-18-pro-max"
        case "iphone 18 pro": return "iphone-18-pro"
        case "ipad pro 13-inch (m5)": return "ipad-pro-13-m5"
        default: return nil
        }
    }
    static func keyboard(platform: String, name: String) -> Bool { id(platform: platform, name: name) == "ipad-pro-13-m5" }

    private static var located: [String: [String: URL]] = [:]
    private static var locating: [String: [([String: URL]) -> Void]] = [:]
    private static var models: [URL: SCNNode] = [:]

    /// The served `assets/<id>-<hash>.glb` names, found by following the web client's own chunks
    /// (as T3TimelineMermaid finds the server's Mermaid build). Main thread.
    static func locate(origin: URL, _ done: @escaping ([String: URL]) -> Void) {
        let key = origin.absoluteString
        if let found = located[key] { return done(found) }
        if locating[key] != nil { locating[key]!.append(done); return }
        locating[key] = [done]
        guard let base = URL(string: "/", relativeTo: origin) else { return finish(key, [:]) }
        let reference = try! NSRegularExpression(pattern: #"(?:assets/|\./)([A-Za-z0-9_.-]+\.js)"#)
        let glb = try! NSRegularExpression(pattern: #"assets/((?:iphone-duo|iphone-18-pro-max|iphone-18-pro|ipad-pro-13-m5-magic-keyboard|ipad-pro-13-m5))-[A-Za-z0-9_-]{8}\.glb"#)
        func names(_ text: String) -> [String] { reference.matches(in: text, range: NSRange(text.startIndex..., in: text)).compactMap { Range($0.range(at: 1), in: text).map { "assets/" + text[$0] } } }
        func score(_ name: String) -> Int { name.contains("_chat") ? 3 : (name.contains("main-") || name.contains("index-")) ? 2 : 0 }
        var queue: [String] = [], seen = Set<String>()
        func fetch(_ url: URL, _ next: @escaping (String?) -> Void) {
            URLSession.shared.dataTask(with: URLRequest(url: url, cachePolicy: .returnCacheDataElseLoad, timeoutInterval: 15)) { data, response, _ in
                let text = data.flatMap { (response as? HTTPURLResponse)?.statusCode == 200 ? String(data: $0, encoding: .utf8) : nil }
                DispatchQueue.main.async { next(text) }
            }.resume()
        }
        func step() {
            queue.sort { score($0) > score($1) }
            guard seen.count < 80, let next = queue.first else { return finish(key, [:]) }
            queue.removeFirst()
            if !seen.insert(next).inserted { return step() }
            guard let url = URL(string: next, relativeTo: base) else { return step() }
            fetch(url) { text in
                guard let text else { return step() }
                var found: [String: URL] = [:]
                for match in glb.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
                    guard let whole = Range(match.range, in: text), let id = Range(match.range(at: 1), in: text), let url = URL(string: String(text[whole]), relativeTo: base) else { continue }
                    found[String(text[id])] = url.absoluteURL
                }
                if !found.isEmpty { return finish(key, found) }
                queue.append(contentsOf: names(text).filter { !seen.contains($0) })
                step()
            }
        }
        fetch(base) { index in
            guard let index else { return finish(key, [:]) }
            queue = names(index)
            step()
        }
    }
    private static func finish(_ key: String, _ found: [String: URL]) {
        if !found.isEmpty { located[key] = found }
        let waiting = locating.removeValue(forKey: key) ?? []
        waiting.forEach { $0(found) }
    }

    /// loadDeviceModel: one parse per file; each viewer takes a clone with its own screen.
    static func load(_ url: URL, _ done: @escaping (SCNNode?) -> Void) {
        if let model = models[url] { return done(model.clone()) }
        URLSession.shared.dataTask(with: url) { data, response, _ in
            let node = data.flatMap { (response as? HTTPURLResponse)?.statusCode == 200 ? try? R7GLB.parse($0) : nil }
            DispatchQueue.main.async {
                if let node { models[url] = node }
                done(node?.clone())
            }
        }.resume()
    }
}

final class R7DevicePhoneView: NSView {
    private let scnView: SCNView
    private let ground = CALayer()
    private let scene = SCNScene()
    private let camera = SCNNode()
    private let root = SCNNode(), orientation = SCNNode()
    private var body = SCNNode()
    private var display: SCNNode?
    private var displaySize = CGSize(width: 1, height: 2.2)
    private var displayZ = 0.043
    private let screenMaterial = SCNMaterial()
    private var imported = false
    var isImported: Bool { imported }
    private var accessory: SCNNode?
    private var accessoryWanted = false
    private let platform: String
    private var name: String
    private var screen: R7DeviceClient.Screen?
    private var raw = CGSize.zero
    private var displayLayout = R7DeviceGeometry.layout(screen: nil, rawWidth: 0, rawHeight: 0)
    private var profile: R7ShapeProfile
    private var orientationAngle = 0.0
    private var orientationTurn: (from: Double, to: Double, at: Double)?
    // Lane r9-device: an Android foldable's hinged body (R9DeviceFold.swift R9FoldScene).
    private(set) var fold: R9FoldScene?
    private var foldAngle: Double?
    private var foldTurn: (from: Double, to: Double, at: Double)?
    private var foldAspect = R9FoldScene.defaultInnerAspect
    private let motion = R7DeviceMotion()
    private let framing = R7DeviceFraming()
    private var timer: Timer?
    private var active: (mode: String, last: CGPoint, screen: CGPoint?)?
    private var scrolling = false
    private var disposed = false
    var origin: URL? { didSet { if origin != oldValue { loadModel() } } }
    let touch: (String, Double, Double) -> Void
    let unavailable: () -> Void
    static let fov = 32.0
    /// three.js light units → SceneKit lumens, matched against the reference's own renders.
    static var lightScale = (ambient: 1000 / Double.pi, direct: 360.0)

    init(platform: String, name: String, touch: @escaping (String, Double, Double) -> Void, unavailable: @escaping () -> Void) {
        self.platform = platform; self.name = name; self.touch = touch; self.unavailable = unavailable
        profile = R7ShapeProfile.resolve(platform: platform, name: name, portraitAspect: 9 / 19.5)
        scnView = SCNView(frame: .zero, options: [SCNView.Option.preferredRenderingAPI.rawValue: SCNRenderingAPI.metal.rawValue])
        super.init(frame: .zero)
        scnView.scene = scene
        scnView.backgroundColor = .clear
        scnView.antialiasingMode = .multisampling4X
        scnView.autoresizingMask = [.width, .height]
        scnView.rendersContinuously = false
        scnView.isJitteringEnabled = false
        wantsLayer = true
        // DevicePhoneViewport's ground shadow: bottom-[6%] h-5 w-2/5 rounded-full bg-foreground/10 blur-xl.
        ground.shadowOpacity = 1
        ground.shadowOffset = .zero
        ground.shadowRadius = 24
        layer?.addSublayer(ground)
        addSubview(scnView)
        setAccessibilityElement(true)
        setAccessibilityRole(.application)
        setAccessibilityLabel("Interactive 3D device. Drag the screen to interact. Drag outside it or swipe with two fingers to turn.")
        buildScene()
        tick()
    }
    required init?(coder: NSCoder) { nil }

    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func layout() { super.layout(); scnView.frame = bounds; layoutGround(); fit(immediate: true); invalidate() }
    override func viewDidChangeEffectiveAppearance() { super.viewDidChangeEffectiveAppearance(); layoutGround() }
    private func layoutGround() {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        let width = bounds.width * 0.4, height: CGFloat = 20
        ground.frame = CGRect(x: (bounds.width - width) / 2, y: bounds.height * 0.94 - height, width: width, height: height)
        ground.shadowPath = CGPath(roundedRect: ground.bounds, cornerWidth: height / 2, cornerHeight: height / 2, transform: nil)
        let dark = effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        ground.shadowColor = dark ? CGColor(srgbRed: 0xf5 / 255, green: 0xf5 / 255, blue: 0xf5 / 255, alpha: 0.1) : CGColor(srgbRed: 0x27 / 255, green: 0x27 / 255, blue: 0x2a / 255, alpha: 0.1)
        CATransaction.commit()
    }
    override func hitTest(_ point: NSPoint) -> NSView? { frame.contains(point) ? self : nil }

    // MARK: Scene

    private func buildScene() {
        let cam = SCNCamera()
        cam.fieldOfView = Self.fov
        cam.projectionDirection = .vertical
        cam.zNear = 0.1; cam.zFar = 30
        camera.camera = cam
        camera.position = SCNVector3(0, 0, 5.5)
        scene.rootNode.addChildNode(camera)
        // phoneViewer: ambient 2.4, a cool key, a white rim and a blue fill (three.js units; π of
        // them is one SceneKit kilolumen of Lambertian response).
        func light(_ type: SCNLight.LightType, _ color: UInt32, _ intensity: Double, _ position: SCNVector3?) {
            let light = SCNLight()
            light.type = type
            light.color = R7DevicePhoneView.color(color)
            light.intensity = CGFloat(intensity * (type == .ambient ? R7DevicePhoneView.lightScale.ambient : R7DevicePhoneView.lightScale.direct))
            let node = SCNNode(); node.light = light
            if let position { node.position = position; node.look(at: SCNVector3(0, 0, 0)) }
            scene.rootNode.addChildNode(node)
        }
        // three.js's AmbientLight lights only the diffuse term (2.4 / π of the albedo, less the metal);
        // SceneKit's also reaches a metal's specular, so it is applied as each material's emission.
        light(.directional, 0xe4edff, 5, SCNVector3(-3, 4, 5))
        light(.directional, 0xffffff, 4, SCNVector3(3, 1, -3))
        light(.directional, 0x9facd4, 2, SCNVector3(-2, -2, -4))
        screenMaterial.lightingModel = .constant
        screenMaterial.diffuse.contents = NSColor.black
        screenMaterial.isDoubleSided = true
        screenMaterial.diffuse.minificationFilter = .linear; screenMaterial.diffuse.magnificationFilter = .linear; screenMaterial.diffuse.mipFilter = .none
        root.addChildNode(orientation)
        scene.rootNode.addChildNode(root)
        installProcedural()
    }

    static func color(_ hex: UInt32) -> NSColor { NSColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: 1) }

    static func material(_ color: UInt32, metal: Double, rough: Double, clearcoat: Double = 0) -> SCNMaterial {
        let material = SCNMaterial()
        material.lightingModel = .physicallyBased
        material.diffuse.contents = R7DevicePhoneView.color(color)
        material.metalness.contents = NSNumber(value: metal)
        material.roughness.contents = NSNumber(value: rough)
        if clearcoat > 0 { material.clearCoat.contents = NSNumber(value: clearcoat) }
        ambient(material)
        return material
    }

    /// The ambient light's diffuse share as emission: albedo × 2.4 / π × (1 − metalness).
    static func ambient(_ material: SCNMaterial) {
        guard material.lightingModel == .physicallyBased, material.emission.contents == nil || (material.emission.contents as? NSColor) == .black else { return }
        let metal = material.metalness.contents is NSNumber ? (material.metalness.contents as! NSNumber).doubleValue : material.metalness.intensity * 0.5
        material.emission.contents = material.diffuse.contents
        material.emission.intensity = CGFloat(R7DevicePhoneView.lightScale.ambient * 2.4 / 1000 * max(0, 1 - metal))
    }

    static func rounded(_ width: Double, _ height: Double, _ radius: Double) -> NSBezierPath {
        NSBezierPath(roundedRect: NSRect(x: -width / 2, y: -height / 2, width: width, height: height), xRadius: radius, yRadius: radius)
    }

    /// createPhoneScene: an original procedural body for devices without a model.
    private func installProcedural() {
        body.removeFromParentNode()
        imported = false
        if platform == "android", let angle = foldAngle {
            let scene = R9FoldScene(material: screenMaterial, angle: visibleFoldAngle(angle), innerAspect: foldAspect)
            scene.rotation = displayLayout.rotation
            fold = scene; body = scene.root; display = nil
            orientationAngle = 0
            orientation.addChildNode(scene.root)
            accessory?.removeFromParentNode()
            fit(immediate: true)
            return
        }
        fold = nil
        let group = SCNNode()
        let screenHeight = 2.2, screenWidth = screenHeight * displayLayout.aspect
        let width = screenWidth + profile.bezel * 2, height = screenHeight + profile.bezel * 2
        let backZ = 0.01 - profile.depth
        let metal = Self.material(0xb5bcc7, metal: 0.88, rough: 0.27)
        let glass = Self.material(0x141820, metal: 0.15, rough: 0.2, clearcoat: 1)
        let back = Self.material(profile.backColor, metal: 0.45, rough: 0.32)
        let lens = Self.material(0x071326, metal: 0.6, rough: 0.12, clearcoat: 1)
        func shape(_ path: NSBezierPath, _ depth: Double, _ material: SCNMaterial, chamfer: Double = 0) -> SCNNode {
            path.flatness = 0.002
            let geometry = SCNShape(path: path, extrusionDepth: CGFloat(depth))
            geometry.chamferRadius = CGFloat(chamfer)
            geometry.materials = [material]
            return SCNNode(geometry: geometry)
        }
        let shell = shape(Self.rounded(width + 0.024, height + 0.024, profile.bodyRadius + 0.012), profile.depth + 0.024, metal, chamfer: 0.012)
        shell.position.z = CGFloat(0.025 - profile.depth / 2)
        group.addChildNode(shell)
        let face = shape(Self.rounded(width - 0.014, height - 0.014, profile.bodyRadius - 0.01), 0, glass)
        face.position.z = 0.04
        group.addChildNode(face)
        let rear = shape(Self.rounded(width - 0.012, height - 0.012, profile.bodyRadius - 0.01), 0, back)
        rear.eulerAngles.y = .pi
        rear.position.z = CGFloat(backZ)
        group.addChildNode(rear)
        let screenNode = SCNNode(geometry: displayGeometry(width: screenWidth, height: screenHeight, radius: profile.screenRadius))
        screenNode.position.z = 0.043
        group.addChildNode(screenNode)
        for button in profile.buttons {
            let box = SCNBox(width: button.edge == "top" ? button.length : 0.026, height: button.edge == "top" ? 0.026 : button.length, length: profile.depth * 0.65, chamferRadius: 0.006)
            box.materials = [metal]
            let node = SCNNode(geometry: box)
            node.position = SCNVector3(button.edge == "top" ? button.offset : (button.edge == "left" ? -1 : 1) * (width / 2 + 0.015), button.edge == "top" ? height / 2 + 0.015 : button.offset, (0.025 + backZ) / 2)
            group.addChildNode(node)
        }
        let rearCamera = SCNNode()
        rearCamera.position = SCNVector3(width / 2 - profile.insetX, height / 2 - profile.insetY, backZ + 0.001)
        rearCamera.eulerAngles.y = .pi
        group.addChildNode(rearCamera)
        let plateFront = 0.025 + 0.007, ringDepth = 0.025
        let plate = shape(Self.rounded(profile.cameraWidth, profile.cameraHeight, min(profile.cameraWidth, profile.cameraHeight) / 4), 0.025 + 0.014, back, chamfer: 0.007)
        plate.position.z = CGFloat((0.025 + 0.014) / 2 - 0.007)
        rearCamera.addChildNode(plate)
        for point in profile.lenses {
            let ring = SCNNode(geometry: { let c = SCNCylinder(radius: profile.lensRadius + 0.014, height: ringDepth); c.radialSegmentCount = 32; c.materials = [metal]; return c }())
            ring.eulerAngles.x = .pi / 2
            ring.position = SCNVector3(point.x, point.y, plateFront + ringDepth / 2 - 0.003)
            rearCamera.addChildNode(ring)
            let glassLens = shape(NSBezierPath(ovalIn: NSRect(x: -profile.lensRadius, y: -profile.lensRadius, width: profile.lensRadius * 2, height: profile.lensRadius * 2)), 0, lens)
            glassLens.position = SCNVector3(point.x, point.y, Double(ring.position.z) + ringDepth / 2 + 0.0005)
            rearCamera.addChildNode(glassLens)
        }
        if let flash = profile.flash {
            let material = SCNMaterial(); material.lightingModel = .constant; material.diffuse.contents = Self.color(0xf2ead6)
            let node = shape(NSBezierPath(ovalIn: NSRect(x: -0.022, y: -0.022, width: 0.044, height: 0.044)), 0, material)
            node.position = SCNVector3(flash.x, flash.y, plateFront + 0.0005)
            rearCamera.addChildNode(node)
        }
        display = screenNode; displaySize = CGSize(width: screenWidth, height: screenHeight); displayZ = 0.043
        body = group
        orientation.addChildNode(group)
        accessory?.removeFromParentNode()
        fit(immediate: true)
    }

    /// The display as a triangle fan over its rounded outline, UVs from updateDisplayUv.
    private func displayGeometry(width: Double, height: Double, radius: Double) -> SCNGeometry {
        var points: [SIMD2<Double>] = []
        let corners: [(SIMD2<Double>, Double)] = [(SIMD2(width / 2 - radius, -height / 2 + radius), -.pi / 2), (SIMD2(width / 2 - radius, height / 2 - radius), 0), (SIMD2(-width / 2 + radius, height / 2 - radius), .pi / 2), (SIMD2(-width / 2 + radius, -height / 2 + radius), .pi)]
        for (center, start) in corners { for step in 0...20 { let a = start + Double(step) / 20 * (.pi / 2); points.append(center + SIMD2(cos(a), sin(a)) * radius) } }
        let positions = [SCNVector3(0, 0, 0)] + points.map { SCNVector3($0.x, $0.y, 0) }
        var indices: [UInt16] = []
        for i in 1..<positions.count { indices += [0, UInt16(i), UInt16(i == positions.count - 1 ? 1 : i + 1)] }
        return displayGeometry(positions: positions, indices: indices, width: width, height: height)
    }

    private func displayGeometry(positions: [SCNVector3], indices: [UInt16], width: Double, height: Double) -> SCNGeometry {
        let uvs = positions.map { R7DeviceGeometry.uv(x: Double($0.x), y: Double($0.y), width: width, height: height, layout: displayLayout) }
        let normals = positions.map { _ in SCNVector3(0, 0, 1) }
        let uvData = uvs.withUnsafeBufferPointer { Data(buffer: $0) }
        let geometry = SCNGeometry(sources: [SCNGeometrySource(vertices: positions), SCNGeometrySource(normals: normals),
                                             SCNGeometrySource(data: uvData, semantic: .texcoord, vectorCount: uvs.count, usesFloatComponents: true, componentsPerVector: 2, bytesPerComponent: 4, dataOffset: 0, dataStride: 8)],
                                   elements: [SCNGeometryElement(indices: indices, primitiveType: .triangles)])
        geometry.materials = [screenMaterial]
        return geometry
    }

    /// The display's vertices and indices, to rebuild its UVs for a new layout.
    private func rebuildDisplayUV() {
        guard let display, let geometry = display.geometry, let vertices = geometry.sources(for: .vertex).first, let element = geometry.elements.first else { return }
        var positions: [SCNVector3] = []
        vertices.data.withUnsafeBytes { raw in
            for i in 0..<vertices.vectorCount {
                let base = vertices.dataOffset + i * vertices.dataStride
                let read = { (k: Int) -> Double in Double(raw.load(fromByteOffset: base + k * vertices.bytesPerComponent, as: Float.self)) }
                positions.append(SCNVector3(read(0), read(1), read(2)))
            }
        }
        var indices: [UInt16] = []
        element.data.withUnsafeBytes { raw in
            for i in 0..<(element.primitiveCount * 3) {
                indices.append(element.bytesPerIndex == 4 ? UInt16(truncatingIfNeeded: raw.load(fromByteOffset: i * 4, as: UInt32.self)) : raw.load(fromByteOffset: i * 2, as: UInt16.self))
            }
        }
        display.geometry = displayGeometry(positions: positions, indices: indices, width: displaySize.width, height: displaySize.height)
    }

    /// createImportedPhoneScene: the model's one `device-screen` mesh shows the stream.
    private func install(model: SCNNode) -> Bool {
        var screens: [SCNNode] = []
        model.enumerateHierarchy { node, _ in if node.name == "device-screen", node.geometry != nil { screens.append(node) } }
        guard screens.count == 1, let screenNode = screens.first else { return false }
        let (lower, upper) = screenNode.boundingBox
        let width = Double(upper.x - lower.x), height = Double(upper.y - lower.y)
        guard width > 0, abs(height - 2.2) < 0.001, abs(Double(lower.x + upper.x)) < 0.001 else { return false }
        body.removeFromParentNode()
        model.enumerateHierarchy { node, _ in node.geometry?.materials.forEach(Self.ambient) }
        display = screenNode; displaySize = CGSize(width: width, height: height); displayZ = Double(upper.z)
        rebuildDisplayUV()
        body = model; imported = true
        orientation.addChildNode(model)
        if accessoryWanted, let accessory { orientation.addChildNode(accessory) }
        fit(immediate: true); invalidate()
        return true
    }

    private func loadModel() {
        guard let origin, let id = R7DeviceModels.id(platform: platform, name: name), id != "iphone-duo" else { return }
        let wanted = name
        R7DeviceModels.locate(origin: origin) { [weak self] found in
            guard let self, !self.disposed, self.name == wanted, let url = found[id] else { return }
            R7DeviceModels.load(url) { [weak self] node in
                guard let self, !self.disposed, self.name == wanted, let node else { return }
                // The procedural body stays installed on download, decoding or validation failure.
                _ = self.install(model: node)
            }
            if R7DeviceModels.keyboard(platform: self.platform, name: self.name), let keyboard = found["ipad-pro-13-m5-magic-keyboard"] {
                R7DeviceModels.load(keyboard) { [weak self] node in
                    guard let self, !self.disposed else { return }
                    node?.enumerateHierarchy { child, _ in child.geometry?.materials.forEach(Self.ambient) }
                    self.accessory = node
                    if self.accessoryWanted, self.imported, let node { self.orientation.addChildNode(node); self.fit(immediate: true); self.invalidate() }
                }
            }
        }
    }

    // MARK: Inputs from the stream

    func setName(_ next: String) {
        guard next != name else { return }
        name = next
        accessory?.removeFromParentNode(); accessory = nil
        profile = R7ShapeProfile.resolve(platform: platform, name: name, portraitAspect: displayLayout.aspect)
        installProcedural(); loadModel(); invalidate()
    }

    func setKeyboard(_ attached: Bool) {
        guard attached != accessoryWanted else { return }
        accessoryWanted = attached
        if attached, imported, let accessory { orientation.addChildNode(accessory) } else { accessory?.removeFromParentNode() }
        fit(immediate: true); invalidate()
    }

    func setScreen(_ next: R7DeviceClient.Screen?) { screen = next; updateLayout() }

    func setFrame(_ image: CGImage) {
        let size = CGSize(width: image.width, height: image.height)
        if size != raw { raw = size; updateLayout() }
        SCNTransaction.begin(); SCNTransaction.disableActions = true
        screenMaterial.diffuse.contents = image
        SCNTransaction.commit()
        invalidate()
    }

    /// updateLayout: a new aspect rebuilds the procedural body; a new rotation turns the display
    /// (Android over 450 ms, iOS at once).
    private func updateLayout() {
        let next = R7DeviceGeometry.layout(screen: screen, rawWidth: Double(raw.width), rawHeight: Double(raw.height))
        // Learn the inner display shape from any unfolded frame, including before fold mode.
        let frameAspect = raw.height > 0 ? Double(raw.width / raw.height) : 0
        if R9FoldScene.isInnerAspect(frameAspect), frameAspect != foldAspect {
            foldAspect = frameAspect
            if fold != nil { displayLayout = next; installProcedural(); applyPose(); fit(immediate: true); invalidate(); return }
        }
        guard next != displayLayout else { return }
        if let fold {
            displayLayout = next; fold.rotation = next.rotation; orientationTurn = nil; orientationAngle = 0
            applyPose(); fit(immediate: true); invalidate(); return
        }
        let previous = displayLayout
        displayLayout = next
        let nextProfile = R7ShapeProfile.resolve(platform: platform, name: name, portraitAspect: next.aspect)
        if !imported, nextProfile != profile || next.aspect != previous.aspect { profile = nextProfile; installProcedural() } else { profile = nextProfile; rebuildDisplayUV() }
        if next.rotation != previous.rotation {
            if profile.id.hasPrefix("android") && !reducedMotion {
                let difference = atan2(sin(next.rotation - orientationAngle), cos(next.rotation - orientationAngle))
                orientationTurn = (orientationAngle, orientationAngle + difference, now)
            } else { orientationTurn = nil; orientationAngle = next.rotation }
        }
        applyPose(); fit(immediate: orientationTurn == nil); invalidate()
    }

    func resetPose() { motion.reset(R7Quat.identity, now); invalidate() }

    /// The hinge currently drawn, including an unfinished turn.
    private func visibleFoldAngle(_ fallback: Double) -> Double {
        guard let turn = foldTurn else { return fallback }
        let progress = min(1, (now - turn.at) / 850), eased = progress * progress * (3 - 2 * progress)
        return turn.from + (turn.to - turn.from) * eased
    }

    /// setFoldAngle: a foldable's hinge (null: the device does not fold); a change turns over 850 ms.
    func setFoldAngle(_ next: Double?) {
        guard !disposed, next != foldAngle, platform == "android" else { return }
        let previous = foldAngle
        foldAngle = next
        if next == nil || fold == nil {
            foldTurn = nil
            installProcedural()
            orientationTurn = nil
            orientationAngle = next == nil ? displayLayout.rotation : 0
            applyPose(); fit(immediate: true)
        } else if let next, let fold {
            let from = visibleFoldAngle(previous ?? next)
            if reducedMotion { foldTurn = nil; fold.setAngle(next); fit(immediate: true) } else { foldTurn = (from, next, now) }
        }
        invalidate()
    }

    func dispose() { disposed = true; timer?.invalidate(); timer = nil; active = nil; scnView.scene = nil }

    // MARK: Frame loop

    private var now: Double { CACurrentMediaTime() * 1000 }
    private var reducedMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

    private func applyPose() {
        let q = motion.rotation
        root.simdOrientation = simd_quatf(ix: Float(q.imag.x), iy: Float(q.imag.y), iz: Float(q.imag.z), r: Float(q.real))
        orientation.eulerAngles.z = CGFloat(orientationAngle)
    }

    /// Box3.setFromObject over the rotated assembly, then the framing spring.
    private func fit(immediate: Bool = false) {
        guard bounds.width > 0, bounds.height > 0 else { return }
        applyPose()
        var lower = SIMD3<Double>(repeating: .infinity), upper = SIMD3<Double>(repeating: -.infinity)
        root.enumerateHierarchy { node, _ in
            guard node.geometry != nil else { return }
            let (lo, hi) = node.boundingBox
            let a = SIMD3<Double>(Double(lo.x), Double(lo.y), Double(lo.z)), b = SIMD3<Double>(Double(hi.x), Double(hi.y), Double(hi.z))
            for corner in [SIMD3(a.x, a.y, a.z), SIMD3(b.x, a.y, a.z), SIMD3(a.x, b.y, a.z), SIMD3(b.x, b.y, a.z), SIMD3(a.x, a.y, b.z), SIMD3(b.x, a.y, b.z), SIMD3(a.x, b.y, b.z), SIMD3(b.x, b.y, b.z)] {
                let world = node.simdConvertPosition(SIMD3<Float>(Float(corner.x), Float(corner.y), Float(corner.z)), to: nil)
                let point = SIMD3<Double>(Double(world.x), Double(world.y), Double(world.z))
                lower = simd_min(lower, point); upper = simd_max(upper, point)
            }
        }
        guard lower.x.isFinite else { return }
        framing.setBounds(min: lower, max: upper, vertical: Self.fov * .pi / 360, aspect: Double(bounds.width / bounds.height), now: now, immediate: immediate)
        applyCamera()
    }

    private func applyCamera() {
        camera.simdPosition = SIMD3(Float(framing.center.x), Float(framing.center.y), Float(framing.distance))
        camera.look(at: SCNVector3(framing.center.x, framing.center.y, 0))
    }

    func invalidate() {
        guard !disposed else { return }
        if timer == nil {
            let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] _ in self?.tick() }
            RunLoop.main.add(timer, forMode: .common)
            self.timer = timer
        }
    }

    private func tick() {
        guard !disposed else { return }
        let time = now, reduced = reducedMotion
        if motion.advance(time, reduced: reduced) { applyPose(); fit(immediate: reduced) }
        if let turn = orientationTurn {
            let progress = min(1, (time - turn.at) / 450), eased = progress * progress * (3 - 2 * progress)
            orientationAngle = turn.from + (turn.to - turn.from) * eased
            if progress == 1 { orientationTurn = nil }
            applyPose(); fit(immediate: reduced)
        }
        if let turn = foldTurn, let fold {
            let progress = min(1, (time - turn.at) / 850), eased = progress * progress * (3 - 2 * progress)
            fold.setAngle(turn.from + (turn.to - turn.from) * eased)
            if progress == 1 { foldTurn = nil }
            fit(immediate: reduced)
        }
        framing.advance(time, immediate: reduced)
        applyCamera()
        scnView.needsDisplay = true
        if !(motion.needsFrame || framing.needsFrame || orientationTurn != nil || foldTurn != nil) { timer?.invalidate(); timer = nil }
    }

    // MARK: Interaction (createPhoneInteraction)

    private func normalized(_ event: NSEvent) -> CGPoint {
        let point = convert(event.locationInWindow, from: nil)
        return CGPoint(x: point.x / max(1, bounds.width), y: point.y / max(1, bounds.height))
    }

    /// phone.screenPoint: a new touch must hit the display; a captured drag projects onto its plane.
    func screenPoint(_ point: CGPoint, captured: Bool) -> CGPoint? {
        if let fold, bounds.width > 0 {
            applyPose()
            return fold.screenPoint(view: scnView, at: CGPoint(x: point.x * bounds.width, y: (1 - point.y) * bounds.height), captured: captured)
        }
        guard let display, bounds.width > 0 else { return nil }
        applyPose()
        let view = CGPoint(x: point.x * bounds.width, y: (1 - point.y) * bounds.height)
        var local: SIMD3<Double>
        if !captured {
            guard let hit = scnView.hitTest(view, options: [.rootNode: display, .searchMode: SCNHitTestSearchMode.closest.rawValue, .backFaceCulling: false]).first(where: { $0.node === display }) else { return nil }
            let world = hit.simdWorldCoordinates
            let inOrientation = orientation.simdConvertPosition(world, from: nil)
            local = SIMD3(Double(inOrientation.x), Double(inOrientation.y), Double(inOrientation.z))
        } else {
            let near = scnView.unprojectPoint(SCNVector3(view.x, view.y, 0)), far = scnView.unprojectPoint(SCNVector3(view.x, view.y, 1))
            let a = orientation.simdConvertPosition(SIMD3(Float(near.x), Float(near.y), Float(near.z)), from: nil)
            let b = orientation.simdConvertPosition(SIMD3(Float(far.x), Float(far.y), Float(far.z)), from: nil)
            let direction = b - a
            guard abs(direction.z) > 1e-9 else { return nil }
            let t = (Float(displayZ) - a.z) / direction.z
            guard t >= 0 else { return nil }
            let p = a + direction * t
            local = SIMD3(Double(p.x), Double(p.y), Double(p.z))
        }
        let u = min(1, max(0, (local.x + Double(displaySize.width) / 2) / Double(displaySize.width)))
        let v = min(1, max(0, (local.y + Double(displaySize.height) / 2) / Double(displaySize.height)))
        return R7DeviceGeometry.screenPoint(u: u, v: v, rotation: displayLayout.rotation)
    }

    private func setInteractionActive(_ on: Bool, _ mode: String) {
        if mode == "orbit" { motion.dragActive(on, now) } else { motion.hold(on, now); framing.hold(on, now) }
        invalidate()
    }

    private func orbit(_ dx: Double, _ dy: Double) {
        motion.orbit(dx * Double(bounds.width), dy * Double(bounds.height), now)
        invalidate()
    }

    override func mouseDown(with event: NSEvent) {
        superview?.window?.makeFirstResponder(superview)
        guard active == nil else { return }
        endScroll()
        let point = normalized(event)
        let screen = event.modifierFlags.contains(.option) ? nil : screenPoint(point, captured: false)
        active = (screen == nil ? "orbit" : "touch", point, screen)
        setInteractionActive(true, active!.mode)
        if let screen { touch("begin", Double(screen.x), Double(screen.y)) }
    }

    override func mouseDragged(with event: NSEvent) {
        guard var current = active else { return }
        let point = normalized(event)
        if current.mode == "touch" {
            if let screen = screenPoint(point, captured: true) { current.screen = screen; touch("move", Double(screen.x), Double(screen.y)) }
        } else { orbit(Double(point.x - current.last.x), Double(point.y - current.last.y)) }
        current.last = point
        active = current
    }

    override func mouseUp(with event: NSEvent) { mouseDragged(with: event); end() }

    func end() {
        guard let previous = active else { return }
        active = nil
        if previous.mode == "touch", let screen = previous.screen { touch("end", Double(screen.x), Double(screen.y)) }
        setInteractionActive(false, previous.mode)
    }

    /// phoneTrackpad: two-finger swipes turn the device (momentum is ignored); the gesture's end
    /// releases it to the nearest view.
    override func scrollWheel(with event: NSEvent) {
        if event.momentumPhase != [] { return endScroll() }
        if event.phase == .ended || event.phase == .cancelled { return endScroll() }
        guard active == nil, bounds.width > 0 else { return }
        let unit = event.hasPreciseScrollingDeltas ? 1.0 : 16.0
        let x = max(-0.25, min(0.25, Double(event.scrollingDeltaX) * unit / Double(bounds.width)))
        let y = max(-0.25, min(0.25, Double(event.scrollingDeltaY) * unit / Double(bounds.height)))
        guard x != 0 || y != 0 else { return }
        scrolling = true
        orbit(x, y)
        if event.phase == [] { NSObject.cancelPreviousPerformRequests(withTarget: self, selector: #selector(endScrollLater), object: nil); perform(#selector(endScrollLater), with: nil, afterDelay: 1.2) }
    }
    @objc private func endScrollLater() { endScroll() }
    private func endScroll() {
        guard scrolling else { return }
        scrolling = false
        if active == nil { setInteractionActive(false, "orbit") }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        NotificationCenter.default.removeObserver(self)
        if let window { NotificationCenter.default.addObserver(self, selector: #selector(resigned), name: NSWindow.didResignKeyNotification, object: window) }
    }
    @objc private func resigned() { end(); endScroll() }

    /// Test seam: the rendered picture.
    func snapshot() -> NSImage { scnView.snapshot() }
}
#endif
