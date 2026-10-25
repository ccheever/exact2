// A local PDF in a frame is PDFKit's `PDFView` (@ref LLP 1020.002 D10): an
// unpartitioned, unsandboxed `iframe` whose `src` is an `app:/` file or an
// asset. A partitioned, sandboxed or remote PDF stays WebKit's viewer.
import Foundation
import PDFKit

#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// `PDFView` at its automatic scale times the frame's `zoom`, opened at a
/// page once it has a size to scroll in.
final class ExactPDFView: PDFView {
    var zoom: CGFloat = 1 {
        didSet { if zoom != oldValue { settle() } }
    }
    /// The page `#page=N` asked for, until the view first lays out.
    var pendingPage: Int?

    func settle() {
        guard let document, bounds.width > 0, bounds.height > 0 else { return }
        if zoom == 1 {
            if !autoScales { autoScales = true }
        } else {
            if autoScales { autoScales = false }
            let want = scaleFactorForSizeToFit * zoom
            minScaleFactor = min(minScaleFactor, want)
            maxScaleFactor = max(maxScaleFactor, want)
            if want > 0, abs(scaleFactor - want) > 0.0001 { scaleFactor = want }
        }
        if let index = pendingPage, let page = document.page(at: index) {
            pendingPage = nil
            go(to: page)
        }
    }

    #if os(macOS)
    override func layout() {
        super.layout()
        settle()
    }
    #else
    override func layoutSubviews() {
        super.layoutSubviews()
        settle()
    }
    #endif
}

final class FramePDF: NSObject, PDFViewDelegate {
    weak var arm: WebArm?
    let view = ExactPDFView()

    init(arm: WebArm) {
        self.arm = arm
        super.init()
        view.displayMode = .singlePageContinuous
        view.displaysPageBreaks = true
        view.autoScales = true
        view.delegate = self
    }

    var pages: Int { view.document?.pageCount ?? 0 }

    /// The document, at `page` (0-based) when the source asked for one.
    func show(_ document: PDFDocument, page: Int?) {
        view.document = document
        view.pendingPage = page.map { max(0, min($0, document.pageCount - 1)) }
        view.settle()
    }

    func close() {
        view.delegate = nil
        view.document = nil
        view.removeFromSuperview()
    }

    /// `#page=N` among a source's fragment parameters (Adobe's PDF open
    /// parameters), as a 0-based index.
    static func page(_ source: String?) -> Int? {
        guard let fragment = source?.split(separator: "#", maxSplits: 1).dropFirst().first else { return nil }
        for parameter in fragment.split(separator: "&") {
            let pair = parameter.split(separator: "=", maxSplits: 1)
            if pair.count == 2, pair[0].lowercased() == "page", let n = Int(pair[1]), n >= 1 { return n - 1 }
        }
        return nil
    }

    /// A link in the document is the frame's to decide, not `PDFView`'s
    /// default, which opens the URL: an http(s) link is a `foreground-tab`
    /// for `newwindow=`, another app's scheme the platform's confirmation
    /// (D5).
    func pdfViewWillClick(onLink sender: PDFView, with url: URL) {
        guard let arm, !arm.invalidated else { return }
        let log = { (message: String) in FileHandle.standardError.write(Data("exact: iframe \(arm.id): \(message)\n".utf8)) }
        if GuestScheme.web(url) {
            guard arm.handlesNewWindow else { return log("a foreground-tab for \(url.absoluteString) is refused: the frame has no newwindow=") }
            return arm.emitRecord("newwindow", ["url": url.absoluteString, "disposition": "foreground-tab"])
        }
        guard let external = GuestScheme.external(url) else { return log("a link to \(url.scheme ?? "this"): is refused") }
        arm.ui?.confirmExternal(external, from: view)
    }

    /// The view as it shows now, `rect` in its points, y down, drawn in this
    /// process (a PDF view has no remote layers) at the screen's scale.
    func picture(_ rect: CGRect) -> CGImage? {
        #if os(macOS)
        let drawn = view.isFlipped ? rect : CGRect(x: rect.minX, y: view.bounds.height - rect.maxY, width: rect.width, height: rect.height)
        guard let rep = view.bitmapImageRepForCachingDisplay(in: drawn) else { return nil }
        view.cacheDisplay(in: drawn, to: rep)
        return rep.cgImage
        #else
        let format = UIGraphicsImageRendererFormat.preferred()
        format.scale = view.window?.screen.scale ?? UIScreen.main.scale
        let image = UIGraphicsImageRenderer(size: rect.size, format: format).image { context in
            context.cgContext.translateBy(x: -rect.minX, y: -rect.minY)
            if view.window == nil || !view.drawHierarchy(in: view.bounds, afterScreenUpdates: true) { view.layer.render(in: context.cgContext) }
        }
        return image.cgImage
        #endif
    }
}

extension WebArm {
    /// Shows `bytes` in a PDF view in place of the web view, which goes; false
    /// when PDFKit cannot read them, leaving the file to WebKit's viewer.
    func showPDF(_ bytes: Data) -> Bool {
        guard let document = PDFDocument(data: bytes) else { return false }
        generation += 1
        serving = false
        if built != nil { teardown() }
        let shown = pdf ?? FramePDF(arm: self)
        pdf = shown
        shown.view.zoom = CGFloat((options["zoom"] as? NSNumber)?.doubleValue ?? 1)
        shown.show(document, page: FramePDF.page(src))
        // The presenter shows the PDF view and the agent's `tree` its pages.
        emit(kind: 7, text: "{\"pdf\":\(shown.pages)}")
        emit(kind: 2)
        emit(kind: 0)
        return true
    }

    /// The source is no longer a local PDF: the presenter shows a web view
    /// again. It is made now, before `serve` adds a wrapper's bridge to its
    /// content controller, which a new web view replaces.
    func closePDF() {
        guard let shown = pdf else { return }
        shown.close()
        pdf = nil
        _ = webView
        emit(kind: 7, text: "{}")
    }
}
