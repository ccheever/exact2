#if os(macOS)
import AppKit
import Foundation
import SceneKit
import CoreImage

/// Lane r9-device: the iPhone Duo's hinged 3D viewer (MIT reference, see LICENSE-T3:
/// apps/web/src/components/device/DeviceDuoViewport.tsx; packages/client-runtime/src/device/
/// duoViewer.ts createDuoViewer, duoScene.ts createDuoScene) in SceneKit where the reference uses
/// three.js. The connected server's own `iphone-duo` model (never bundled) has two hinge groups
/// (`left-half`, `right-half`) and three displays (`cover-display`, `inner-display-left`,
/// `inner-display-right`); the cover shows panel 1, the inner pair shares panel 3's framebuffer.
/// The hinge follows the hub's confirmed angle (or a pinch's preview); the body rests in the pose's
/// physical presentation and snaps to views built from the real display planes.
final class R9DuoView: NSView {
    private let scnView = SCNView(frame: .zero, options: [SCNView.Option.preferredRenderingAPI.rawValue: SCNRenderingAPI.metal.rawValue])
    private let ground = CALayer()
    private let scene = SCNScene()
    private let camera = SCNNode()
    private let root = SCNNode(), content = SCNNode()
    private var asset: SCNNode?
    private var leftHalf: SCNNode?, rightHalf: SCNNode?
    private var surfaces: [Int: [SCNNode]] = [:]
    private var bounds3: [Int: (lower: SIMD2<Double>, size: SIMD2<Double>)] = [:]
    private let materials: [Int: SCNMaterial] = [1: R9DuoView.screenMaterial(1), 3: R9DuoView.screenMaterial(3)]
    private var screen: R7DeviceClient.Screen?
    private var readyKey = "", primaryKey = "", activationAt = 0.0
    private var restFace = "inside"
    private var requestedOrientation: String?, viewOrientation: String?
    private var requestedPanel: Int?
    private var handoff: DispatchWorkItem?
    private var pivot = SIMD3<Double>.zero, targetPivot = SIMD3<Double>.zero
    private var angle = 180.0, targetAngle = 180.0, appliedAngle = Double.nan
    private var previewAngle: Double?
    private var hingeLeaf: String?
    private var interactionActive = false
    private var firstPose = true
    private var physicalPose = "open", presentationAngle = 180.0
    private var targetPresentation = R7Quat.identity
    private var lastTime = 0.0
    private lazy var motion = R7DeviceMotion(choose: { [weak self] rotation in self?.choose(rotation) ?? rotation })
    private let framing = R7DeviceFraming()
    private var timer: Timer?
    private var disposed = false
    private var active: (mode: String, last: CGPoint, screen: CGPoint?)?
    private var scrolling = false
    private(set) var pinch: R9DuoPinch!
    private var captured: (panel: Int, node: SCNNode, key: String)?
    let touch: (String, Double, Double) -> Void
    let unavailable: () -> Void
    let panelRequested: (Int) -> Void
    let orientationRequested: (String) -> Void
    let hinge: (Double) -> Void
    var origin: URL? { didSet { if origin != oldValue { loadModel() } } }
    static let fov = 36.0

