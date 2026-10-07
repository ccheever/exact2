#if os(iOS)
// @ref llp/1106.003-pairing-and-transport.decision.md#mobile-adaptations
// T3 Code mobile 365aa87982, ConnectionsNewRouteScreen: QR-only capture,
// raw payload delivery and a 600ms scan lock. This app view uses real camera input.
import AVFoundation
import UIKit

final class T3QRScanner: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let instance = T3QRScanner(events: events)
        try instance.setProps(props)
        return instance
    }

    /// Call from the app module's camera permission operation before opening the scanner.
    /// Agent requests never display an OS permission prompt. Simulator capture is unavailable.
    static func requestCameraPermission(agent: Bool, completion: @escaping (String) -> Void) {
        #if targetEnvironment(simulator)
        completion("unavailable")
        #else
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: completion("granted")
        case .denied: completion("denied")
        case .restricted: completion("restricted")
        case .notDetermined:
            guard !agent else { completion("undetermined"); return }
            guard Bundle.main.object(forInfoDictionaryKey: "NSCameraUsageDescription") as? String != nil else {
                completion("error:Camera permission is not configured in this build."); return
            }
            AVCaptureDevice.requestAccess(for: .video) { granted in
                DispatchQueue.main.async { completion(granted ? "granted" : "denied") }
            }
        @unknown default: completion("restricted")
        }
        #endif
    }

    private let preview = T3CameraPreview()
    private let session = AVCaptureSession()
    private let captureQueue = DispatchQueue(label: "com.exact.t3code.ios.qr")
    private let detector = T3QRDetector()
    // Main-thread presentation ownership. Capture configuration lives only on captureQueue.
    private var alive = true
    private var active = false
    private var lastPayloadAt: TimeInterval = -.infinity
    private var observers: [NSObjectProtocol] = []
    private var configured = false
    private var captureDestroyed = false
    override var view: UIView { preview }

    override init(events: ExactNativeEvents) {
        super.init(events: events)
        preview.layerView.session = session
        preview.layerView.videoGravity = .resizeAspectFill
        preview.backgroundColor = .black
        preview.isAccessibilityElement = true
        preview.accessibilityLabel = "Scan environment pairing QR code"
        detector.received = { [weak self] payload in
            guard let self, self.alive, self.active, UIApplication.shared.applicationState == .active else { return }
            let now = ProcessInfo.processInfo.systemUptime
            guard now - self.lastPayloadAt >= 0.6 else { return }
            self.lastPayloadAt = now
            self.events.message(payload)
        }
        observers.append(NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification,
            object: nil, queue: .main) { [weak self] _ in self?.stopCapture() })
        observers.append(NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification,
            object: nil, queue: .main) { [weak self] _ in
                guard let self, self.alive, self.active else { return }
                self.startCapture()
            })
        observers.append(NotificationCenter.default.addObserver(forName: .AVCaptureSessionRuntimeError,
            object: session, queue: .main) { [weak self] notification in
                guard let self, self.alive, self.active else { return }
                let error = notification.userInfo?[AVCaptureSessionErrorKey] as? Error
                self.events.change("error:\(error?.localizedDescription ?? "The camera could not start.")")
            })
    }

    override func setProps(_ props: [String: String]) throws {
        let next = props["active"] ?? "true"
        guard next == "true" || next == "false" else {
            throw ExactNativeRefusal("QR scanner active must be true or false.")
        }
        let shouldRun = next == "true"
        guard shouldRun != active else { return }
        active = shouldRun
        lastPayloadAt = -.infinity
        if shouldRun { startCapture() } else { stopCapture() }
    }

    private func sendStatus(_ status: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.alive, self.active else { return }
            self.events.change(status)
        }
    }

    private func startCapture() {
        #if targetEnvironment(simulator)
        sendStatus("unavailable")
        #else
        guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized else {
            let status = AVCaptureDevice.authorizationStatus(for: .video)
            sendStatus(status == .restricted ? "restricted" : status == .notDetermined ? "undetermined" : "denied")
            return
        }
        captureQueue.async { [weak self] in
            guard let self, !self.captureDestroyed else { return }
            if !self.configured {
                do {
                    let camera = try self.configureSession()
                    self.configured = true
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.alive else { return }
                        self.preview.configureRotation(camera)
                    }
                } catch {
                    self.sendStatus("error:\(error.localizedDescription)")
                    return
                }
            }
            if !self.session.isRunning { self.session.startRunning() }
            self.sendStatus("granted")
        }
        #endif
    }

    private func configureSession() throws -> AVCaptureDevice {
        guard let camera = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back) else {
            throw T3CameraError("This device has no available back camera.")
        }
        let input = try AVCaptureDeviceInput(device: camera)
        let output = AVCaptureMetadataOutput()
        session.beginConfiguration()
        defer { session.commitConfiguration() }
        session.sessionPreset = .high
        guard session.canAddInput(input) else { throw T3CameraError("The camera input is unavailable.") }
        session.addInput(input)
        guard session.canAddOutput(output) else {
            session.removeInput(input)
            throw T3CameraError("QR capture is unavailable on this device.")
        }
        session.addOutput(output)
        guard output.availableMetadataObjectTypes.contains(.qr) else {
            session.removeOutput(output); session.removeInput(input)
            throw T3CameraError("QR capture is unavailable on this device.")
        }
        output.setMetadataObjectsDelegate(detector, queue: .main)
        output.metadataObjectTypes = [.qr]
        return camera
    }

    private func stopCapture() {
        captureQueue.async { [weak self] in
            guard let self else { return }
            if self.session.isRunning { self.session.stopRunning() }
        }
    }

    override func destroy() {
        alive = false; active = false
        detector.received = nil
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
        preview.stopRotation()
        preview.layerView.session = nil
        // Retain until the queued stop completes, even if Exact releases the instance now.
        captureQueue.async { [self] in
            captureDestroyed = true
            if session.isRunning { session.stopRunning() }
            session.beginConfiguration()
            for output in session.outputs { session.removeOutput(output) }
            for input in session.inputs { session.removeInput(input) }
            session.commitConfiguration()
        }
    }
}

private final class T3QRDetector: NSObject, AVCaptureMetadataOutputObjectsDelegate {
    var received: ((String) -> Void)?
    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard let code = objects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject })
            .first(where: { $0.type == .qr }), let payload = code.stringValue else { return }
        received?(payload)
    }
}

// Mirrors expo-camera 58.0.7's iOS17+ RotationCoordinator policy, from the
// dependency installed by the pinned mobile lockfile. No guessed orientation map.
private final class T3CameraPreview: UIView {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    var layerView: AVCaptureVideoPreviewLayer { layer as! AVCaptureVideoPreviewLayer }
    private var rotation: AVCaptureDevice.RotationCoordinator?
    private var observation: NSKeyValueObservation?

    func configureRotation(_ device: AVCaptureDevice) {
        stopRotation()
        let coordinator = AVCaptureDevice.RotationCoordinator(device: device, previewLayer: layerView)
        rotation = coordinator
        applyRotation()
        observation = coordinator.observe(\.videoRotationAngleForHorizonLevelPreview, options: [.new]) { [weak self] _, _ in
            DispatchQueue.main.async { self?.applyRotation() }
        }
    }
    func stopRotation() {
        observation?.invalidate(); observation = nil; rotation = nil
    }
    private func applyRotation() {
        guard let angle = rotation?.videoRotationAngleForHorizonLevelPreview,
              let connection = layerView.connection, connection.isVideoRotationAngleSupported(angle) else { return }
        connection.videoRotationAngle = angle
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        applyRotation()
    }
}

private struct T3CameraError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

#endif
