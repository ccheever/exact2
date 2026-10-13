import XCTest
import ImageIO
import CoreGraphics
import CoreImage
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

    private static let dir = Bundle.module.resourceURL!.appendingPathComponent("ColorFixtures")

    private func convert(_ file: String, finalize: (CGImageDestination) -> Bool = CGImageDestinationFinalize) throws -> CGImageSource {
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        try Picker.jpeg(from: Self.dir.appendingPathComponent(file), to: out, finalize: finalize)
        return try XCTUnwrap(CGImageSourceCreateWithData(try Data(contentsOf: out) as CFData, nil))
    }

    func testARejectedHDRJPEGFallsBackToAnOrientedSDRJPEG() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain-map encoding needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let source = Self.dir.appendingPathComponent("gainmap-orient6.heic")
        var attempts = 0
        try Picker.jpeg(from: source, to: out, finalize: { destination in
            attempts += 1
            let written = CGImageDestinationFinalize(destination)
            return attempts > 1 && written
        })
        XCTAssertEqual(attempts, 2)
        let jpeg = try XCTUnwrap(CGImageSourceCreateWithURL(out as CFURL, nil))
        XCTAssertFalse(Picker.isHDR(jpeg))
        XCTAssertNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
        XCTAssertEqual(CIImage(cgImage: image).contentHeadroom, 1)
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(jpeg, 0, nil) as? [CFString: Any])
        XCTAssertEqual(properties[kCGImagePropertyPixelWidth] as? Int, 64)
        XCTAssertEqual(properties[kCGImagePropertyPixelHeight] as? Int, 256)
        XCTAssertEqual(properties[kCGImagePropertyOrientation] as? Int ?? 1, 1)
        attempts = 0
        XCTAssertThrowsError(try Picker.jpeg(from: source, to: out, finalize: { _ in attempts += 1; return false }))
        XCTAssertEqual(attempts, 2, "only failure of both encodes cancels conversion")
    }

    func testAFinalizedJPEGWithoutAGainMapRetriesSDR() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain-map encoding needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let sdr = try Data(contentsOf: Self.dir.appendingPathComponent("p3-camera.jpg"))
        var attempts = 0
        try Picker.jpeg(from: Self.dir.appendingPathComponent("gainmap-orient6.heic"), to: out, finalize: { destination in
            attempts += 1
            let written = CGImageDestinationFinalize(destination)
            guard attempts == 1 else { return written }
            // ImageIO may report success while declining the gain-map request.
            do { try sdr.write(to: out); return true }
            catch { XCTFail("substituting the finalized JPEG: \(error)"); return false }
        })
        XCTAssertEqual(attempts, 2)
        let jpeg = try XCTUnwrap(CGImageSourceCreateWithURL(out as CFURL, nil))
        XCTAssertFalse(Picker.isHDR(jpeg))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
        XCTAssertEqual(image.width, 64, "the SDR retry applies the source orientation")
        XCTAssertEqual(image.height, 256)
        XCTAssertEqual(CIImage(cgImage: image).contentHeadroom, 1)
    }

    func testAnSDRRetryRejectsAJPEGThatStillHasAGainMap() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain-map encoding needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let hdr = try Data(contentsOf: Self.dir.appendingPathComponent("gainmap-iso.jpg"))
        var attempts = 0
        XCTAssertThrowsError(try Picker.jpeg(from: Self.dir.appendingPathComponent("gainmap-iso.heic"), to: out, finalize: { destination in
            attempts += 1
            _ = CGImageDestinationFinalize(destination)
            guard attempts == 2 else { return false }
            do { try hdr.write(to: out); return true }
            catch { XCTFail("substituting the finalized JPEG: \(error)"); return false }
        }))
        XCTAssertEqual(attempts, 2)
    }

    func testAnHDRPhotoKeepsItsGainMapAndProfile() throws {
        for file in ["gainmap-iso.heic", "pq.heic", "hlg.heic"] {
            let declined = try HDRPlatformProbe.declined("\(file): HDR JPEG and gain map", {
                try HDRPlatformProbe.requireGainMapJPEG(Self.dir.appendingPathComponent(file))
            })
            var attempts = 0
            var converted: CGImageSource?
            var conversionError: Error?
            let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
            defer { try? FileManager.default.removeItem(at: out) }
            do {
                try Picker.jpeg(from: Self.dir.appendingPathComponent(file), to: out, finalize: { destination in
                    attempts += 1
                    return CGImageDestinationFinalize(destination)
                })
                converted = try XCTUnwrap(CGImageSourceCreateWithData(try Data(contentsOf: out) as CFData, nil))
            } catch {
                conversionError = error
            }
            #if os(iOS) && targetEnvironment(simulator)
            if file == "pq.heic" || file == "hlg.heic" {
                XCTAssertEqual(attempts, 2, file)
                if let error = conversionError {
                    XCTAssertEqual((error as? CocoaError)?.code, .fileWriteUnknown, file)
                    let rejected = try XCTUnwrap(CGImageSourceCreateWithURL(out as CFURL, nil))
                    let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(rejected, 0, nil))
                    XCTAssertTrue(image.colorSpace.map(CGColorSpaceUsesITUR_2100TF) == true, file)
                    if #available(iOS 18, *) { XCTAssertEqual(CIImage(cgImage: image).contentHeadroom, 4.93, accuracy: 0.01, file) }
                }
                XCTExpectFailure("QUEUE: iOS PQ/HLG picker SDR conversion and physical HDR qualification; \(file) retains BT.2100 at headroom 4.93", strict: true) {
                    XCTAssertNil(conversionError, "\(file): JPEG conversion must succeed")
                }
            } else {
                XCTAssertNil(conversionError, "\(file): JPEG conversion must succeed")
            }
            #else
            XCTAssertNil(conversionError, "\(file): JPEG conversion must succeed")
            #endif
            guard let jpeg = converted else { continue }
            let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
            let (w, h) = try XCTUnwrap(Picker.pixels(Self.dir.appendingPathComponent(file)))
            XCTAssertEqual(image.width, w, file)
            XCTAssertEqual(image.height, h, file)
            if attempts == 2 {
                XCTAssertFalse(Picker.isHDR(jpeg), "\(file): the retry writes plain SDR")
                if #available(macOS 15, iOS 18, tvOS 18, *) {
                    XCTAssertEqual(CIImage(cgImage: image).contentHeadroom, 1, file)
                    XCTAssertNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap), file)
                }
            }
            if declined == nil && attempts == 1 {
                XCTAssertTrue(Picker.isHDR(jpeg), "\(file): still HDR as JPEG")
                if #available(macOS 15, iOS 18, tvOS 18, *) {
                    XCTAssertNotNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap), file)
                }
            }
        }
        let p3 = try convert("p3.heic")
        XCTAssertFalse(Picker.isHDR(p3))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(p3, 0, nil))
        XCTAssertEqual(image.colorSpace.map(colorSpaceName), "display-p3", "an SDR photo keeps its profile")
    }

    func testOrientationIsAppliedNotCopied() throws {
        var attempts = 0
        let jpeg = try convert("gainmap-orient6.heic", finalize: { destination in
            attempts += 1
            return CGImageDestinationFinalize(destination)
        })
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(jpeg, 0, nil) as? [CFString: Any])
        XCTAssertEqual(properties[kCGImagePropertyPixelWidth] as? Int, 64)
        XCTAssertEqual(properties[kCGImagePropertyPixelHeight] as? Int, 256)
        XCTAssertEqual(properties[kCGImagePropertyOrientation] as? Int ?? 1, 1)
        if try HDRPlatformProbe.declined("gainmap-orient6.heic: HDR JPEG", {
            try HDRPlatformProbe.requireGainMapJPEG(Self.dir.appendingPathComponent("gainmap-orient6.heic"))
        }) == nil && attempts == 1 {
            XCTAssertTrue(Picker.isHDR(jpeg))
        }
    }
}
