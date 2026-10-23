#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class NativeButtonRasterIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?
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
    func testARasterFaceLoadsChangesSizeAndColourAndReleasesWhenRemoved() throws {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_BUTTON_RASTER_PLAN"])))
        let session = ExactApp.shared.makeSession(label: "button-raster")
        self.session = session
        let view = ExactView(session: session), controller = UIViewController()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.rootViewController = controller
        window.makeKeyAndVisible()
        controller.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = controller.view.bounds
        view.layoutIfNeeded()
        let p = session.presenter
        let owner = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "photo" })
        let button = try XCTUnwrap(p.controls.controls[owner.id] as? NativeButtonIOS)
        until { button.configuration?.image != nil && p.controls.face(owner.id).image?.size == CGSize(width: 32, height: 20) }
        XCTAssertEqual(button.configuration?.subtitle, "Choose a picture")
        XCTAssertEqual(button.configuration?.image?.size, CGSize(width: 32, height: 20))
        XCTAssertEqual(try pixel(button), [255, 0, 0, 255])
        let before = button.naturalSize
        p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "swap" }).id)
        until { p.controls.face(owner.id).image?.size == CGSize(width: 48, height: 24) && button.configuration?.image?.size == CGSize(width: 48, height: 24) }
        XCTAssertEqual(try pixel(button), [0, 0, 255, 255])
        XCTAssertGreaterThan(button.naturalSize.width, before.width)
        p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "remove" }).id)
        until { button.configuration?.image == nil }
        XCTAssertNil(owner.raster)
        XCTAssertNil(owner.imageSource)
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
