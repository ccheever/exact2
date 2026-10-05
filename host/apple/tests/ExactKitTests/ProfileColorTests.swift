import XCTest
import CoreGraphics
import CoreText
import IOSurface
import QuartzCore
#if os(macOS)
import AppKit
#else
import UIKit
#endif
@testable import ExactKit

/// LLP 1100 D3: a profile's colour is made by Core Graphics in the profile's
/// space — a standard one by name, an app's from its ICC file — and a reader
/// that needs sRGB asks Core Graphics to convert it.
final class ProfileColorTests: XCTestCase {
    func testAStandardSpaceIsCoreGraphicsOwn() throws {
        let row: BatchValue = ["cs": [["s": "cg:kCGColorSpaceDCIP3", "i": "relative-colorimetric", "v": [1, 0, 0, 1]]]]
        let c = try XCTUnwrap(row.cgColor(dark: false))
        XCTAssertEqual(c.colorSpace?.name, CGColorSpace.dcip3)
        XCTAssertEqual(c.components, [1, 0, 0, 1])
        let srgb = try XCTUnwrap(row.channels(dark: false), "Core Graphics converts it for a channel reader")
        XCTAssertEqual(srgb[0], 255, accuracy: 1)
        XCTAssertEqual(try XCTUnwrap(row.textChannels(dark: false)).count, 9, "text carries Core Graphics' extended sRGB")
    }

    private func root() throws -> URL {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
        return root
    }

    private func row(_ resolver: AssetResolver, intent: String = "relative-colorimetric", values: [Double] = [1, 0, 0, 1]) throws -> BatchValue {
        let wire: [String: Any] = ["ops": [["op": "style", "id": 1, "style": ["text_color": ["cs": [["s": "icc:assets/brand.icc", "i": intent, "v": values]]]]]]]
        let batch = Batch.decode(try JSONSerialization.data(withJSONObject: wire), resolver: resolver)
        XCTAssertNil(batch.error)
        return try XCTUnwrap(batch.ops.first?.style["text_color"])
    }

