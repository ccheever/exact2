// A page's `window.print()` in a frame (@ref LLP 1020.002 D5): on macOS the
// platform's print panel, a sheet on the window, for the frame that called
// it, as Safari shows it. WebKit hands the call to its UI delegate only
// through `WKUIDelegatePrivate.h`'s
// `_webView:printFrame:pdfFirstPageSize:completionHandler:`, and
// prints a frame other than the main one only through `WKWebViewPrivate.h`'s
// `_printOperationWithPrintInfo:forFrame:`; the public
// `printOperation(with:)` prints the main frame alone. A frame showing a PDF
// prints the PDF's own pages. Under the agent the panel is a `print` request
// whose answer saves the job as a PDF file (§10). The iOS arm answers no
// print yet.
import Foundation
import WebKit

#if os(macOS)
import AppKit

extension GuestScheme {
    /// `speaker(_:)` for a page known by its URL alone.
    static func speaker(_ url: URL?) -> String {
        guard let url, let host = url.host, !host.isEmpty else { return url?.scheme.map { "\($0):" } ?? "This page" }
        return url.port.map { "\(host):\($0)" } ?? host
    }
}

extension FrameUI {
    @objc(_webView:printFrame:pdfFirstPageSize:completionHandler:)
    func webView(_ web: WKWebView, printFrame frame: NSObject, pdfFirstPageSize size: CGSize, completionHandler: @escaping () -> Void) {
        printFrame(web, frame, firstPage: size, done: completionHandler)
    }

    /// Prints `frame` of `web`, then `done`, which WebKit waits on before the
    /// page's `window.print()` returns.
    func printFrame(_ web: WKWebView, _ frame: NSObject, firstPage: CGSize, done: @escaping () -> Void) {
        let speaker = GuestScheme.speaker(web.url)
        guard web.responds(to: FramePrint.operation) else {
            log("a print from \(speaker) is refused: this WebKit has no -[WKWebView _printOperationWithPrintInfo:forFrame:]")
            return done()
        }
        // A frame data code drives prints nothing: no person is there to answer the panel.
        if web === arm?.built, arm?.automation?.controlled == true {
            log("a print from \(speaker) is refused while automation drives the frame")
            return done()
        }
        let title = web.title ?? ""
        if let arm, arm.agent {
            return arm.ask("print", ["origin": speaker, "title": title], from: web) { [weak self, weak web] answer in
                guard let self, let web, let path = answer?["path"] as? String else { return done() }
                self.run(web, frame, firstPage: firstPage, title: title, saving: URL(fileURLWithPath: path), done: done)
            }
        }
        // A page whose dialogs a person stopped prints nothing either.
        guard !asked(web, Self.origin(web.url)).suppressed else { return done() }
        guard !printing else {
            log("a print from \(speaker) is refused: another print is up")
            return done()
        }
        run(web, frame, firstPage: firstPage, title: title, saving: nil, done: done)
    }

    /// The print operation for `frame`: the panel as a sheet on the frame's
    /// window, or, with `saving`, the job written there as a PDF unshown.
    private func run(_ web: WKWebView, _ frame: NSObject, firstPage: CGSize, title: String, saving: URL?, done: @escaping () -> Void) {
        guard let window = web.window else {
            log("a print is cancelled: the frame is in no window")
            return done()
        }
        let info = (NSPrintInfo.shared.copy() as? NSPrintInfo) ?? NSPrintInfo()
        // WebKit names a PDF's first page size so that the paper can be turned to it.
        if firstPage.width > 0, firstPage.height > 0 { info.orientation = firstPage.width > firstPage.height ? .landscape : .portrait }
        if let saving {
            try? FileManager.default.createDirectory(at: saving.deletingLastPathComponent(), withIntermediateDirectories: true)
            info.jobDisposition = .save
            info.dictionary()[NSPrintInfo.AttributeKey.jobSavingURL] = saving
        }
        typealias Make = @convention(c) (AnyObject, Selector, NSPrintInfo, NSObject) -> NSPrintOperation?
        let make = unsafeBitCast(web.method(for: FramePrint.operation), to: Make.self)
        guard let operation = make(web, FramePrint.operation, info, frame) else {
            log("a print is cancelled: WebKit made no print operation for the frame")
            return done()
        }
        if !title.isEmpty { operation.jobTitle = title }
        operation.showsPrintPanel = saving == nil
        operation.showsProgressPanel = saving == nil
        // WebKit's printing view lays the pages out at its own size.
        operation.view?.frame = web.bounds
        let ending = FramePrint(done: done) { [weak self] in self?.printing = false }
        ending.ui = self
        printing = true
        operation.runModal(for: window, delegate: ending, didRun: #selector(FramePrint.didRun(_:success:contextInfo:)), contextInfo: Unmanaged.passRetained(ending).toOpaque())
        if saving == nil, let sheet = window.attachedSheet {
            ending.shown = showing(web) { [weak window, weak sheet] in if let sheet { window?.endSheet(sheet, returnCode: .cancel) } }
        }
    }
}

/// One print's end: AppKit's `didRun` selector frees it, then WebKit's
/// `window.print()` returns.
final class FramePrint: NSObject {
    static let operation = NSSelectorFromString("_printOperationWithPrintInfo:forFrame:")
    let done: () -> Void
    let ended: () -> Void
    weak var ui: FrameUI?
    var shown: Int?

    init(done: @escaping () -> Void, ended: @escaping () -> Void) {
        self.done = done
        self.ended = ended
    }

    @objc func didRun(_ operation: NSPrintOperation, success: Bool, contextInfo: UnsafeMutableRawPointer?) {
        if let contextInfo { Unmanaged<FramePrint>.fromOpaque(contextInfo).release() }
        if let shown { ui?.ended(shown) }
        ended()
        done()
    }
}

extension FramePopup {
    @objc(_webView:printFrame:pdfFirstPageSize:completionHandler:)
    func webView(_ web: WKWebView, printFrame frame: NSObject, pdfFirstPageSize size: CGSize, completionHandler: @escaping () -> Void) {
        guard let ui else { return completionHandler() }
        ui.printFrame(web, frame, firstPage: size, done: completionHandler)
    }
}
#endif
