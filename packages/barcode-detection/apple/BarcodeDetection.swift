// @exact/barcode-detection on Apple (LLP 1133.027): Vision's
// `VNDetectBarcodesRequest`, on a still (D4) or on the frames of a camera
// track a viewfinder shows (D5, Scan.swift).
//
// - A still is a file the runner admitted against `fs.read`, decoded upright:
//   the request handler is given the file's EXIF orientation, and Vision's
//   normalised, bottom-left boxes are converted to the upright image's pixels
//   from the top left.
// - Vision has no UPC-A: it reads one as an EAN-13 with a leading 0, and D11's
//   rule answers it as the formats asked name it.
// - Vision runs for real under the agent (§9), on the drive's fixture images;
//   no hatch is called there.
// - Tier 3 (D13): `requestHatch` before each request is performed, after the
//   module's symbologies; `videoOutputHatch` once a scan's output is added.
import AVFoundation
import ExactPackage
import Foundation
import ImageIO
import Vision
#if targetEnvironment(simulator)
import CoreML
#endif

/// Vision's models find no Neural Engine or Metal device in the iOS and tvOS
/// simulators, where a request fails "Could not create inference context";
/// there each stage runs on the CPU.
fileprivate func simulatorCPU(_ request: VNRequest) {
    #if targetEnvironment(simulator)
    if #available(iOS 17.0, tvOS 17.0, *), let stages = try? request.supportedComputeStageDevices {
        for (stage, devices) in stages {
            if let cpu = devices.first(where: { if case .cpu = $0 { return true } else { return false } }) { request.setComputeDevice(cpu, for: stage) }
        }
    }
    #endif
}

public final class BarcodeDetectionModule: ExactPackage {
    /// Each request, on the module's work queue, after its symbologies: the
    /// app may set `revision` and `regionOfInterest`.
    public static var requestHatch: ((VNDetectBarcodesRequest) -> Void)?
    /// A scan's output, inside `configure` on the session's queue, once it is
    /// added: the app may set `videoSettings` beyond the pixel format.
    public static var videoOutputHatch: ((AVCaptureVideoDataOutput) -> Void)?

    let context: ExactPackageContext
    let work = DispatchQueue(label: "exact.barcode-detection", qos: .userInitiated)
    #if !os(tvOS)
    private var scans: [String: Scan] = [:]
    #endif

    public init(context: ExactPackageContext) { self.context = context }