    init(touch: @escaping (String, Double, Double) -> Void, unavailable: @escaping () -> Void, panelRequested: @escaping (Int) -> Void,
         orientationRequested: @escaping (String) -> Void, hinge: @escaping (Double) -> Void) {
        self.touch = touch; self.unavailable = unavailable; self.panelRequested = panelRequested; self.orientationRequested = orientationRequested; self.hinge = hinge
        super.init(frame: .zero)
        scnView.scene = scene
        scnView.backgroundColor = .clear
        scnView.antialiasingMode = .multisampling4X
        scnView.autoresizingMask = [.width, .height]
        scnView.rendersContinuously = false
        wantsLayer = true
        // DeviceDuoViewport's ground shadow: bottom-[6%] h-5 w-2/5 rounded-full bg-foreground/10 blur-xl.
        ground.shadowOpacity = 1; ground.shadowOffset = .zero; ground.shadowRadius = 24
        layer?.addSublayer(ground)
        addSubview(scnView)
        setAccessibilityElement(true)
        setAccessibilityRole(.application)
        setAccessibilityLabel("Interactive 3D iPhone Duo. Drag the screen to interact. Drag outside it or swipe with two fingers to turn. Pinch over the device to open or close its hinge.")
        let cam = SCNCamera()
        cam.fieldOfView = Self.fov; cam.projectionDirection = .vertical; cam.zNear = 0.1; cam.zFar = 150
        camera.camera = cam
        scene.rootNode.addChildNode(camera)
        // createDuoViewer: ambient 2.4, a white key and a cool fill (three.js units, as R7DevicePhoneView converts them).
        func light(_ type: SCNLight.LightType, _ color: UInt32, _ intensity: Double, _ position: SCNVector3) {
            let light = SCNLight(); light.type = type; light.color = R7DevicePhoneView.color(color)
            light.intensity = CGFloat(intensity * R7DevicePhoneView.lightScale.direct)
            let node = SCNNode(); node.light = light; node.position = position; node.look(at: SCNVector3(0, 0, 0))
            scene.rootNode.addChildNode(node)
        }
        light(.directional, 0xffffff, 4, SCNVector3(-6, 10, 14))
        light(.directional, 0xc7dcff, 2, SCNVector3(8, -3, -5))
        // RoomEnvironment through PMREM (sigma 0.04): a soft grey room with bright ceiling panels.
        scene.lightingEnvironment.contents = R9Duo.room()
        scene.lightingEnvironment.intensity = R9Duo.roomIntensity
        root.addChildNode(content)
        scene.rootNode.addChildNode(root)
        pinch = R9DuoPinch(angle: { [weak self] in
            guard let self else { return 180 }
            return self.previewAngle ?? self.screen?.duo?.hingeAngle ?? (self.screen?.duo?.screenId == 1 ? 0 : 180)
        }, contains: { [weak self] x, y in self?.screen != nil && self?.beginHinge(x, y) == true }, change: { [weak self] value in
            guard let self else { return }
            self.setHingePreview(value ?? self.controlPreview)
            if let value { self.hinge(value) }
        })
    }
    required init?(coder: NSCoder) { nil }

    /// The queued or in-flight angle command (DeviceStreamView's hingePreview), kept while a pinch rests.
    var controlPreview: Double?

    static func screenMaterial(_ panel: Int) -> SCNMaterial {
        let material = SCNMaterial()
        material.lightingModel = .constant
        material.isDoubleSided = true
        let size = R9Duo.surfaceSize(panel)
        material.diffuse.contents = R9Duo.blank(size)
        material.diffuse.minificationFilter = .linear; material.diffuse.magnificationFilter = .linear; material.diffuse.mipFilter = .none
        return material
    }

    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { frame.contains(point) ? self : nil }
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

    // MARK: Model (createDuoScene)

    private func loadModel() {
        guard let origin else { return }
        R7DeviceModels.locate(origin: origin) { [weak self] found in
            guard let self, !self.disposed else { return }
            guard let url = found["iphone-duo"] else { return self.unavailable() }
            R7DeviceModels.load(url) { [weak self] node in
                guard let self, !self.disposed else { return }
                guard let node, self.install(node) else { return self.unavailable() }
            }
        }
    }

    /// Validates the rig and replaces the three displays' materials with the live panels.
    func install(_ model: SCNNode) -> Bool {
        let named = { (name: String) in model.childNode(withName: name, recursively: true) }
        guard let left = named("left-half"), let right = named("right-half"), let cover = named("cover-display"), cover.geometry != nil,
              let innerLeft = named("inner-display-left"), innerLeft.geometry != nil, let innerRight = named("inner-display-right"), innerRight.geometry != nil else { return false }
        model.enumerateHierarchy { node, _ in node.geometry?.materials.forEach(R7DevicePhoneView.ambient) }
        let groups: [Int: [SCNNode]] = [1: [cover], 3: [innerLeft, innerRight]]
        for id in [1, 3] {
            var lower = SIMD2<Double>(repeating: .infinity), upper = SIMD2<Double>(repeating: -.infinity)
            for mesh in groups[id]! {
                let (lo, hi) = mesh.boundingBox
                lower = simd_min(lower, SIMD2(Double(lo.x), Double(lo.y))); upper = simd_max(upper, SIMD2(Double(hi.x), Double(hi.y)))
            }
            let size = upper - lower
            guard size.x > 0, size.y > 0, size.x.isFinite, size.y.isFinite else { return false }
            bounds3[id] = (lower, size)
            for mesh in groups[id]! { mesh.geometry = Self.retextured(mesh.geometry!, lower: lower, size: size, mirror: id == 1, material: materials[id]!) }
        }
        // The displays share their planes with the glass behind them (the cover's z = -0.524, the inner
        // bezels' z = 0); three.js draws the replacement materials last and wins the tie, SceneKit
        // z-fights. A third of a millimetre outward, drawn after the body, settles it at every distance.
        cover.simdPosition.z -= 0.03
        innerLeft.simdPosition.z += 0.03; innerRight.simdPosition.z += 0.03
        for node in [cover, innerLeft, innerRight] { node.renderingOrder = 1 }
        asset?.removeFromParentNode()
        asset = model; leftHalf = left; rightHalf = right; surfaces = groups
        content.addChildNode(model)
        appliedAngle = .nan
        applyPose()
        if screen?.duo?.screenId == 1, let snap = R9DuoSnaps.nearest(motion.rotation, activeSnaps()) {
            restFace = snap.face
            motion.setPose(snap.rotation, now, immediate: true)
            applyPose()
        }
        pivot = targetPivot
        applyPose(); fit(); invalidate()
        return true
    }

