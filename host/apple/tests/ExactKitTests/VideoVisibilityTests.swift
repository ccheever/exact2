#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class VideoVisibilityTests: XCTestCase {
    func testNestedClippingUsesAreaAndActualScrollOffsets() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 300),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 300))
        window.contentView = root
        let clip = NSClipView(frame: NSRect(x: 0, y: 0, width: 200, height: 200))
        let document = NSView(frame: NSRect(x: 0, y: 0, width: 600, height: 600))
        let target = NSView(frame: NSRect(x: 0, y: 0, width: 200, height: 200))
        root.addSubview(clip); clip.documentView = document; document.addSubview(target)
        clip.scroll(to: .zero)
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 1, accuracy: 0.001)
        clip.scroll(to: NSPoint(x: 100, y: 100))
        XCTAssertEqual(clip.bounds.origin, NSPoint(x: 100, y: 100))
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0.25, accuracy: 0.001)
        clip.scroll(to: NSPoint(x: 200, y: 100))
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0, accuracy: 0.001)
        clip.scroll(to: .zero)
        target.isHidden = true
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0)
        target.isHidden = false
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 1, accuracy: 0.001)
        target.removeFromSuperview()
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0)
    }
}
#endif

#if os(iOS)
import AVFoundation
import UIKit
import XCTest
@testable import ExactKit

final class AudioControlsIOSTests: XCTestCase {
    func testCompactAudioControlsPlayPauseAndSeek() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "UIKit control actions need an app's event dispatcher")
        }
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("audio-controls-\(UUID())", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        AppFiles.learn(["roots": ["audio-controls": root.path]])
        defer { try? FileManager.default.removeItem(at: root) }
        let url = root.appendingPathComponent("take.wav")
        defer { try? FileManager.default.removeItem(at: url) }
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 1))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 144000))
        buffer.frameLength = buffer.frameCapacity
        let samples = try XCTUnwrap(buffer.floatChannelData?[0])
        samples.initialize(repeating: 0, count: Int(buffer.frameLength))
        do {
            let file = try AVAudioFile(forWriting: url, settings: format.settings)
            try file.write(from: buffer)
        }
        let p = Presenter()
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 300, height: 54)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { p.reset(); window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "video", "props": ["semanticTag": "audio", "src": "app:/audio-controls/take.wav", "controls": "true", "muted": "true"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 54.0],
        ]))
        let owner = try XCTUnwrap(p.views[1])
        let player = try XCTUnwrap(owner.video)
        func descendants(_ view: UIView) -> [UIView] { view.subviews.flatMap { [$0] + descendants($0) } }
        let button = try XCTUnwrap(descendants(owner).compactMap { $0 as? UIButton }.first)
        let slider = try XCTUnwrap(descendants(owner).compactMap { $0 as? UISlider }.first)
        func until(_ condition: () -> Bool) -> Bool {
            let deadline = Date().addingTimeInterval(10)
            while !condition(), Date() < deadline { _ = RunLoop.main.run(mode: .default, before: deadline) }
            return condition()
        }
        XCTAssertTrue(until { button.isEnabled })
        window.layoutIfNeeded()
        owner.layoutIfNeeded()
        XCTAssertEqual(player.state()["renderer"] as? String, "UIKit")
        XCTAssertEqual(button.accessibilityLabel, "Play")
        XCTAssertGreaterThanOrEqual(button.bounds.height, 44)
        XCTAssertTrue(owner.bounds.contains(button.convert(button.bounds, to: owner)), "the button fits in audio's default box")
        XCTAssertTrue(owner.bounds.contains(slider.convert(slider.bounds, to: owner)))
        XCTAssertTrue(button.hitTest(CGPoint(x: button.bounds.midX, y: button.bounds.midY), with: nil) === button)
        button.sendActions(for: .touchUpInside)
        XCTAssertTrue(until { player.state()["paused"] as? Bool == false })
        button.sendActions(for: .touchUpInside)
        XCTAssertTrue(until { player.state()["paused"] as? Bool == true })
        slider.value = 1.5
        slider.sendActions(for: .valueChanged)
        XCTAssertTrue(until { abs((player.state()["currentTime"] as? Double ?? 0) - 1.5) < 0.05 })
        XCTAssertEqual(button.accessibilityLabel, "Play")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["controls": "false"]]]))
        XCTAssertTrue(button.superview?.isHidden == true)
    }
}
#endif