    func say(_ line: String) { context.log("barcode-detection: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "getSupportedFormats": reply(Self.supported())
        case "detectBarcodes": still(args, reply)
        case "detectedBarcodes": scan(args, reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "getSupportedFormats" || name == "getSupportedFormatsSync" ? Self.supported() : ["error": "NotSupportedError"]
    }

    // MARK: formats (D1, D11)

    static let formats = ["aztec", "code_128", "code_39", "code_93", "codabar", "data_matrix", "ean_13", "ean_8", "itf", "pdf417", "qr_code", "upc_a", "upc_e"]

    /// The web's name for a Vision symbology, or nil where the web has none (`unknown`).
    static func format(_ s: VNBarcodeSymbology) -> String? {
        switch s {
        case .aztec: return "aztec"
        case .code128: return "code_128"
        case .code39, .code39Checksum, .code39FullASCII, .code39FullASCIIChecksum: return "code_39"
        case .code93, .code93i: return "code_93"
        case .codabar: return "codabar"
        case .dataMatrix: return "data_matrix"
        case .ean13: return "ean_13"
        case .ean8: return "ean_8"
        case .itf14, .i2of5, .i2of5Checksum: return "itf"
        case .pdf417: return "pdf417"
        case .qr: return "qr_code"
        case .upce: return "upc_e"
        default: return nil
        }
    }

    /// Vision's symbologies on this OS: the default request's.
    static let symbologies: [VNBarcodeSymbology] = (try? VNDetectBarcodesRequest().supportedSymbologies()) ?? []

    /// The formats read here, `upc_a` beside `ean_13` by D11's rule.
    static let here: [String] = {
        let own = Set(symbologies.compactMap(format))
        return formats.filter { own.contains($0) || ($0 == "upc_a" && own.contains("ean_13")) }
    }()

    static func supported() -> [String: Any] {
        #if os(tvOS)
        let tracks = false // a live scan waits for the camera's tvOS cell (D18)
        #else
        let tracks = !here.isEmpty
        #endif
        return ["formats": here, "tracks": tracks, "apple": ["symbologies": symbologies.map(\.rawValue)]]
    }

    /// The formats asked: nil for all; `{error}` for D2's `TypeError` or `NotSupportedError`.
    static func asked(_ value: Any?) -> (Set<String>?, [String: Any]?) {
        guard let value, !(value is NSNull) else { return (nil, nil) }
        guard let list = value as? [String], !list.isEmpty, list.allSatisfy(formats.contains) else {
            return (nil, ["error": "TypeError", "message": "formats name D1's barcode formats"])
        }
        if let missing = list.first(where: { !here.contains($0) }) { return (nil, ["error": "NotSupportedError", "message": "Vision here reads no \(missing)"]) }
        return (Set(list), nil)
    }

    /// The symbologies a request asks for `keep`: an EAN-13 whenever either UPC shape is asked (D11).
    static func symbologies(for keep: Set<String>?) -> [VNBarcodeSymbology] {
        guard let keep else { return symbologies }
        var want = keep
        if want.contains("upc_a") { want.insert("ean_13") }
        return symbologies.filter { format($0).map(want.contains) ?? false }
    }

    // MARK: detection

    /// The barcodes Vision finds in `handler`'s image, `width` by `height` upright pixels, after D11's rule.
    func detect(_ handler: VNImageRequestHandler, width: Double, height: Double, keep: Set<String>, all: Bool) throws -> [[String: Any]] {
        let request = VNDetectBarcodesRequest()
        #if targetEnvironment(simulator)
        // Revisions 3 and 4 find no code on the simulators' CPU; revision 2
        // does, once per scan line it reads, which `merged` makes one.
        request.revision = VNDetectBarcodesRequestRevision2
        let older = Set((try? request.supportedSymbologies()) ?? [])
        request.symbologies = Self.symbologies(for: all ? nil : keep).filter(older.contains)
        #else
        request.symbologies = Self.symbologies(for: all ? nil : keep)
        #endif
        if !context.agent { Self.requestHatch?(request) }
        simulatorCPU(request)
        try handler.perform([request])
        #if targetEnvironment(simulator)
        let found = Self.merged((request.results ?? []).map { ($0, $0.boundingBox) })
        #else
        let found = (request.results ?? []).map { ($0, $0.boundingBox) }
        #endif
        return Self.normalised(found.map { Self.shape($0.0, box: $0.1, width: width, height: height) }, keep: all ? nil : keep)
    }

    #if targetEnvironment(simulator)
    /// The observations of one value as one, with their union box. Two copies of one code in an image are one
    /// on a simulator, whose scan lines of one code need not meet.
    static func merged(_ found: [(VNBarcodeObservation, CGRect)]) -> [(VNBarcodeObservation, CGRect)] {
        var out: [(VNBarcodeObservation, CGRect)] = []
        for (o, box) in found {
            if let i = out.firstIndex(where: { $0.0.symbology == o.symbology && $0.0.payloadStringValue == o.payloadStringValue }) {
                out[i].1 = out[i].1.union(box)
            } else {
                out.append((o, box))
            }
        }
        return out
    }
    #endif

    /// One observation in the upright image's pixels, top left origin, its corners clockwise from the top left. A
    /// `box` other than the observation's own (several merged) gives the corners too.
    static func shape(_ o: VNBarcodeObservation, box b: CGRect, width: Double, height: Double) -> [String: Any] {
        let point = { (p: CGPoint) -> [String: Any] in ["x": Double(p.x) * width, "y": (1 - Double(p.y)) * height] }
        let corners = b == o.boundingBox ? [o.topLeft, o.topRight, o.bottomRight, o.bottomLeft]
            : [CGPoint(x: b.minX, y: b.maxY), CGPoint(x: b.maxX, y: b.maxY), CGPoint(x: b.maxX, y: b.minY), CGPoint(x: b.minX, y: b.minY)]
        var apple: [String: Any] = ["symbology": o.symbology.rawValue]
        if #available(iOS 17.0, macOS 14.0, tvOS 17.0, *), let data = o.payloadData { apple["payloadData"] = data.base64EncodedString() }
        return [
            "rawValue": o.payloadStringValue ?? "",
            "format": format(o.symbology) ?? "unknown",
            "boundingBox": ["x": Double(b.minX) * width, "y": (1 - Double(b.maxY)) * height, "width": Double(b.width) * width, "height": Double(b.height) * height],
            "cornerPoints": corners.map(point),
            "apple": apple,
        ]
    }

    /// D11: one UPC-A answered as the formats asked name it, and only formats asked kept.
    static func normalised(_ results: [[String: Any]], keep: Set<String>?) -> [[String: Any]] {
        results.compactMap { r in
            var r = r
            var format = r["format"] as? String ?? "unknown", raw = r["rawValue"] as? String ?? ""
            let digits = raw.allSatisfy(\.isASCII) && raw.allSatisfy(\.isNumber)
            if digits, (format == "upc_a" && raw.count == 12) || (format == "ean_13" && raw.count == 13 && raw.hasPrefix("0")) {
                let twelve = format == "upc_a" ? raw : String(raw.dropFirst())
                if keep == nil || keep!.contains("upc_a") { format = "upc_a"; raw = twelve } else { format = "ean_13"; raw = "0" + twelve }
            }
            if let keep, !keep.contains(format) { return nil }
            r["format"] = format; r["rawValue"] = raw
            return r
        }
    }

    // MARK: stills (D4)

    private func still(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let (keep, refused) = Self.asked(args["formats"])
        if let refused { return reply(refused) }
        guard let path = args["image"] as? String, let url = context.file(path) else { return reply(["error": "NotFoundError"]) }
        work.async { [self] in
            let answer: [String: Any]
            switch Still.load(url) {
            case .refused(let error): answer = error
            case .loaded(let image):
                do {
                    let barcodes = try detect(VNImageRequestHandler(cgImage: image.cgImage, orientation: image.orientation), width: image.width, height: image.height, keep: keep ?? [], all: keep == nil)
                    answer = ["barcodes": barcodes, "width": image.width, "height": image.height]
                } catch {
                    answer = ["error": "OperationError", "message": error.localizedDescription]
                }
            }
            DispatchQueue.main.async { [self] in
                if let n = (answer["barcodes"] as? [Any])?.count { say("\(path): \(n) found") }
                reply(answer)
            }
        }
    }

    // MARK: live scans (D5)

    private func scan(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        #if os(tvOS)
        reply(["error": "NotSupportedError", "message": "a live scan waits for the camera's tvOS cell"])
        #else
        let (keep, refused) = Self.asked(args["formats"])
        if let refused { return reply(refused) }
        // No track yet (an app's state before its camera opens) is a scan with nothing to read.
        if (args["track"] as? String ?? "").isEmpty {
            return reply(["state": "idle", "barcodes": [Any](), "seen": [Any](), "frame": 0, "width": 0, "height": 0])
        }
        guard let track = args["track"] as? String, let entry = context.tracks.lookup(track) else {
            return reply(["error": "NotFoundError", "message": "the session holds no track \(args["track"] ?? "")"])
        }
        guard entry.kind == "video" else { return reply(["error": "NotSupportedError", "message": "\(track) is not a video track"]) }
        let key = Scan.key(track, keep)
        if scans[key] == nil {
            let interval = max(50, (args["interval"] as? NSNumber)?.doubleValue ?? 100)
            let s = Scan(module: self, track: track, keep: keep, interval: interval)
            do { try s.start() } catch let e as ExactTrackError { return reply(["error": e.code]) } catch { return reply(["error": "NotFoundError"]) }
            scans[key] = s
            say("scan \(key): observing \(track)")
        }
        scans[key]?.analyse(reply)
        #endif
    }

    #if !os(tvOS)
    public func release(_ name: String, args: [String: Any]) {
        guard name == "detectedBarcodes", let track = args["track"] as? String else { return }
        let (keep, _) = Self.asked(args["formats"])
        end(Scan.key(track, keep), "released")
    }

    public func release(_ name: String) {
        if name == "detectedBarcodes" { for key in Array(scans.keys) { end(key, "released") } }
    }

    private func end(_ key: String, _ why: String) {
        guard let s = scans.removeValue(forKey: key) else { return }
        s.stop()
        say("scan \(key): \(why)")
    }

    public func destroy(_ done: @escaping () -> Void) {
        for key in Array(scans.keys) { end(key, "the session ended") }
        done()
    }
    #endif
}

/// A still decoded for Vision: its pixels as stored and the orientation that makes them upright.
struct Still {
    let cgImage: CGImage
    let orientation: CGImagePropertyOrientation
    /// The upright image's size.
    let width: Double, height: Double

    enum Loaded { case loaded(Still), refused([String: Any]) }

    /// The file at `url`, or D2's error.
    static func load(_ url: URL) -> Loaded {
        guard FileManager.default.fileExists(atPath: url.path) else { return .refused(["error": "NotFoundError"]) }
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil), let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            return .refused(["error": "InvalidStateError", "message": "decode"])
        }
        let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any]
        let raw = (properties?[kCGImagePropertyOrientation] as? NSNumber)?.uint32Value ?? 1
        let orientation = CGImagePropertyOrientation(rawValue: raw) ?? .up
        let turned = [.left, .leftMirrored, .right, .rightMirrored].contains(orientation)
        return .loaded(Still(cgImage: image, orientation: orientation,
                              width: Double(turned ? image.height : image.width), height: Double(turned ? image.width : image.height)))
    }
}
