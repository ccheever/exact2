#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class NativeButtonRasterIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?
    private var face = ButtonFace()
    private var reportedImages: [UInt32: CGSize] = [:]
    override func tearDown() {
        session?.destroy()
        window?.isHidden = true
        session = nil; window = nil
        super.tearDown()
    }
    private func until(_ condition: () -> Bool, file: StaticString = #filePath, line: UInt = #line) {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !condition(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertTrue(condition(), file: file, line: line)
    }
    func testASkewedDecodeAndItsPlaceholderRenderThroughUIButtonConfiguration() throws {
        let format = UIGraphicsImageRendererFormat(); format.scale = 1
        let data = try XCTUnwrap(UIGraphicsImageRenderer(size: CGSize(width: 401, height: 127), format: format).image { context in
            UIColor.blue.setFill(); context.fill(CGRect(x: 0, y: 0, width: 401, height: 127))
        }.pngData())
        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: URL(fileURLWithPath: NSTemporaryDirectory()))
        defer { loader.shutdown() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 480, height: 220))
        self.window = window
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        let seed = NodeView(id: 1, kind: "image", presenter: p)
        p.views[1] = seed; p.viewport.addSubview(seed)
        seed.frame = CGRect(x: 0, y: 0, width: 8, height: 8)
        seed.applyStyle(["object_fit": "contain"])
        loader.load(seed, source: "data:image/png;base64," + data.base64EncodedString(), resolver: resolver)
        until { seed.raster != nil }
        let lease = try XCTUnwrap(seed.raster), cg = lease.image.image
        XCTAssertLessThan(cg.width, 401)
        XCTAssertGreaterThan(abs(CGFloat(cg.height) * 401 / CGFloat(cg.width) - 127), 0.001, "exercise the rounded-aspect conversion")
        var face = ButtonFace(); face.raster = true
        face.image = .init(view: 2, source: "probe.png", size: CGSize(width: 401, height: 127))
        let image = try XCTUnwrap(ButtonRasterIOS.image(face, lease: lease))
        XCTAssertEqual(image.size, CGSize(width: 401, height: 127))
        let button = UIButton(configuration: .plain())
        button.frame = CGRect(x: 0, y: 0, width: 460, height: 180)
        window.addSubview(button)
        for candidate in [image, UIImage(cgImage: cg).withRenderingMode(.alwaysOriginal)] {
            ButtonConfigurationIOS.apply(face, to: button, traits: button.traitCollection, accent: nil, raster: candidate)
            button.setNeedsLayout(); button.layoutIfNeeded()
            let context = try XCTUnwrap(CGContext(data: nil, width: 460, height: 180, bitsPerComponent: 8, bytesPerRow: 1840,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            button.layer.render(in: context)
            let pixels = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
            XCTAssertEqual(Array(UnsafeBufferPointer(start: pixels + 90 * 1840 + 230 * 4, count: 4)), [0, 0, 255, 255], "UIButton must display the converted image as well as its CGImage control")
        }
        let placeholder = try XCTUnwrap(ButtonRasterIOS.image(face, lease: nil))
        ButtonConfigurationIOS.apply(face, to: button, traits: button.traitCollection, accent: nil, raster: placeholder)
        button.setNeedsLayout(); button.layoutIfNeeded()
        XCTAssertEqual(button.imageView?.bounds.size, CGSize(width: 401, height: 127), "the transparent placeholder reserves the natural image size")
    }

    func testANativeFaceDecodesForItsImageRatherThanItsInitialButtonBounds() throws {
        let format = UIGraphicsImageRendererFormat(); format.scale = 1
        let data = try XCTUnwrap(UIGraphicsImageRenderer(size: CGSize(width: 1024, height: 256), format: format).image { context in
            for x in stride(from: 0, to: 1024, by: 4) {
                (x.isMultiple(of: 8) ? UIColor.red : UIColor.blue).setFill()
                context.fill(CGRect(x: x, y: 0, width: 4, height: 256))
            }
        }.pngData())
        let source = "data:image/png;base64," + data.base64EncodedString()
        let p = Presenter(), loader = RasterLoader(), resolver = AssetResolver(root: URL(fileURLWithPath: NSTemporaryDirectory()))
        defer { loader.shutdown() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        self.window = window
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        var face = ButtonFace(); face.title = "Photo"; face.raster = true
        face.image = .init(view: 2, source: source, size: nil)
        p.buttonFace = { _ in face }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "control", "props": ["type": "button"], "style": [:]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
        ]))
        let owner = try XCTUnwrap(p.views[1]), button = try XCTUnwrap(p.controls.controls[1] as? NativeButtonIOS)
        XCTAssertEqual(owner.bounds.width, 24)
        loader.load(owner, source: source, resolver: resolver)
        until { owner.raster != nil }
        let decoded = try XCTUnwrap(owner.raster?.image.image)
        XCTAssertEqual(decoded.width, 1024, "natural-size image needs its detail despite the initial image-free button bounds")
        XCTAssertEqual(decoded.height, 256)
        XCTAssertEqual(button.configuration?.image?.size, CGSize(width: 1024, height: 256))
        let context = try XCTUnwrap(CGContext(data: nil, width: 1024, height: 1, bitsPerComponent: 8, bytesPerRow: 4096,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(decoded, in: CGRect(x: 0, y: 0, width: 1024, height: 1))
        let pixels = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
        XCTAssertEqual(Array(UnsafeBufferPointer(start: pixels + 2 * 4, count: 4)), [255, 0, 0, 255])
        XCTAssertEqual(Array(UnsafeBufferPointer(start: pixels + 6 * 4, count: 4)), [0, 0, 255, 255])
    }

    func testARasterFaceLoadsChangesSizeAndColourAndReleasesWhenRemoved() throws {
        let red = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAUCAYAAADskT9PAAAAJUlEQVR4nO3OIQEAAAwEoetfeovxBoGnq1tKQEBAQEBAQEBgHXhQmvs9CPiiZwAAAABJRU5ErkJggg=="
        let blue = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADAAAAAYCAYAAAC8/X7cAAAAP0lEQVR4nO3PsQ0AMAjAMP5/mh7BYFXKkN2Zmd2/44AGNKABDWhAAxrQgAY0oAENaEADGtCABjSgAQ1oQANuPW9i93nzNlyCAAAAAElFTkSuQmCC"
        let session = ExactApp.shared.makeSession(label: "button-raster")
        self.session = session
        let p = session.presenter
        face.title = "Photo"; face.subtitle = "Choose a picture"; face.raster = true
        face.image = .init(view: 2, source: red, size: nil)
        p.buttonFace = { [weak self] _ in self?.face ?? ButtonFace() }
        // Native face data is tested at the host boundary; portable Contract
        // authoring refuses raster faces until all graphical hosts paint them.
        p.onIntrinsic = { [weak self, weak p] sizes in
            guard let self else { return }
            for (id, size) in sizes where id == face.image?.view {
                reportedImages[id] = size
                face.image?.size = size
                p?.controls.sync()
            }
        }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        self.window = window
        p.viewport.frame = window.bounds; window.addSubview(p.viewport); window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "control", "props": ["type": "button"], "style": [:]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 60.0],
        ]))
        let owner = try XCTUnwrap(p.views[1]), button = try XCTUnwrap(p.controls.controls[1] as? NativeButtonIOS)
        until { button.configuration?.image != nil && self.reportedImages[2] == CGSize(width: 32, height: 20) }
        XCTAssertEqual(button.configuration?.subtitle, "Choose a picture")
        XCTAssertEqual(button.configuration?.image?.size, CGSize(width: 32, height: 20))
        XCTAssertEqual(try pixel(button), [255, 0, 0, 255])
        let before = button.naturalSize
        face.image = .init(view: 3, source: blue, size: nil)
        p.controls.sync()
        until { self.reportedImages[3] == CGSize(width: 48, height: 24) && button.configuration?.image?.size == CGSize(width: 48, height: 24) }
        XCTAssertEqual(try pixel(button), [0, 0, 255, 255])
        XCTAssertGreaterThan(button.naturalSize.width, before.width)
        face.image = nil; face.raster = false
        p.controls.sync()
        XCTAssertNil(button.configuration?.image)
        XCTAssertNil(owner.raster)
        XCTAssertNil(owner.imageSource)
        XCTAssertEqual(session.rasters.diagnostics["subscribers"] as? UInt64, 0)
    }
    private func pixel(_ button: UIButton) throws -> [UInt8] {
        let image = try XCTUnwrap(button.configuration?.image)
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        UIGraphicsPushContext(ctx)
        image.draw(in: CGRect(x: 0, y: 0, width: 1, height: 1))
        UIGraphicsPopContext()
        let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        return Array(UnsafeBufferPointer(start: bytes, count: 4))
    }
}
#endif