    func testProfilesAreBoundToTheirSessionAndAssetBytes() throws {
        let a = try root(), b = try root()
        defer { try? FileManager.default.removeItem(at: a); try? FileManager.default.removeItem(at: b) }
        let srgb = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB)?.copyICCData()) as Data
        let p3 = try XCTUnwrap(CGColorSpace(name: CGColorSpace.displayP3)?.copyICCData()) as Data
        try srgb.write(to: a.appendingPathComponent("assets/brand.icc"))
        try p3.write(to: b.appendingPathComponent("assets/brand.icc"))
        let ra = AssetResolver(root: a), rb = AssetResolver(root: b)
        let first = try row(ra), second = try row(rb)
        XCTAssertEqual(try XCTUnwrap(first.cgColor(dark: false)?.components)[0], 1, accuracy: 0.002)
        XCTAssertGreaterThan(try XCTUnwrap(second.cgColor(dark: false)?.components)[0], 1.05)
        XCTAssertEqual(try row(ra), first, "another session cannot change this session's profile")
        try p3.write(to: a.appendingPathComponent("assets/brand.icc"))
        let updated = try row(ra)
        XCTAssertEqual(updated, second, "same bytes give the same immutable identity")
        XCTAssertNotEqual(updated, first, "a replaced file cannot hit the old style/profile cache")
        XCTAssertEqual(try XCTUnwrap(first.cgColor(dark: false)?.components)[0], 1, accuracy: 0.002, "old views retain their profile")
        let missing: BatchValue = ["cs": [["s": "icc:assets/missing.icc", "v": [1, 0, 0, 1]]]]
        XCTAssertNil(missing.cgColor(dark: false))
    }

    #if os(macOS)
    func testFloatTextSurfaceWithoutHeadroomKeepsExtendedStorage() throws {
        let surface = try XCTUnwrap(IOSurface(properties: [.width: 10, .height: 10,
            .bytesPerElement: 8, .pixelFormat: UInt32(0x52476841)]))
        XCTAssertEqual(TextRasterJob.headroom(of: surface), 1)
        let layer = CALayer()
        layer.applyTextRange(headroom: TextRasterJob.headroom(of: surface), limit: nil)
        if #available(macOS 26, *) { XCTAssertEqual(layer.preferredDynamicRange, .high) }
        else { XCTAssertTrue(layer.wantsExtendedDynamicRangeContent) }
    }

    func testExplicitPlanBootInstallsItsProfileResolver() throws {
        let dir = try root(), app = ExactApp.shared
        let oldRoot = app.assetRoot
        defer { app.assetRoot = oldRoot; try? FileManager.default.removeItem(at: dir) }
        let profile = try XCTUnwrap(CGColorSpace(name: CGColorSpace.displayP3)?.copyICCData()) as Data
        try profile.write(to: dir.appendingPathComponent("assets/brand.icc"))
        let source = "color-profile --explicit-test src=\"assets/brand.icc\"\ncomponent App\n  view\n    box width=20 height=20 background-color=\"color(--explicit-test 1 0 0)\"\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        app.assetRoot = dir
        let session = app.makeSession()
        defer { session.destroy() }
        let batch = session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 100, height: 100))
        XCTAssertNil(batch.error)
        let value = try XCTUnwrap(batch.ops.compactMap { $0.style["background_color"] }.first)
        XCTAssertGreaterThan(try XCTUnwrap(value.cgColor(dark: false)?.components)[0], 1.05)
        let view = try XCTUnwrap(session.presenter.views.values.first { $0.style["background_color"] != nil })
        XCTAssertNotNil(view.cgColor("background_color"))
    }
    #endif

    func testProfileCacheIsBoundedAndLiveValuesSurviveEviction() throws {
        let dir = try root()
        defer { try? FileManager.default.removeItem(at: dir) }
        let bytes = try XCTUnwrap(CGColorSpace(name: CGColorSpace.displayP3)?.copyICCData()) as Data
        let url = dir.appendingPathComponent("assets/brand.icc"), resolver = AssetResolver(root: dir)
        try bytes.write(to: url)
        let live = try row(resolver)
        for i in 0..<80 {
            var generation = bytes
            // ICC's profile ID is metadata; the color transform stays valid.
            generation[84] = UInt8(i)
            generation[85] = 1
            try generation.write(to: url)
            XCTAssertNotNil(try row(resolver).cgColor(dark: false))
        }
        XCTAssertLessThanOrEqual(ProfileSpaces.cacheCount, 64)
        XCTAssertGreaterThan(try XCTUnwrap(live.cgColor(dark: false)?.components)[0], 1.05)
    }

    func testICCConversionUsesTheAuthoredRenderingIntent() throws {
        let root = try root()
        defer { try? FileManager.default.removeItem(at: root) }
        let profile = try XCTUnwrap(CGColorSpace(name: CGColorSpace.genericCMYK))
        try (XCTUnwrap(profile.copyICCData()) as Data).write(to: root.appendingPathComponent("assets/brand.icc"))
        let resolver = AssetResolver(root: root), target = try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedSRGB))
        var foundDifferentTransforms = false
        for values: [Double] in [[0, 1, 1, 0, 1], [0.1, 0.8, 0.2, 0.05, 1], [0, 0, 0, 0, 1]] {
            let color = try XCTUnwrap(CGColor(colorSpace: profile, components: values.map { CGFloat($0) }))
            var results: [[CGFloat]] = []
            for (name, intent) in [("perceptual", CGColorRenderingIntent.perceptual), ("relative-colorimetric", .relativeColorimetric), ("absolute-colorimetric", .absoluteColorimetric), ("saturation", .saturation)] {
                let expected = try XCTUnwrap(color.converted(to: target, intent: intent, options: nil)?.components)
                let actual = try XCTUnwrap(row(resolver, intent: name, values: values).cgColor(dark: false)?.components)
                for (a, e) in zip(actual, expected) { XCTAssertEqual(a, e, accuracy: 0.0001, name) }
                results.append(expected)
            }
            foundDifferentTransforms = foundDifferentTransforms || results.dropFirst().contains { r in zip(r, results[0]).contains { abs($0 - $1) > 0.001 } }
        }
        XCTAssertTrue(foundDifferentTransforms, "the fixture must distinguish intents")
    }

    func testProfiledTextPreservesGamutBeyondSRGBAndP3() throws {
        for name in ["cg:kCGColorSpaceDCIP3", "cg:kCGColorSpaceROMMRGB"] {
            let value: BatchValue = ["cs": [["s": .string(name), "v": [0, 1, 0, 1]]]]
            let channels = try XCTUnwrap(value.textChannels(dark: false))
            let color = TextEngine.color(channels).cgColor
            XCTAssertTrue(ColorRange.needsExtendedComponents(color))
            let source = NSAttributedString(string: "Wide", attributes: [.font: CTFontCreateWithName("Helvetica" as CFString, 24, nil), .foregroundColor: color])
            let box = CGRect(x: 0, y: 0, width: 100, height: 40)
            let image = try XCTUnwrap(TextRasterJob(source: source, ranges: [CFRange(location: 0, length: 4)], baselines: [28], flush: 0, box: box, size: box.size, scale: 2).render())
            #if os(macOS)
            XCTAssertEqual(IOSurfaceGetBytesPerElement(image.surface), 8, "profile outside P3 needs float storage")
            XCTAssertEqual(TextRasterJob.headroom(of: image.surface), image.headroom)
            if name == "cg:kCGColorSpaceDCIP3" { XCTAssertEqual(image.headroom, 1, "wide SDR exercises both presentation branches") }
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            session.presenter.apply(wireBatch([
                ["op": "create", "id": 1, "kind": "text", "props": ["text": "Wide"]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 40]
            ]))
            let node = try XCTUnwrap(session.presenter.views[1])
            node.textRaster = image.surface; node.textRasterScale = 2
            for frame in [box, box.insetBy(dx: -2, dy: -2)] {
                node.textRasterFrame = frame
                node.presentTextRaster()
                let ink = try XCTUnwrap(node.textRasterOverflowLayer ?? node.layer)
                if #available(macOS 26, *) {
                    XCTAssertEqual(ink.preferredDynamicRange, .high)
                    XCTAssertEqual(ink.contentsHeadroom, CGFloat(image.headroom))
                } else { XCTAssertTrue(ink.wantsExtendedDynamicRangeContent) }
            }
            #else
            XCTAssertEqual(image.image.bitsPerComponent, 16)
            if #available(iOS 18, tvOS 18, *) { XCTAssertEqual(image.image.contentHeadroom, image.headroom, accuracy: 0.001) }
            let session = ExactApp.shared.makeSession()
            defer { session.destroy() }
            session.presenter.apply(wireBatch([
                ["op": "create", "id": 1, "kind": "text", "props": ["text": "Wide"]],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 40]
            ]))
            let node = try XCTUnwrap(session.presenter.views[1])
            let key = TextRasterKey(spec: node.paragraphSpec(), size: box.size, box: box, scale: 2)
            node.textRasterKey = key; node.textRasterReady = false
            node.showTextRaster(image, for: key)
            let ink = try XCTUnwrap(node.textRasterLayer)
            if #available(iOS 26, tvOS 26, *) {
                XCTAssertEqual(ink.preferredDynamicRange, .high)
                XCTAssertEqual(ink.contentsHeadroom, CGFloat(image.headroom))
            }

            #endif
        }
    }
}