    /// The display's UVs from its own bounds; the cover's face outward on the rear leaf (mirrored).
    static func retextured(_ geometry: SCNGeometry, lower: SIMD2<Double>, size: SIMD2<Double>, mirror: Bool, material: SCNMaterial) -> SCNGeometry {
        guard let vertices = geometry.sources(for: .vertex).first else { return geometry }
        var uvs: [SIMD2<Float>] = []
        vertices.data.withUnsafeBytes { raw in
            for i in 0..<vertices.vectorCount {
                let base = vertices.dataOffset + i * vertices.dataStride
                let read = { (k: Int) -> Double in vertices.bytesPerComponent == 8 ? raw.load(fromByteOffset: base + k * 8, as: Double.self) : Double(raw.load(fromByteOffset: base + k * 4, as: Float.self)) }
                let x = (read(0) - lower.x) / size.x, y = (read(1) - lower.y) / size.y
                uvs.append(SIMD2(Float(mirror ? 1 - x : x), Float(1 - y)))
            }
        }
        let uvData = uvs.withUnsafeBufferPointer { Data(buffer: $0) }
        let sources = geometry.sources.filter { $0.semantic != .texcoord } + [SCNGeometrySource(data: uvData, semantic: .texcoord, vectorCount: uvs.count, usesFloatComponents: true, componentsPerVector: 2, bytesPerComponent: 4, dataOffset: 0, dataStride: 8)]
        let next = SCNGeometry(sources: sources, elements: geometry.elements)
        next.materials = [material]
        return next
    }

    private func setAngle(_ value: Double) {
        guard let leftHalf, let rightHalf, let asset else { return }
        let radians = (180 - min(180, max(0, value))) * .pi / 360
        leftHalf.eulerAngles.y = CGFloat(radians); rightHalf.eulerAngles.y = CGFloat(-radians)
        // Orbit the folded body's centre while retaining the authored hinge pivots.
        content.simdPosition = .zero
        var lower = SIMD3<Double>(repeating: .infinity), upper = SIMD3<Double>(repeating: -.infinity)
        asset.enumerateHierarchy { node, _ in
            guard node.geometry != nil else { return }
            let (lo, hi) = node.boundingBox
            for corner in Self.corners(lo, hi) {
                let p = node.simdConvertPosition(corner, to: root)
                lower = simd_min(lower, SIMD3(Double(p.x), Double(p.y), Double(p.z))); upper = simd_max(upper, SIMD3(Double(p.x), Double(p.y), Double(p.z)))
            }
        }
        guard lower.x.isFinite else { return }
        let center = (lower + upper) / 2
        content.simdPosition = SIMD3(Float(-center.x), Float(-center.y), Float(-center.z))
    }

    static func corners(_ lo: SCNVector3, _ hi: SCNVector3) -> [SIMD3<Float>] {
        let a = SIMD3<Float>(Float(lo.x), Float(lo.y), Float(lo.z)), b = SIMD3<Float>(Float(hi.x), Float(hi.y), Float(hi.z))
        return [SIMD3(a.x, a.y, a.z), SIMD3(b.x, a.y, a.z), SIMD3(a.x, b.y, a.z), SIMD3(b.x, b.y, a.z), SIMD3(a.x, a.y, b.z), SIMD3(b.x, a.y, b.z), SIMD3(a.x, b.y, b.z), SIMD3(b.x, b.y, b.z)]
    }

