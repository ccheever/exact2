// @exact/text-detection on Apple (LLP 1133.027): Vision's
// `VNRecognizeTextRequest` on a still (D4), each observation one line in
// reading order.
//
// - A still is a file the runner admitted against `fs.read`, decoded upright
//   (the file's EXIF orientation given to the request handler), its boxes
//   converted to the upright image's pixels from the top left.
// - `languages` are matched by LLP 1133.027 D7's table to Vision's
//   `supportedRecognitionLanguages()`; with none, the request's default
//   `recognitionLanguages` (D6). Vision downloads nothing, so the
//   availability is `available` or `unavailable`.
// - Tier 2 (D11): `apple.recognitionLevel`, `apple.usesLanguageCorrection`,
//   `apple.customWords`, `apple.minimumTextHeight`. Tier 3 (D13):
//   `requestHatch` before each request is performed.
// - Vision runs for real under the agent (§9); no hatch is called there.
import ExactPackage
import Foundation
import ImageIO
import Vision

public final class TextDetectionModule: ExactPackage {
    /// Each request, on the module's work queue, after the module's options:
    /// the app may set `revision`, `regionOfInterest` and
    /// `automaticallyDetectsLanguage`.
    public static var requestHatch: ((VNRecognizeTextRequest) -> Void)?

    let context: ExactPackageContext
    private let work = DispatchQueue(label: "exact.text-detection", qos: .userInitiated)

    public init(context: ExactPackageContext) { self.context = context }

    private func say(_ line: String) { context.log("text-detection: \(line)") }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "textDetectorAvailability": reply(Self.availability(args["languages"]))
        case "detectText": detect(args, reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "textDetectorAvailability" || name == "textDetectorAvailabilitySync" ? Self.availability(args["languages"]) : ["error": "NotSupportedError"]
    }

    // MARK: languages (D6, D7)

    /// Vision's languages at the accurate level, the default.
    static let supported: [String] = (try? VNRecognizeTextRequest().supportedRecognitionLanguages()) ?? []
    /// The default request's languages.
    static let defaults: [String] = VNRecognizeTextRequest().recognitionLanguages

    /// `tag`'s script made explicit where its region implies one (`zh-TW` is `zh-Hant`), as D7's table says.
    static func scripted(_ tag: String) -> [String] {
        var parts = tag.replacingOccurrences(of: "_", with: "-").split(separator: "-").map(String.init)
        guard let language = parts.first?.lowercased() else { return [] }
        parts[0] = language
        if language == "zh", !parts.dropFirst().contains(where: { $0.count == 4 }) {
            let region = parts.dropFirst().first { $0.count == 2 }?.uppercased()
            parts.insert(["TW", "HK", "MO"].contains(region ?? "") ? "Hant" : "Hans", at: 1)
        }
        return parts
    }

    /// The recognizer's language for `tag`: the tag truncated as LLP 1060 truncates until Vision lists it, else a
    /// listed language of the same language and script.
    static func match(_ tag: String) -> String? {
        var parts = scripted(tag)
        let listed = Dictionary(supported.map { ($0.lowercased(), $0) }, uniquingKeysWith: { a, _ in a })
        while !parts.isEmpty {
            let t = parts.joined(separator: "-").lowercased()
            if let hit = listed[t] { return hit }
            if let hit = supported.first(where: { $0.lowercased().hasPrefix(t + "-") }) { return hit }
            parts.removeLast()
        }
        return nil
    }

    /// The languages asked, matched; `unmatched` names the first that is not.
    static func languages(_ value: Any?) -> (matched: [String], unmatched: String?) {
        guard let asked = value as? [String] else { return (defaults, nil) }
        var matched: [String] = []
        for tag in asked {
            guard let m = match(tag) else { return ([], tag) }
            if !matched.contains(m) { matched.append(m) }
        }
        return (matched, nil)
    }

    static func availability(_ value: Any?) -> [String: Any] {
        let apple: [String: Any] = ["supportedRecognitionLanguages": supported]
        let (matched, unmatched) = languages(value)
        if unmatched != nil || supported.isEmpty {
            return ["availability": "unavailable", "reason": supported.isEmpty ? "not-supported" : "unsupported-options", "languages": [String](), "apple": apple]
        }
        return ["availability": "available", "languages": matched, "apple": apple]
    }

    // MARK: detection

    private func detect(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let (matched, unmatched) = Self.languages(args["languages"])
        if let unmatched { return reply(["error": "NotSupportedError", "message": "Vision reads no \(unmatched)"]) }
        guard let path = args["image"] as? String, let url = context.file(path) else { return reply(["error": "NotFoundError"]) }
        let apple = args["apple"] as? [String: Any] ?? [:]
        let agent = context.agent
        work.async { [self] in
            let answer = Self.read(url, languages: matched, apple: apple, hatch: !agent)
            DispatchQueue.main.async { [self] in
                if let n = (answer["texts"] as? [Any])?.count { say("\(path): \(n) lines") }
                reply(answer)
            }
        }
    }

    static func read(_ url: URL, languages: [String], apple: [String: Any], hatch: Bool) -> [String: Any] {
        guard FileManager.default.fileExists(atPath: url.path) else { return ["error": "NotFoundError"] }
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil), let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            return ["error": "InvalidStateError", "message": "decode"]
        }
        let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any]
        let orientation = CGImagePropertyOrientation(rawValue: (properties?[kCGImagePropertyOrientation] as? NSNumber)?.uint32Value ?? 1) ?? .up
        let turned = [.left, .leftMirrored, .right, .rightMirrored].contains(orientation)
        let width = Double(turned ? image.height : image.width), height = Double(turned ? image.width : image.height)

        let request = VNRecognizeTextRequest()
        request.recognitionLanguages = languages
        if apple["recognitionLevel"] as? String == "fast" { request.recognitionLevel = .fast }
        if let correct = apple["usesLanguageCorrection"] as? Bool { request.usesLanguageCorrection = correct }
        if let words = apple["customWords"] as? [String] { request.customWords = words }
        if let height = (apple["minimumTextHeight"] as? NSNumber)?.floatValue { request.minimumTextHeight = height }
        if hatch { requestHatch?(request) }
        do {
            try VNImageRequestHandler(cgImage: image, orientation: orientation).perform([request])
        } catch {
            return ["error": "OperationError", "message": error.localizedDescription]
        }
        let point = { (p: CGPoint) -> [String: Any] in ["x": Double(p.x) * width, "y": (1 - Double(p.y)) * height] }
        let texts: [[String: Any]] = (request.results ?? []).compactMap { o in
            guard let top = o.topCandidates(1).first else { return nil }
            let b = o.boundingBox
            return [
                "rawValue": top.string,
                "boundingBox": ["x": Double(b.minX) * width, "y": (1 - Double(b.maxY)) * height, "width": Double(b.width) * width, "height": Double(b.height) * height],
                "cornerPoints": [o.topLeft, o.topRight, o.bottomRight, o.bottomLeft].map(point),
                "apple": ["confidence": Double(top.confidence)],
            ]
        }
        return ["texts": texts, "width": width, "height": height]
    }
}
