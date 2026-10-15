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

    // Finalizer fault injection must reach both encoders even when ImageIO declines HDR thumbnails.
    @available(macOS 15, iOS 18, tvOS 18, *)
    private func thumbnailForFinalization(_ source: CGImageSource, _ index: Int, _ options: CFDictionary?) -> CGImage? {
        var decode = options as? [CFString: Any] ?? [:]
        decode[kCGImageSourceDecodeRequest] = kCGImageSourceDecodeToSDR
        return CGImageSourceCreateThumbnailAtIndex(source, index, decode as CFDictionary)
    }

    func testARejectedHDRJPEGFallsBackToAnOrientedSDRJPEG() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain-map encoding needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let source = Self.dir.appendingPathComponent("gainmap-orient6.heic")
        var attempts = 0
        try Picker.jpeg(from: source, to: out, thumbnail: thumbnailForFinalization, finalize: { destination in
            attempts += 1
            let written = CGImageDestinationFinalize(destination)
            return attempts > 1 && written
        })
        XCTAssertEqual(attempts, 2)
        let jpeg = try XCTUnwrap(CGImageSourceCreateWithURL(out as CFURL, nil))
        XCTAssertFalse(Picker.isHDR(jpeg))
        XCTAssertNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
        assertSDRHeadroom(image)
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(jpeg, 0, nil) as? [CFString: Any])
        XCTAssertEqual(properties[kCGImagePropertyPixelWidth] as? Int, 64)
        XCTAssertEqual(properties[kCGImagePropertyPixelHeight] as? Int, 256)
        XCTAssertEqual(properties[kCGImagePropertyOrientation] as? Int ?? 1, 1)
        attempts = 0
        XCTAssertThrowsError(try Picker.jpeg(from: source, to: out, thumbnail: thumbnailForFinalization, finalize: { _ in attempts += 1; return false }))
        XCTAssertEqual(attempts, 2, "a failed SDR write cancels conversion")
        XCTAssertFalse(FileManager.default.fileExists(atPath: out.path), "failed conversion leaves no JPEG")
    }

    func testAFinalizedJPEGWithoutAGainMapRetriesSDR() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain-map encoding needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let sdr = try Data(contentsOf: Self.dir.appendingPathComponent("p3-camera.jpg"))
        var attempts = 0
        try Picker.jpeg(from: Self.dir.appendingPathComponent("gainmap-orient6.heic"), to: out, thumbnail: thumbnailForFinalization, finalize: { destination in
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
        assertSDRHeadroom(image)
    }

    func testAnSDRRetryThatStaysHDRUsesAnOrientedCoreImageToneMap() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain-map encoding needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let hdr = try Data(contentsOf: Self.dir.appendingPathComponent("gainmap-iso.jpg"))
        var attempts = 0
        try Picker.jpeg(from: Self.dir.appendingPathComponent("gainmap-orient6.heic"), to: out, thumbnail: thumbnailForFinalization, finalize: { destination in
            attempts += 1
            let written = CGImageDestinationFinalize(destination)
            if attempts == 1 { return false }
            guard attempts == 2 else { return written }
            // A finalized SDR request can still carry HDR; Core Image must retry it.
            do { try hdr.write(to: out); return true }
            catch { XCTFail("substituting the finalized JPEG: \(error)"); return false }
        })
        XCTAssertEqual(attempts, 3)
        let jpeg = try XCTUnwrap(CGImageSourceCreateWithURL(out as CFURL, nil))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
        XCTAssertFalse(Picker.isHDR(jpeg))
        assertSDRHeadroom(image)
        XCTAssertEqual(image.colorSpace?.name, CGColorSpace.displayP3)
        XCTAssertEqual(image.width, 64)
        XCTAssertEqual(image.height, 256)
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(jpeg, 0, nil) as? [CFString: Any])
        XCTAssertEqual(properties[kCGImagePropertyOrientation] as? Int ?? 1, 1)
        let bright = try HDRPlatformProbe.sample(image, x: 147, sourceWidth: 256, rotated: true)
        XCTAssertGreaterThan(try XCTUnwrap(bright.max()), 0.5, "the oriented tone map retains light")
    }

    func testFailedConversionRemovesItsOutput() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("gain-map encoding needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let hdr = try Data(contentsOf: Self.dir.appendingPathComponent("gainmap-iso.jpg"))
        var attempts = 0
        XCTAssertThrowsError(try Picker.jpeg(from: Self.dir.appendingPathComponent("gainmap-iso.heic"), to: out, thumbnail: thumbnailForFinalization, finalize: { destination in
            attempts += 1
            _ = CGImageDestinationFinalize(destination)
            if attempts == 1 { return false }
            do { try hdr.write(to: out); return true }
            catch { XCTFail("substituting the finalized JPEG: \(error)"); return false }
        }))
        XCTAssertEqual(attempts, 3)
        XCTAssertFalse(FileManager.default.fileExists(atPath: out.path), "even a finalized but rejected JPEG is removed")
    }

    func testAnSDRSourceDoesNotUseTheHDRToneMap() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("JPEG range validation needs macOS 15 / iOS 18") }
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        let hdr = try Data(contentsOf: Self.dir.appendingPathComponent("gainmap-iso.jpg"))
        var attempts = 0
        XCTAssertThrowsError(try Picker.jpeg(from: Self.dir.appendingPathComponent("p3.heic"), to: out, thumbnail: thumbnailForFinalization, finalize: { destination in
            attempts += 1
            _ = CGImageDestinationFinalize(destination)
            do { try hdr.write(to: out); return true }
            catch { XCTFail("substituting the finalized JPEG: \(error)"); return false }
        }))
        XCTAssertEqual(attempts, 1, "an SDR source cannot enter the HDR tone-map fallback")
        XCTAssertFalse(FileManager.default.fileExists(atPath: out.path))
    }

    func testAnAcceptableJPEGSurvivesAnUnavailableCoreImageFallback() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("JPEG range validation needs macOS 15 / iOS 18") }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("source.heic"), out = directory.appendingPathComponent("picked.jpg")
        try FileManager.default.copyItem(at: Self.dir.appendingPathComponent("gainmap-iso.heic"), to: source)
        let sdr = try Data(contentsOf: Self.dir.appendingPathComponent("p3-camera.jpg"))
        let hdr = try Data(contentsOf: Self.dir.appendingPathComponent("gainmap-iso.jpg"))
        var attempts = 0
        XCTAssertNoThrow(try Picker.jpeg(from: source, to: out, thumbnail: thumbnailForFinalization, finalize: { destination in
            attempts += 1
            _ = CGImageDestinationFinalize(destination)
            do {
                try (attempts == 1 ? sdr : hdr).write(to: out)
                if attempts == 2 { try FileManager.default.removeItem(at: source) }
                return true
            } catch { XCTFail("substituting the finalized JPEG: \(error)"); return false }
        }))
        XCTAssertEqual(attempts, 2)
        XCTAssertNil(CIImage(contentsOf: source), "Core Image cannot decode the removed source")
        XCTAssertEqual(try Data(contentsOf: out), sdr, "a valid earlier SDR rendition is still delivered")
    }

    func testARejectedHDRThumbnailDeclinesBeforeFinalizationAndStillConverts() throws {
        guard #available(macOS 15, iOS 18, tvOS 18, *) else { throw XCTSkip("HDR decode requests need macOS 15 / iOS 18") }
        let source = Self.dir.appendingPathComponent("gainmap-orient6.heic")
        let rejectHDR: (CGImageSource, Int, CFDictionary?) -> CGImage? = { source, index, options in
            let request = (options as? [CFString: Any])?[kCGImageSourceDecodeRequest] as? String
            if request == kCGImageSourceDecodeToHDR as String { return nil }
            return CGImageSourceCreateThumbnailAtIndex(source, index, options)
        }
        var declined: HDRPlatformProbe.Declined?
        XCTAssertNoThrow(declined = try HDRPlatformProbe.declined("gainmap-orient6.heic: rejected HDR thumbnail", {
            try HDRPlatformProbe.requireGainMapJPEG(source, thumbnail: rejectHDR)
        }))
        XCTAssertEqual(declined?.finalizations, 0, "a declined HDR decode never reached the encoder")
        let out = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        defer { try? FileManager.default.removeItem(at: out) }
        var attempts = 0
        try Picker.jpeg(from: source, to: out, thumbnail: rejectHDR, finalize: { destination in
            attempts += 1
            return CGImageDestinationFinalize(destination)
        })
        XCTAssertEqual(attempts, 1, "only the SDR retry reached finalization")
        let jpeg = try XCTUnwrap(CGImageSourceCreateWithURL(out as CFURL, nil))
        XCTAssertFalse(Picker.isHDR(jpeg))
        let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
        assertSDRHeadroom(image)
        XCTAssertEqual(image.width, 64)
        XCTAssertEqual(image.height, 256)
    }

    func testAnHDRPhotoKeepsItsGainMapAndProfile() throws {
        for file in ["gainmap-iso.heic", "pq.heic", "pq.avif", "hlg.heic"] {
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
            XCTAssertNil(conversionError, "\(file): JPEG conversion must succeed")
            var expectedAttempts = 1
            var needsToneMap = false
            if #available(macOS 15, iOS 18, tvOS 18, *), declined != nil {
                let sdrDeclined = try HDRPlatformProbe.declined("\(file): ImageIO SDR JPEG", {
                    try HDRPlatformProbe.requireSDRJPEG(Self.dir.appendingPathComponent(file))
                })
                needsToneMap = sdrDeclined != nil
                expectedAttempts = declined!.finalizations + (sdrDeclined?.finalizations ?? 1) + (needsToneMap ? 1 : 0)
            }
            XCTAssertEqual(attempts, expectedAttempts, "\(file): each supported encode must succeed on its first attempt")
            XCTContext.runActivity(named: "\(file): JPEG conversion attempted \(attempts) finalization(s)") { _ in }
            guard let jpeg = converted else { continue }
            let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(jpeg, 0, nil))
            let (w, h) = try XCTUnwrap(Picker.pixels(Self.dir.appendingPathComponent(file)))
            XCTAssertEqual(image.width, w, file)
            XCTAssertEqual(image.height, h, file)
            if #available(macOS 15, iOS 18, tvOS 18, *), declined != nil {
                XCTAssertFalse(Picker.isHDR(jpeg), "\(file): the retry writes plain SDR")
                assertSDRHeadroom(image)
                XCTAssertNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap), file)
            }
            if needsToneMap {
                try assertCoreImageToneMap(image, file: file)
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
        let declined = try HDRPlatformProbe.declined("gainmap-orient6.heic: HDR JPEG", {
            try HDRPlatformProbe.requireGainMapJPEG(Self.dir.appendingPathComponent("gainmap-orient6.heic"))
        })
        if declined == nil { XCTAssertEqual(attempts, 1, "a supported gain-map encode must not fall back to SDR") }
        XCTContext.runActivity(named: "gainmap-orient6.heic: JPEG conversion attempted \(attempts) finalization(s)") { _ in }
        if declined == nil && attempts == 1 {
            XCTAssertTrue(Picker.isHDR(jpeg))
            if #available(macOS 15, iOS 18, tvOS 18, *) {
                XCTAssertNotNil(CGImageSourceCopyAuxiliaryDataInfoAtIndex(jpeg, 0, kCGImageAuxiliaryDataTypeISOGainMap))
            }
        }
    }

    @available(macOS 15, iOS 18, tvOS 18, *)
    private func assertSDRHeadroom(_ image: CGImage, file: StaticString = #filePath, line: UInt = #line) {
        let headroom = CIImage(cgImage: image).contentHeadroom
        XCTAssertTrue(headroom == 1 || (headroom == 0 && image.colorSpace.map(CGColorSpaceUsesITUR_2100TF) != true),
            "SDR headroom is 1, or unknown in a non-BT.2100 space: \(headroom)", file: file, line: line)
    }

    private func assertCoreImageToneMap(_ image: CGImage, file: String) throws {
        let source = try XCTUnwrap(CIImage(contentsOf: Self.dir.appendingPathComponent(file), options: [.toneMapHDRtoSDR: true]))
        let context = CIContext(options: [.workingColorSpace: try XCTUnwrap(CGColorSpace(name: CGColorSpace.extendedLinearDisplayP3))])
        let reference = try XCTUnwrap(context.createCGImage(source, from: source.extent, format: .RGBA8,
            colorSpace: try XCTUnwrap(CGColorSpace(name: CGColorSpace.displayP3))))
        XCTAssertEqual(image.colorSpace?.name, CGColorSpace.displayP3, file)
        // These columns fall inside neutral patches in both the PQ and HLG fixtures.
        let columns = [21, 63, 105, 147, 189, 233]
        let stored = try columns.map { try HDRPlatformProbe.sample(image, x: $0, sourceWidth: 256) }
        let expected = try columns.map { try HDRPlatformProbe.sample(reference, x: $0, sourceWidth: 256) }
        for (got, want) in zip(stored, expected) {
            for c in 0..<3 { XCTAssertEqual(got[c], want[c], accuracy: 0.02, "\(file): Core Image's tone map survives JPEG encoding") }
        }
        // The gain-map fixture starts above black and ends with coloured patches.
        guard file != "gainmap-iso.heic" else { return }
        let neutral = stored.map { $0[1] }
        XCTAssertEqual(neutral.first!, 0, accuracy: 0.01, file)
        XCTAssertGreaterThan(neutral[1], 0.01, "\(file): the tone map is not black")
        XCTAssertEqual(neutral.last!, 1, accuracy: 0.02, file)
        XCTAssertEqual(neutral, neutral.sorted(), "\(file): brighter stays brighter")
    }
}