    /// restFrames: each display's plane in the body's frame (orbit excluded).
    func restFrames(_ panel: Int) -> [R9DuoRestFrame] {
        guard let cover = surfaces[1]?.first, let inner = surfaces[3], inner.count == 2 else { return [] }
        func frame(_ meshes: [SCNNode], _ face: String) -> R9DuoRestFrame {
            var lower = SIMD3<Double>(repeating: .infinity), upper = SIMD3<Double>(repeating: -.infinity)
            var normal = SIMD3<Double>.zero, up = SIMD3<Double>.zero
            for mesh in meshes {
                let (lo, hi) = mesh.boundingBox
                for corner in Self.corners(lo, hi) {
                    let p = rootLocal(mesh, corner)
                    lower = simd_min(lower, p); upper = simd_max(upper, p)
                }
                normal += rootDirection(mesh, Self.firstNormal(mesh.geometry))
                up += rootDirection(mesh, SIMD3(0, 1, 0))
            }
            return R9DuoRestFrame(face: face, normal: simd_normalize(normal), up: simd_normalize(up), center: (lower + upper) / 2)
        }
        if panel == 1 { return [frame([cover], "cover")] }
        let inside = frame(inner, "inside")
        return appliedAngle > 20 && appliedAngle < 165 ? [inside, frame([inner[0]], "left"), frame([inner[1]], "right")] : [inside]
    }

    /// A point in the body's frame: the root's own rotation and position removed.
    private func rootLocal(_ node: SCNNode, _ point: SIMD3<Float>) -> SIMD3<Double> {
        let p = node.simdConvertPosition(point, to: root)
        return SIMD3(Double(p.x), Double(p.y), Double(p.z))
    }
    private func rootDirection(_ node: SCNNode, _ direction: SIMD3<Double>) -> SIMD3<Double> {
        let d = node.simdConvertVector(SIMD3(Float(direction.x), Float(direction.y), Float(direction.z)), to: root)
        return simd_normalize(SIMD3(Double(d.x), Double(d.y), Double(d.z)))
    }
    static func firstNormal(_ geometry: SCNGeometry?) -> SIMD3<Double> {
        guard let source = geometry?.sources(for: .normal).first, source.vectorCount > 0 else { return SIMD3(0, 0, 1) }
        return source.data.withUnsafeBytes { raw in
            let base = source.dataOffset
            let read = { (k: Int) -> Double in source.bytesPerComponent == 8 ? raw.load(fromByteOffset: base + k * 8, as: Double.self) : Double(raw.load(fromByteOffset: base + k * 4, as: Float.self)) }
            return SIMD3(read(0), read(1), read(2))
        }
    }

    // MARK: Views and snapping (createDuoViewer)