#if os(iOS)
final class HDRVideoIOSTests: XCTestCase {
    private var fixtures: URL { Bundle.module.url(forResource: "ColorFixtures", withExtension: nil)! }

    func testVideoFixturesRetainTheirTransferAndDecodeEveryFrame() throws {
        for (name, hdr, transfer, primaries) in [
            ("sdr.mp4", false, "ITU_R_709_2", "ITU_R_709_2"),
            ("pq.mov", true, "SMPTE_ST_2084_PQ", "ITU_R_2020"),
            ("hlg.mov", true, "ITU_R_2100_HLG", "ITU_R_2020"),
            ("p3-sdr.mov", false, "ITU_R_709_2", "P3_D65")
        ] {
            let asset = AVURLAsset(url: fixtures.appendingPathComponent(name))
            let track = try XCTUnwrap(asset.tracks(withMediaType: .video).first, name)
            XCTAssertEqual(track.hasMediaCharacteristic(.containsHDRVideo), hdr, name)
            let format = try XCTUnwrap(track.formatDescriptions.first) as! CMFormatDescription
            let properties = try XCTUnwrap(CMFormatDescriptionGetExtensions(format)) as NSDictionary
            XCTAssertEqual(properties[kCMFormatDescriptionExtension_TransferFunction] as? String, transfer, name)
            XCTAssertEqual(properties[kCMFormatDescriptionExtension_ColorPrimaries] as? String, primaries, name)
            let reader = try AVAssetReader(asset: asset)
            let output = AVAssetReaderTrackOutput(track: track, outputSettings: [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_64RGBAHalf])
            reader.add(output)
            XCTAssertTrue(reader.startReading(), name)
            var frames = 0
            while let sample = output.copyNextSampleBuffer() {
                let pixels = try XCTUnwrap(CMSampleBufferGetImageBuffer(sample), name)
                XCTAssertEqual(CVPixelBufferGetPixelFormatType(pixels), kCVPixelFormatType_64RGBAHalf)
                frames += 1
            }
            XCTAssertEqual(reader.status, .completed, "\(name): \(String(describing: reader.error))")
            XCTAssertEqual(frames, 90, name)
        }
    }

    func testLiveLimitsReachInlineAndNativeControls() throws {
        guard #available(iOS 26, *) else { throw XCTSkip("AVKit range needs iOS 26") }
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 360, height: 200))
        let controller = UIViewController()
        window.rootViewController = controller
        p.viewport.frame = window.bounds
        controller.view.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { p.reset(); window.isHidden = true }
        for id in [1, 2] {
            p.apply(wireBatch([
                ["op": "create", "id": id, "kind": "video", "props": ["src": fixtures.appendingPathComponent("pq.mov").absoluteString, "controls": id == 2 ? "true" : "false", "muted": "true", "paused": "true", "playsinline": "true"]],
                ["op": "roots", "ids": Array(1...id)],
                ["op": "frame", "id": id, "x": Double((id - 1) * 180), "y": 0.0, "w": 180.0, "h": 100.0]
            ]))
        }
        for limit in ["standard", "constrained", "no-limit", "standard"] {
            for id in [1, 2] {
                p.apply(wireBatch([["op": "style", "id": id, "style": ["dynamic_range_limit": limit]]]))
                let player = try XCTUnwrap(p.views[UInt32(id)]?.video)
                let state = player.state()
                let actual = try XCTUnwrap(state["appliedDynamicRange"] as? [String: String])
                XCTAssertEqual(actual[id == 1 ? "layer" : "controller"], limit)
                XCTAssertEqual(state["dynamicRange"] as? String, limit)
            }
        }
    }
}
#endif
