// The app's module: haptics (`later` with `{"op": "haptic", "kind": …}`,
// which Exact has no API for yet), `<progressive-blur>` and `<qr-scanner>`.
//
// `<qr-scanner>` on iOS (LLP 1024): the back camera behind an
// AVCaptureVideoPreviewLayer, reading QR codes with AVCaptureMetadataOutput,
// the way the Camera app does. Events: `change` with a code's text (once per
// distinct code, with the success haptic); `message` with the camera's state
// — `scanning`, `denied` (the person said no, or a restriction did) or
// `unavailable` (no camera: the Simulator). No props.
//
// Camera stays deferred in Exact itself (`rules/DEFERRED.md`); this is the
// app's own module declaring `device.camera` for one purpose: reading the
// pairing code Ocho's Pair Phone shows.
import Foundation

#if os(iOS)
import AVFoundation
import UIKit

final class OchoMobileModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        [
            "qr-scanner": ExactNativeFactory { _, events in QRScanner(events: events) },
            "progressive-blur": ExactNativeFactory { props, events in ProgressiveBlur(props: props, events: events) },
            "title-reveal": ExactNativeFactory { props, events in TitleReveal(props: props, events: events) },
            "glass-button": ExactNativeFactory { props, events in GlassButton(props: props, events: events) },
            "glass-composer": ExactNativeFactory { props, events in GlassComposer(props: props, events: events) },
        ]
    }

    override func later(_ request: [String: Any], reply: ExactReply) {
        guard request["op"] as? String == "haptic" else {
            reply.fail("Ocho answers no \(request["op"] ?? "op")")
            return
        }
        let kind = request["kind"] as? String ?? ""
        DispatchQueue.main.async {
            Haptics.play(kind)
            reply.send(["played": kind])
        }
    }
}
let exactModule: ExactModule.Type = OchoMobileModule.self

/// The system's feedback, by name: a notification (`success`, `warning`,
/// `error`), an impact (`light`, `medium`, `soft`, `rigid`), or `selection`.
enum Haptics {
    static func play(_ kind: String) {
        switch kind {
        case "success": UINotificationFeedbackGenerator().notificationOccurred(.success)
        case "warning": UINotificationFeedbackGenerator().notificationOccurred(.warning)
        case "error": UINotificationFeedbackGenerator().notificationOccurred(.error)
        case "light": UIImpactFeedbackGenerator(style: .light).impactOccurred()
        case "medium": UIImpactFeedbackGenerator(style: .medium).impactOccurred()
        case "soft": UIImpactFeedbackGenerator(style: .soft).impactOccurred()
        case "rigid": UIImpactFeedbackGenerator(style: .rigid).impactOccurred()
        case "selection": UISelectionFeedbackGenerator().selectionChanged()
        default: break
        }
    }
}

private final class PreviewView: UIView {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    var preview: AVCaptureVideoPreviewLayer { layer as! AVCaptureVideoPreviewLayer }
}

/// AVFoundation's delegate is an NSObject; the instance is not.
private final class Reader: NSObject, AVCaptureMetadataOutputObjectsDelegate {
    var found: ((String) -> Void)?
    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        for case let code as AVMetadataMachineReadableCodeObject in objects where code.type == .qr {
            if let text = code.stringValue, !text.isEmpty { found?(text); return }
        }
    }
}

final class QRScanner: ExactNativeInstance {
    private let preview = PreviewView()
    private let session = AVCaptureSession()
    private let reader = Reader()
    /// AVCaptureSession's configuration and start block; never the main thread.
    private let queue = DispatchQueue(label: "dev.getfirewood.ocho.qr-scanner")
    private var last = ""
    private var alive = true

    override init(events: ExactNativeEvents) {
        super.init(events: events)
        preview.backgroundColor = .black
        preview.preview.videoGravity = .resizeAspectFill
        preview.preview.session = session
        preview.isAccessibilityElement = true
        preview.accessibilityLabel = "Camera, looking for a pairing code"
        reader.found = { [weak self] text in self?.found(text) }
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            start()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { granted in
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    if granted { self.start() } else { self.events.message("denied") }
                }
            }
        default:
            events.message("denied")
        }
        events.load()
    }

    override var view: ExactNativeView { preview }

    private func start() {
        let session = self.session, reader = self.reader
        queue.async { [weak self] in
            let camera = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back)
                ?? AVCaptureDevice.default(for: .video)
            guard let camera, let input = try? AVCaptureDeviceInput(device: camera), session.canAddInput(input) else {
                DispatchQueue.main.async { self?.say("unavailable") }
                return
            }
            session.beginConfiguration()
            session.addInput(input)
            let output = AVCaptureMetadataOutput()
            if session.canAddOutput(output) {
                session.addOutput(output)
                output.setMetadataObjectsDelegate(reader, queue: .main)
                if output.availableMetadataObjectTypes.contains(.qr) { output.metadataObjectTypes = [.qr] }
            }
            session.commitConfiguration()
            session.startRunning()
            DispatchQueue.main.async { self?.say("scanning") }
        }
    }

    private func say(_ state: String) {
        if alive { events.message(state) }
    }

    private func found(_ text: String) {
        guard alive, text != last else { return }
        last = text
        Haptics.play("success")
        events.change(text)
    }

    override func destroy() {
        alive = false
        reader.found = nil
        let session = self.session
        queue.async { if session.isRunning { session.stopRunning() } }
    }
}
#else
final class OchoMobileModule: ExactModule {}
let exactModule: ExactModule.Type = OchoMobileModule.self
#endif