    private var now: Double { CACurrentMediaTime() * 1000 }
    private var reducedMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }
    private var activePanel: Int { screen?.duo?.screenId == 1 ? 1 : 3 }

    private func activeSnaps() -> [R9DuoViewSnap] { asset == nil ? [] : R9DuoSnaps.snaps(restFrames(activePanel), panel: activePanel) }
    private func snaps() -> [R9DuoViewSnap] {
        guard asset != nil, screen?.duo?.supportsPhysicalOrientation == true else { return activeSnaps() }
        let cover = R9DuoSnaps.snaps(restFrames(1), panel: 1)
        // A shut inner display is occluded and cannot become a useful rest view.
        return angle > 20 && angle < 180 ? R9DuoSnaps.snaps(restFrames(3), panel: 3) + cover : activeSnaps()
    }

    private func clearHandoff() { handoff?.cancel(); handoff = nil; requestedPanel = nil }

    private func choose(_ rotation: simd_quatd) -> simd_quatd {
        guard let snap = R9DuoSnaps.nearest(rotation, snaps()) else { return rotation }
        restFace = snap.face
        let panel = snap.face == "cover" ? 1 : 3
        if let screen, screen.duo?.supportsPhysicalOrientation == true, panel != (requestedPanel ?? screen.duo?.screenId) {
            clearHandoff()
            requestedPanel = panel
            captured = nil
            panelRequested(panel)
            let item = DispatchWorkItem { [weak self] in
                guard let self else { return }
                self.clearHandoff()
                if let confirmed = R9DuoSnaps.nearest(self.motion.rotation, self.activeSnaps()) { self.restFace = confirmed.face; self.motion.setPose(confirmed.rotation, self.now) }
                self.invalidate()
            }
            handoff = item
            DispatchQueue.main.asyncAfter(deadline: .now() + 5, execute: item)
        } else if screen != nil, requestedPanel == nil, snap.orientation != viewOrientation {
            viewOrientation = snap.orientation
            requestedOrientation = snap.orientation
            orientationRequested(snap.orientation)
        }
        return snap.rotation
    }

    func setScreen(_ next: R7DeviceClient.Screen?) {
        guard !disposed else { return }
        let wasMoving = moving || motion.needsFrame
        if R9Duo.displayKey(screen) != R9Duo.displayKey(next) { readyKey = ""; activationAt = now; captured = nil }
        let previous = screen
        let ownedHandoff = requestedPanel != nil
        if next == nil || next?.duo?.screenId == requestedPanel { clearHandoff() }
        let ownedRotation = requestedOrientation != nil && next?.duo?.screenId == previous?.duo?.screenId
        let changedDisplay = next?.duo?.screenId != previous?.duo?.screenId
        requestedOrientation = nil
        screen = next
        let changedPose = next?.duo?.hingePose != nil && next?.duo?.hingePose != previous?.duo?.hingePose
        let folding = hingeLeaf != nil && !changedPose
        let rotated = next?.duo?.screenId == previous?.duo?.screenId && next?.duo?.hingeAngle == previous?.duo?.hingeAngle && next?.orientation != previous?.orientation
        let leftPhysicalPose = rotated && !ownedRotation && !ownedHandoff && !folding && next?.duo?.hingePose == nil && previewAngle == nil
        if firstPose || changedPose || leftPhysicalPose {
            physicalPose = next?.duo?.hingePose ?? (next?.duo?.screenId == 1 ? "closed" : "open")
            presentationAngle = next?.duo?.hingeAngle ?? (next?.duo?.screenId == 1 ? 0 : 180)
        }
        targetAngle = previewAngle ?? next?.duo?.hingeAngle ?? (next?.duo?.screenId == 1 ? 0 : 180)
        if firstPose || changedPose || (rotated && !ownedRotation && !ownedHandoff && !folding) {
            hingeLeaf = nil
            viewOrientation = next?.orientation
            targetPresentation = R9DuoSnaps.presentation(pose: physicalPose, angle: presentationAngle, screen: next)
            if next?.duo?.screenId == 1, let snap = R9DuoSnaps.nearest(targetPresentation, activeSnaps()) { targetPresentation = snap.rotation }
            if firstPose { angle = targetAngle }
            restFace = next?.duo?.screenId == 1 ? "cover" : physicalPose == "laptop" ? "right" : "inside"
            motion.setPose(targetPresentation, now, immediate: firstPose)
            firstPose = false
        } else if next != nil, changedDisplay, !ownedHandoff, !folding, let snap = R9DuoSnaps.nearest(motion.rotation, activeSnaps()) {
            restFace = snap.face
            motion.setPose(snap.rotation, now)
        }
        if !wasMoving { lastTime = now }
        applyPose(); fit(); invalidate()
    }

    func setHingePreview(_ next: Double?) {
        guard !disposed else { return }
        if let next, !next.isFinite || next < 0 || next > 180 { return }
        if !moving { lastTime = now }
        previewAngle = next
        if next != nil, hingeLeaf == nil, asset != nil {
            clearHandoff(); requestedOrientation = nil
            hingeLeaf = screen?.duo?.screenId == 1 && angle > 20 ? "left" : "right"
            motion.setPose(motion.rotation, now, immediate: true)
        }
        motion.hold(interactionActive || next != nil, now)
        targetAngle = next ?? screen?.duo?.hingeAngle ?? (screen?.duo?.screenId == 1 ? 0 : 180)
        if let next { angle = next; applyPose(); fit() }
        captured = nil
        invalidate()
    }

    /// The control queue's preview (`requested` angle), unless a pinch is in progress.
    func setControlPreview(_ value: Double?) {
        controlPreview = value
        if !pinch.active { setHingePreview(value) }
    }

    func rejectOrientation() {
        guard !disposed, requestedOrientation != nil || requestedPanel != nil else { return }
        clearHandoff(); requestedOrientation = nil
        viewOrientation = screen?.orientation
        if let snap = R9DuoSnaps.nearest(motion.rotation, activeSnaps().filter { $0.orientation == screen?.orientation }) { restFace = snap.face; motion.setPose(snap.rotation, now) }
        invalidate()
    }

    func resetPose() {
        guard !disposed else { return }
        hingeLeaf = nil; requestedOrientation = nil
        clearHandoff()
        let snap = activeSnaps().first { $0.orientation == screen?.orientation }
        if let snap, snap.orientation != viewOrientation {
            viewOrientation = snap.orientation; requestedOrientation = snap.orientation
            orientationRequested(snap.orientation)
        }
        motion.reset(snap?.rotation ?? targetPresentation, now)
        applyPose(); fit(); invalidate()
    }

    func cancelInput() { hingeLeaf = nil; captured = nil }

    /// frameUpdated: the active panel's frame painted into its surface; a native shutdown blank is
    /// ignored for 1.5 s after the display changes.
    func frameUpdated(_ panel: Int, _ image: CGImage) {
        guard !disposed, let screen, screen.duo?.screenId == panel else { return }
        guard R9Duo.frameMatches(width: Double(image.width), height: Double(image.height), screen: screen) else { return }
        if readyKey.isEmpty, now - activationAt < 1500, R9Duo.isBlank(image) { return }
        guard let painted = R9Duo.paint(image, panel: panel) else { return }
        SCNTransaction.begin(); SCNTransaction.disableActions = true
        materials[panel]?.diffuse.contents = painted
        SCNTransaction.commit()
        readyKey = R9Duo.displayKey(screen); primaryKey = readyKey
        invalidate()
    }

    private var moving: Bool { abs(angle - targetAngle) > 0.01 || simd_distance(pivot, targetPivot) > 0.001 }

    private func applyPose() {
        guard asset != nil else { return }
        let before = hingeLeaf != nil && appliedAngle != angle ? leafRotation(hingeLeaf!) : nil
        setAngle(angle)
        appliedAngle = angle
        if let before, let leaf = hingeLeaf {
            // The primary surface stays in camera space; its partner supplies the fold.
            let correction = before * leafRotation(leaf).inverse
            if R7Quat.angle(correction, R7Quat.identity) > 1e-8 { motion.setPose(motion.rotation * correction, now, immediate: true) }
        }
        let q = motion.rotation
        root.simdOrientation = simd_quatf(ix: Float(q.imag.x), iy: Float(q.imag.y), iz: Float(q.imag.z), r: Float(q.real))
        root.simdPosition = .zero
        let frames = restFrames(restFace == "cover" ? 1 : 3)
        targetPivot = (frames.first { $0.face == restFace } ?? frames.first)?.center ?? .zero
        let offset = q.act(pivot)
        root.simdPosition = SIMD3(Float(-offset.x), Float(-offset.y), Float(-offset.z))
    }

    private func leafRotation(_ leaf: String) -> simd_quatd {
        guard let node = leaf == "left" ? leftHalf : rightHalf else { return R7Quat.identity }
        let m = node.simdConvertTransform(matrix_identity_float4x4, to: root)
        let q = simd_quatf(simd_float3x3(SIMD3(m.columns.0.x, m.columns.0.y, m.columns.0.z), SIMD3(m.columns.1.x, m.columns.1.y, m.columns.1.z), SIMD3(m.columns.2.x, m.columns.2.y, m.columns.2.z)))
        return simd_quatd(ix: Double(q.imag.x), iy: Double(q.imag.y), iz: Double(q.imag.z), r: Double(q.real)).normalized
    }

    private func fit(immediate: Bool = false) {
        guard asset != nil, bounds.width > 0, bounds.height > 0 else { return }
        var lower = SIMD3<Double>(repeating: .infinity), upper = SIMD3<Double>(repeating: -.infinity)
        root.enumerateHierarchy { node, _ in
            guard node.geometry != nil, !node.isHidden else { return }
            let (lo, hi) = node.boundingBox
            for corner in Self.corners(lo, hi) {
                let w = node.simdConvertPosition(corner, to: nil)
                lower = simd_min(lower, SIMD3(Double(w.x), Double(w.y), Double(w.z))); upper = simd_max(upper, SIMD3(Double(w.x), Double(w.y), Double(w.z)))
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
        guard !disposed, timer == nil else { return }
        let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    private func tick() {
        guard !disposed else { return }
        let time = now, reduced = reducedMotion
        let elapsed = max(0, (time - lastTime) / 1000)
        let amount = reduced ? 1 : 1 - exp(-14 * (elapsed > 0 ? elapsed : 0.016))
        lastTime = time
        let inMotion = moving
        let orbitChanged = motion.advance(time, reduced: reduced)
        if inMotion {
            angle += (targetAngle - angle) * amount
            if abs(angle - targetAngle) <= 0.01 { angle = targetAngle }
            applyPose()
            pivot += (targetPivot - pivot) * amount
            if simd_distance(pivot, targetPivot) <= 0.001 { pivot = targetPivot }
            applyPose(); fit(immediate: reduced || elapsed > 0.5)
        }
        if orbitChanged { applyPose(); fit(immediate: reduced || elapsed > 0.5) }
        if framing.advance(time, immediate: reduced) { applyCamera() }
        scnView.needsDisplay = true
        if !(inMotion || moving || motion.needsFrame || framing.needsFrame) { timer?.invalidate(); timer = nil }
    }

    // MARK: Input (DeviceDuoViewport, createPhoneInteraction, duoScene.screenPoint / hingeLeafAt)

    private func viewPoint(_ x: Double, _ y: Double) -> CGPoint { CGPoint(x: x * Double(bounds.width), y: (1 - y) * Double(bounds.height)) }

    /// screenPoint: a new contact must land first on the active display; a captured drag stays on its plane.
    func screenPoint(_ x: Double, _ y: Double, captured isCaptured: Bool) -> CGPoint? {
        guard !disposed, !moving, previewAngle == nil, requestedOrientation == nil, requestedPanel == nil, let screen else { return nil }
        applyPose()
        let key = R9Duo.displayKey(screen)
        guard !readyKey.isEmpty, key == readyKey else { captured = nil; return nil }
        let point = viewPoint(x, y)
        var node: SCNNode, panel: Int, local: SIMD3<Float>
        if isCaptured, let current = captured, current.key == key {
            let near = scnView.unprojectPoint(SCNVector3(point.x, point.y, 0)), far = scnView.unprojectPoint(SCNVector3(point.x, point.y, 1))
            let a = current.node.simdConvertPosition(SIMD3(Float(near.x), Float(near.y), Float(near.z)), from: nil)
            let b = current.node.simdConvertPosition(SIMD3(Float(far.x), Float(far.y), Float(far.z)), from: nil)
            let plane = Float(current.node.boundingBox.min.z)
            guard abs(b.z - a.z) > 1e-9 else { return nil }
            local = a + (b - a) * ((plane - a.z) / (b.z - a.z))
            node = current.node; panel = current.panel
        } else {
            // The chassis occludes rear displays: only the first visible hit can own a contact.
            guard let hit = scnView.hitTest(point, options: [.searchMode: SCNHitTestSearchMode.closest.rawValue, .backFaceCulling: false, .rootNode: root]).first else { return nil }
            guard let found = [1, 3].first(where: { surfaces[$0]?.contains { $0 === hit.node } == true }), found == screen.duo?.screenId else { return nil }
            node = hit.node; panel = found
            local = node.simdConvertPosition(hit.simdWorldCoordinates, from: nil)
            captured = (panel, node, key)
        }
        guard let box = bounds3[panel] else { return nil }
        let u = (Double(local.x) - box.lower.x) / box.size.x, v = (Double(local.y) - box.lower.y) / box.size.y
        return R9Duo.rawPoint(panel: panel, x: panel == 1 ? 1 - u : u, y: 1 - v)
    }

    /// beginHinge: a pinch over either leaf holds the leaf the primary display lives on.
    func beginHinge(_ x: Double, _ y: Double) -> Bool {
        guard !disposed, asset != nil, screen != nil else { return false }
        applyPose()
        guard var node = scnView.hitTest(viewPoint(x, y), options: [.searchMode: SCNHitTestSearchMode.closest.rawValue, .rootNode: root]).first?.node else { return false }
        var leaf: String?
        while true {
            if node === leftHalf { leaf = "left"; break }
            if node === rightHalf { leaf = "right"; break }
            guard let parent = node.parent else { break }
            node = parent
        }
        guard leaf != nil else { return false }
        clearHandoff(); requestedOrientation = nil
        hingeLeaf = screen?.duo?.screenId == 1 && angle > 20 ? "left" : "right"
        motion.setPose(motion.rotation, now, immediate: true)
        captured = nil
        return true
    }

    private func setInteractionActive(_ on: Bool, _ mode: String) {
        if mode == "orbit" { if on { hingeLeaf = nil }; motion.dragActive(on, now) }
        else { interactionActive = on; motion.hold(on || previewAngle != nil, now); framing.hold(on, now) }
        invalidate()
    }

    private func orbit(_ dx: Double, _ dy: Double) {
        hingeLeaf = nil
        motion.orbit(dx * Double(bounds.width), dy * Double(bounds.height), now)
        invalidate()
    }

    private func normalized(_ event: NSEvent) -> CGPoint {
        let point = convert(event.locationInWindow, from: nil)
        return CGPoint(x: point.x / max(1, bounds.width), y: point.y / max(1, bounds.height))
    }

    override func mouseDown(with event: NSEvent) {
        superview?.window?.makeFirstResponder(superview)
        guard active == nil else { return }
        endScroll()
        let point = normalized(event)
        let hit = event.modifierFlags.contains(.option) ? nil : screenPoint(Double(point.x), Double(point.y), captured: false)
        active = (hit == nil ? "orbit" : "touch", point, hit)
        setInteractionActive(true, active!.mode)
        if let hit { touch("begin", Double(hit.x), Double(hit.y)) }
    }
    override func mouseDragged(with event: NSEvent) {
        guard var current = active else { return }
        let point = normalized(event)
        if current.mode == "touch" {
            if let hit = screenPoint(Double(point.x), Double(point.y), captured: true) { current.screen = hit; touch("move", Double(hit.x), Double(hit.y)) }
        } else { orbit(Double(point.x - current.last.x), Double(point.y - current.last.y)) }
        current.last = point
        active = current
    }
    override func mouseUp(with event: NSEvent) { mouseDragged(with: event); end() }
    func end() {
        guard let previous = active else { return }
        active = nil
        if previous.mode == "touch", let hit = previous.screen { touch("end", Double(hit.x), Double(hit.y)) }
        setInteractionActive(false, previous.mode)
    }

    /// phoneTrackpad with a pinch: two-finger swipes turn the device; a pinch over it moves the hinge.
    override func scrollWheel(with event: NSEvent) {
        if event.momentumPhase != [] { return endScroll() }
        if event.phase == .ended || event.phase == .cancelled { return endScroll() }
        guard active == nil, bounds.width > 0, !pinch.active else { return }
        let unit = event.hasPreciseScrollingDeltas ? 1.0 : 16.0
        let x = max(-0.25, min(0.25, Double(event.scrollingDeltaX) * unit / Double(bounds.width)))
        let y = max(-0.25, min(0.25, Double(event.scrollingDeltaY) * unit / Double(bounds.height)))
        guard x != 0 || y != 0 else { return }
        scrolling = true
        orbit(x, y)
    }
    private func endScroll() { guard scrolling else { return }; scrolling = false; if active == nil { setInteractionActive(false, "orbit") } }

    override func magnify(with event: NSEvent) {
        switch event.phase {
        case .began:
            end(); endScroll()
            let point = normalized(event)
            _ = pinch.begin(Double(point.x), Double(point.y))
        case .changed: pinch.move(log(max(1e-6, 1 + Double(event.magnification))))
        default: pinch.end()
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        NotificationCenter.default.removeObserver(self)
        if let window { NotificationCenter.default.addObserver(self, selector: #selector(resigned), name: NSWindow.didResignKeyNotification, object: window) }
    }
    @objc private func resigned() { end(); endScroll(); cancelInput() }

    func dispose() { disposed = true; clearHandoff(); timer?.invalidate(); timer = nil; active = nil; scnView.scene = nil }

    /// Test seams.
    func snapshot() -> NSImage { scnView.snapshot() }
    var hingeAngle: Double { angle }
    var orbitRotation: simd_quatd { motion.rotation }
    var isLoaded: Bool { asset != nil }
    var readyDisplay: String { readyKey }
}

extension R9Duo {
    static var roomIntensity: CGFloat = 1
    static var roomWall: CGFloat = 0.45, roomPanel: CGFloat = 1
    /// An equirectangular stand-in for three.js's RoomEnvironment: grey walls, a darker floor and
    /// emissive panels overhead and to the sides, blurred as PMREM blurs them.
    static func room() -> CGImage? {
        let width = 256, height = 128
        guard let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
        let wall = roomWall
        let gradient = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB)!, colors: [CGColor(gray: min(1, wall * 1.25), alpha: 1), CGColor(gray: wall, alpha: 1), CGColor(gray: wall * 0.55, alpha: 1)] as CFArray, locations: [0, 0.5, 1])!
        context.drawLinearGradient(gradient, start: CGPoint(x: 0, y: height), end: CGPoint(x: 0, y: 0), options: [])
        context.setFillColor(CGColor(gray: roomPanel, alpha: 1))
        for (x, y, w, h) in [(0.2, 0.86, 0.18, 0.08), (0.62, 0.86, 0.18, 0.08), (0.05, 0.55, 0.06, 0.2), (0.48, 0.55, 0.06, 0.2), (0.86, 0.6, 0.06, 0.16)] {
            context.fill(CGRect(x: x * Double(width), y: y * Double(height), width: w * Double(width), height: h * Double(height)))
        }
        guard let image = context.makeImage() else { return nil }
        let blurred = CIImage(cgImage: image).clampedToExtent().applyingGaussianBlur(sigma: 6).cropped(to: CGRect(x: 0, y: 0, width: width, height: height))
        return CIContext().createCGImage(blurred, from: blurred.extent)
    }

    /// A surface's #080a10 fill before its first frame.
    static func blank(_ size: CGSize) -> CGImage? {
        guard let context = CGContext(data: nil, width: Int(size.width), height: Int(size.height), bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
        context.setFillColor(CGColor(srgbRed: 8 / 255, green: 10 / 255, blue: 16 / 255, alpha: 1))
        context.fill(CGRect(origin: .zero, size: size))
        return context.makeImage()
    }
}

#endif
