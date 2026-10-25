// A picture of a frame's guest into an `app:/` file (@ref LLP 1020.002 D7):
// WebDriver BiDi's `browsingContext.captureScreenshot`, answered by the
// frame's `capture` record instead of a base64 string.
import Foundation
import ImageIO
import UniformTypeIdentifiers
import WebKit

#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension WebArm {
    /// `{op: "captureScreenshot", path, file, clip}`: `path` is the `app:/`
    /// path the command named, which the record carries back; `file` the
    /// file it names, empty when it names none; `clip` `[x, y, width,
    /// height]` in the guest's CSS pixels relative to its viewport, or none
    /// for the whole viewport.
    func capture(_ op: [String: Any]) {
        let path = op["path"] as? String ?? ""
        let answer: (Int, Int, String, String) -> Void = { [weak self] width, height, error, message in
            if !error.isEmpty { FileHandle.standardError.write(Data("exact: iframe \(self?.id ?? 0): captureScreenshot: \(message)\n".utf8)) }
            self?.emitRecord("capture", ["path": path, "width": width, "height": height, "error": error, "message": message])
        }
        guard let file = (op["file"] as? String).flatMap({ $0.isEmpty ? nil : URL(fileURLWithPath: $0) }) else {
            return answer(0, 0, "SyntaxError", "\(path) is not a file under app:/data, app:/cache or app:/tmp")
        }
        let clip = (op["clip"] as? [NSNumber]).flatMap { $0.count >= 4 ? $0.prefix(4).map(\.doubleValue) : nil }
        let write: (CGImage?, String) -> Void = { image, failure in
            guard let image else { return answer(0, 0, "UnknownError", failure) }
            FrameCaptures.encode.async {
                let written = FrameCaptures.write(image, to: file)
                DispatchQueue.main.async {
                    if let written { answer(0, 0, "UnknownError", "\(path): \(written)") } else { answer(image.width, image.height, "", "") }
                }
            }
        }
        if let pdf {
            // A PDF frame draws in-process, its clip in the view's points.
            let bounds = pdf.view.bounds
            let rect = clip.map { CGRect(x: $0[0], y: $0[1], width: $0[2], height: $0[3]) }?.intersection(bounds) ?? bounds
            guard bounds.width > 0, bounds.height > 0 else { return answer(0, 0, "InvalidStateError", "the frame has no box") }
            guard FrameCaptures.drawable(rect) else { return answer(0, 0, "IndexSizeError", "the clip is outside the frame's viewport") }
            return write(pdf.picture(rect), "the PDF view drew nothing")
        }
        let web = webView
        guard web.bounds.width > 0, web.bounds.height > 0 else { return answer(0, 0, "InvalidStateError", "the frame has no box") }
        guard let clip else { return snap(web, web.bounds, attempt: 0, write) }
        // A rect from `getBoundingClientRect()` is in client coordinates, the
        // document's less `scrollX` and `scrollY`; the web view shows the
        // visual viewport, at `pageLeft` and `pageTop` in the document and
        // scaled by a pinch. So the clip's offset is `pageLeft − scrollX`:
        // Chrome's `offsetLeft`, where `scrollX` is the layout viewport's, and
        // 0 in WebKit on macOS, whose `scrollX` and client coordinates follow
        // the visual viewport. `WKSnapshotConfiguration.rect` is in the view's
        // points, y down.
        web.evaluateJavaScript("[visualViewport.pageLeft - scrollX, visualViewport.pageTop - scrollY, visualViewport.scale]", in: nil, in: world) { [weak self, weak web] result in
            guard let self, let web, !self.invalidated else { return }
            let viewport = ((try? result.get()) as? [NSNumber])?.map(\.doubleValue) ?? []
            let (left, top, scale) = viewport.count == 3 ? (viewport[0], viewport[1], viewport[2]) : (0, 0, 1)
            let points = scale * Double(web.pageZoom)
            var rect = CGRect(x: (clip[0] - left) * points, y: (clip[1] - top) * points, width: clip[2] * points, height: clip[3] * points)
            #if os(iOS)
            rect.origin.y += web.scrollView.adjustedContentInset.top
            #endif
            rect = rect.intersection(web.bounds)
            guard FrameCaptures.drawable(rect) else { return answer(0, 0, "IndexSizeError", "the clip is outside the frame's viewport") }
            self.snap(web, rect, attempt: 0, write)
        }
    }

    /// `takeSnapshot` after the screen updates, three tries 120 ms apart, as
    /// the oracle and the clone try.
    func snap(_ web: WKWebView, _ rect: CGRect, attempt: Int, _ done: @escaping (CGImage?, String) -> Void) {
        let configuration = WKSnapshotConfiguration()
        configuration.rect = rect
        configuration.afterScreenUpdates = true
        web.takeSnapshot(with: configuration) { [weak self, weak web] image, error in
            if let image = image.flatMap(FrameCaptures.cgImage) { return done(image, "") }
            guard attempt < 2, let self, let web, !self.invalidated else { return done(nil, error?.localizedDescription ?? "WebKit returned no snapshot") }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) { self.snap(web, rect, attempt: attempt + 1, done) }
        }
    }
}

enum FrameCaptures {
    /// Encoding and writing stay off the main thread.
    static let encode = DispatchQueue(label: "exact.web.capture", qos: .userInitiated)

    /// A rect a picture can be made of: at least a point each way.
    static func drawable(_ rect: CGRect) -> Bool { !rect.isNull && rect.width >= 1 && rect.height >= 1 }

    static func cgImage(_ image: PlatformImage) -> CGImage? {
        #if os(macOS)
        image.cgImage(forProposedRect: nil, context: nil, hints: nil)
        #else
        image.cgImage
        #endif
    }

    static func png(_ image: CGImage) -> Data? {
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, nil)
        return CGImageDestinationFinalize(destination) ? data as Data : nil
    }

    /// The PNG at `file`, its folder made first; nil, or why it failed.
    static func write(_ image: CGImage, to file: URL) -> String? {
        guard let data = png(image) else { return "the PNG did not encode" }
        do {
            try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: file, options: .atomic)
            return nil
        } catch {
            return error.localizedDescription
        }
    }
}
