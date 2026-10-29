// `exact-camera` on Apple (LLP 1133.008 D1, D2, §9): an
// `AVCaptureVideoPreviewLayer` on the track's capture session, attached on
// the main thread, turned by the rotation coordinator, mirrored for a front
// camera by its connection; under the agent, a layer showing the agent's
// picture, mirrored for a `user` camera. On iOS a tap focuses and exposes at
// that point and a pinch zooms, the track's `zoom`; `tapToFocus` and
// `pinchToZoom` written false turn each off. A written `object-fit` is the
// layer's gravity (`contain` the platform's unsaid default).
import AVFoundation
import ExactPackage
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

#if os(macOS)
final class ViewfinderBox: NSView {
    var laidOut: () -> Void = {}
    override var isFlipped: Bool { true }
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
    }
    required init?(coder: NSCoder) { nil }
    override func layout() { super.layout(); laidOut() }
}
#else
final class ViewfinderBox: UIView {
    var laidOut: () -> Void = {}
    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        clipsToBounds = true
    }
    required init?(coder: NSCoder) { nil }
    override func layoutSubviews() { super.layoutSubviews(); laidOut() }
}
#endif

final class Viewfinder: NSObject, ExactPackageView {
    let id: String
    private weak var module: CameraModule?
    private let events: ExactPackageEvents
    private let box = ViewfinderBox(frame: .zero)
    private var preview: AVCaptureVideoPreviewLayer?
    private var picture: CALayer?
    private var source: ExactVideoSource?
    private var rotation: AnyObject?
    private var observation: NSKeyValueObservation?
    private var fit = "contain", tapToFocus = true, pinchToZoom = true
    private var loaded = false
    #if os(iOS)
    private var zoomAtPinch: CGFloat = 1
    #endif

    init(id: String, module: CameraModule, events: ExactPackageEvents) {
        self.id = id; self.module = module; self.events = events
        super.init()
        box.laidOut = { [weak self] in self?.layOut() }
        #if os(iOS)
        box.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped(_:))))
        box.addGestureRecognizer(UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:))))
        #endif
    }

    var view: ExactPackagePlatformView { box }

    func setProps(_ props: [String: String]) throws {
        fit = props["object-fit"].flatMap { $0.isEmpty ? nil : $0 } ?? "contain"
        tapToFocus = props["tapToFocus"] != "false"
        pinchToZoom = props["pinchToZoom"] != "false"
        preview?.videoGravity = gravity
        picture?.contentsGravity = contentsGravity
        module?.props(id, props)
    }

    func destroy() {
        blank()
        module?.gone(id)
    }

    private var gravity: AVLayerVideoGravity {
        switch fit {
        case "cover": .resizeAspectFill
        case "fill": .resize
        default: .resizeAspect
        }
    }

    private var contentsGravity: CALayerContentsGravity {
        switch fit {
        case "cover": .resizeAspectFill
        case "fill": .resize
        default: .resizeAspect
        }
    }

    private var host: CALayer? { box.layer }

    /// The capture's frames, through a preview layer on its session.
    func show(source: ExactVideoSource) {
        if source === self.source, let preview {
            preview.isHidden = false
            return
        }
        blank()
        self.source = source
        let layer = AVCaptureVideoPreviewLayer(session: source.session)
        layer.videoGravity = gravity
        host?.addSublayer(layer)
        preview = layer
        if #available(iOS 17.0, macOS 14.0, *) {
            let coordinator = AVCaptureDevice.RotationCoordinator(device: source.device, previewLayer: layer)
            let turn = { [weak layer] (angle: CGFloat) in
                guard let connection = layer?.connection, connection.isVideoRotationAngleSupported(angle) else { return }
                connection.videoRotationAngle = angle
            }
            turn(coordinator.videoRotationAngleForHorizonLevelPreview)
            observation = coordinator.observe(\.videoRotationAngleForHorizonLevelPreview, options: [.new]) { c, _ in
                let angle = c.videoRotationAngleForHorizonLevelPreview
                DispatchQueue.main.async { turn(angle) }
            }
            rotation = coordinator
        }
        layOut()
        once()
    }

    /// The agent's picture, mirrored for a front camera (§9).
    func show(picture image: CGImage?, mirrored: Bool) {
        blank()
        let layer = CALayer()
        layer.contents = image
        layer.contentsGravity = contentsGravity
        layer.setAffineTransform(mirrored ? CGAffineTransform(scaleX: -1, y: 1) : .identity)
        host?.addSublayer(layer)
        picture = layer
        layOut()
        once()
    }

    /// Keep the capture connection through an interruption: removing it can
    /// end the interruption, whose unmute would reconnect it in the background.
    func mute() {
        preview?.isHidden = true
        picture?.isHidden = true
    }

    func blank() {
        observation = nil
        rotation = nil
        preview?.removeFromSuperlayer()
        preview = nil
        picture?.removeFromSuperlayer()
        picture = nil
        source = nil
    }

    private func once() {
        guard !loaded else { return }
        loaded = true
        events.load()
    }

    private func layOut() {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        preview?.frame = box.bounds
        picture?.frame = box.bounds
        CATransaction.commit()
    }

    #if os(iOS)
    @objc private func tapped(_ g: UITapGestureRecognizer) {
        guard tapToFocus, let preview, let source else { return }
        source.focus(at: preview.captureDevicePointConverted(fromLayerPoint: g.location(in: box)))
    }

    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        guard pinchToZoom, let source else { return }
        if g.state == .began { zoomAtPinch = source.device.videoZoomFactor }
        source.apply(["zoom": Double(zoomAtPinch * g.scale)])
    }
    #endif
}
