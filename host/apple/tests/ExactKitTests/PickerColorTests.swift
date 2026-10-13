import XCTest
import ImageIO
import CoreGraphics
import UniformTypeIdentifiers
@testable import ExactKit

/// LLP 1100 D13: a photo the picker converts to JPEG keeps its color
/// profile and, where the system can encode one, its gain map.
final class PickerColorTests: XCTestCase {
    func testUnrestrictedFileInputOffersAnyItem() {
        XCTAssertEqual(Picker.types([]), [.item])
        XCTAssertEqual(Picker.types(["*/*"]), [.item])
        XCTAssertEqual(Picker.types(["image/*"]), [.image])
    }

    #if os(macOS)
    func testUnrestrictedPanelHasNoTypeFilter() {
        for accept in [[], ["*/*"], ["image/jpeg", "*/*"]] {
            let request = Picker.Request(view: 1, id: "attach", accept: accept, multiple: false)
            XCTAssertTrue(Picker.panel(for: request).allowedContentTypes.isEmpty, "\(accept) must leave the panel unrestricted")
        }
        let request = Picker.Request(view: 1, id: "photo", accept: ["image/jpeg"], multiple: false)
        XCTAssertEqual(Picker.panel(for: request).allowedContentTypes, [.jpeg])
    }
    #endif

    func testUnknownFileIsCopiedAndMixedWildcardKeepsHEICBytes() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let unknown = directory.appendingPathComponent("archive.xyz")
        try Data("any bytes".utf8).write(to: unknown)
        let copied = directory.appendingPathComponent("copied.xyz")
        try Picker.copy(unknown, to: copied, accept: [])
        XCTAssertEqual(try Data(contentsOf: copied), Data("any bytes".utf8))

        let original = Self.dir.appendingPathComponent("p3.heic")
        let preserved = directory.appendingPathComponent("photo.heic")
        try Picker.copy(original, to: preserved, accept: ["image/jpeg", "*/*"])
        XCTAssertEqual(try Data(contentsOf: preserved), try Data(contentsOf: original))
    }

    #if os(macOS)
    func testAnsweredCopiesAnUnknownFileWithNoAcceptIntoAppTmp() throws {
        _ = NSApplication.shared
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = """
        component App
          state picked = "none"
          action onPick(files: Picked[])
            picked = match first(files) { case some(f) => f.path, case none => "empty" }
          view
            column
              input type="file" id="attach" change=onPick
              text picked
        """
        try source.write(to: directory.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", directory.appendingPathComponent("app.contract").path, "-o", directory.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "any-file")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: directory.appendingPathComponent("app.plan")), size: CGSize(width: 300, height: 100)).error)
        let request = try XCTUnwrap((try JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"picker","id":"attach"}"#).utf8)) as? [String: Any])?["view"] as? NSNumber)
        let file = directory.appendingPathComponent("archive.xyz")
        try Data("any bytes".utf8).write(to: file)
        XCTAssertNil(session.presenter.views[request.uint32Value]?.props["accept"])
        session.picker.answered(["capability": "pick", "node": request, "answered": "value"], request: ["text": file.path])
        let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any])
        let slots = try XCTUnwrap(state["slots"] as? [String: Any])
        let path = try XCTUnwrap(slots["picked"] as? String)
        XCTAssertTrue(path.hasPrefix("app:/tmp/picked/") && path.hasSuffix(".xyz"), path)
        XCTAssertEqual(try Data(contentsOf: XCTUnwrap(AppFiles.url(path))), Data("any bytes".utf8))
    }
    #endif

    private static let dir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("scripts/fixtures/color")

    private func convert(_ file: String) throws -> CGImageSource {
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        try Picker.jpeg(from: Self.dir.appendingPathComponent(file), to: out)
        return try XCTUnwrap(CGImageSourceCreateWithData(try Data(contentsOf: out) as CFData, nil))
    }

    func testAnHDRPhotoKeepsItsGainMapAndProfile() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain maps encode from macOS 15 / iOS 18") }
        for file in ["gainmap-iso.heic", "pq.heic"] {
            let jpeg = try convert(file)
            XCTAssertTrue(Picker.isHDR(jpeg), "\(file): still HDR as JPEG")
            XCTAssertNotNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap), file)
        }
        let p3 = try convert("p3.heic")
        XCTAssertFalse(Picker.isHDR(p3))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(p3, 0, nil))
        XCTAssertEqual(image.colorSpace.map(colorSpaceName), "display-p3", "an SDR photo keeps its profile")
    }

    func testOrientationIsAppliedNotCopied() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain maps encode from macOS 15 / iOS 18") }
        let jpeg = try convert("gainmap-orient6.heic")
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(jpeg, 0, nil) as? [CFString: Any])
        XCTAssertEqual(properties[kCGImagePropertyPixelWidth] as? Int, 64)
        XCTAssertEqual(properties[kCGImagePropertyPixelHeight] as? Int, 256)
        XCTAssertEqual(properties[kCGImagePropertyOrientation] as? Int ?? 1, 1)
        XCTAssertTrue(Picker.isHDR(jpeg))
    }
}
