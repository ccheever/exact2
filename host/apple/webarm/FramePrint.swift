// A page's `window.print()` in a frame (@ref LLP 1020.002 D5): on macOS the
// platform's print panel, a sheet on the window, as Safari shows it. WebKit
// hands a page's print to its UI delegate only through private API, so the
// arm takes the call in the page instead: a script in the page's world, in
// every frame, makes `print()` an event that only the arm's own content
// world hears, and that world posts it with the frame it came from
// (`WKScriptMessage.frameInfo`). What can print through public API:
// - a web view's main frame, through `printOperation(with:)`, which prints
//   a page as WebKit lays it out for paper, or a PDF's own pages;
// - a frame below it whose document is a PDF: its bytes, read in the frame
//   while `print()` runs, printed by PDFKit.
// Any other frame below the main one has no public print operation: its
// print is refused in the log. Under the agent the panel is a `print`
// request whose answer saves the job as a PDF file (§10). The iOS arm
// answers no print yet.
import Foundation
import WebKit

#if os(macOS)
import AppKit
import PDFKit

extension GuestScheme {
    /// `speaker(_:)` for a page known by its URL alone.
    static func speaker(_ url: URL?) -> String {
        guard let url, let host = url.host, !host.isEmpty else { return url?.scheme.map { "\($0):" } ?? "This page" }
        return url.port.map { "\(host):\($0)" } ?? host
    }
}

/// Takes the pages' `print()` calls for one content controller: the arm's
/// web view's, or a popup's.
final class FramePrints: NSObject, WKScriptMessageHandlerWithReply {
    static let world = WKContentWorld.world(name: "exact.print")
    /// The event the page's `print()` becomes. A page cannot read it from the
    /// script's closure; one that guessed it would only call `print()`.
    static let event = "exact-print-\(UUID().uuidString)"
    /// Page world: `print()` fires the event, once the document has loaded,
    /// as WebKit's own `print()` waits for it. The functions are taken
    /// before the page's scripts run.
    static let pageScript = """
        (() => { const w = window, dispatch = EventTarget.prototype.dispatchEvent, listen = EventTarget.prototype.addEventListener, E = Event; let waiting = false;
        const go = () => dispatch.call(w, new E('\(event)'));
        w.print = new Proxy(w.print, { apply() { if (document.readyState === 'complete') go(); else if (!waiting) { waiting = true; listen.call(w, 'load', () => { waiting = false; go(); }, { once: true }); } } }); })()
        """
    /// The arm's world: posts the print. A PDF below the main frame goes
    /// with its bytes, read now because the page may remove the frame as
    /// soon as `print()` returns, and this world fires its `beforeprint` and
    /// `afterprint`, which WebKit fires itself only as it prints a main
    /// frame. A frame in a `sandbox` without `allow-modals` prints nothing,
    /// as WebKit's `print()` checks.
    static let worldScript = """
        (() => { const post = window.webkit.messageHandlers.exactPrint;
        addEventListener('\(event)', () => {
          for (let w = window; w !== w.parent; w = w.parent) { let f = null; try { f = w.frameElement } catch {} if (f && f.hasAttribute('sandbox') && !f.sandbox.contains('allow-modals')) return; }
          let pdf = null, title = document.title;
          try { title = top.document.title } catch {}
          const pdfFrame = window !== top && document.contentType === 'application/pdf';
          if (pdfFrame) {
            dispatchEvent(new Event('beforeprint'));
            try { const x = new XMLHttpRequest(); x.open('GET', location.href, false); x.overrideMimeType('text/plain; charset=x-user-defined'); x.send();
              const t = x.responseText, parts = [];
              for (let i = 0; i < t.length; i += 0x8000) { const b = new Uint8Array(Math.min(0x8000, t.length - i)); for (let j = 0; j < b.length; j++) b[j] = t.charCodeAt(i + j) & 0xff; parts.push(String.fromCharCode.apply(null, b)); }
              pdf = btoa(parts.join(''));
            } catch { pdf = ''; }
          }
          post.postMessage({ title, pdf }).catch(() => {}).then(() => { if (pdfFrame) dispatchEvent(new Event('afterprint')); });
        }); })()
        """

    weak var ui: FrameUI?

    init(ui: FrameUI) { self.ui = ui }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        let done = { replyHandler(nil, nil) }
        guard let ui, let web = message.webView, web === ui.arm?.built || ui.popups.contains(where: { $0.web === web }) else { return done() }
        let body = message.body as? [String: Any] ?? [:]
        ui.printFrame(web, message.frameInfo, title: body["title"] as? String, pdf: body["pdf"] as? String, done: done)
    }
}

extension FrameUI {
    /// Makes `controller`'s pages print through this delegate.
    func takePrints(on controller: WKUserContentController) {
        controller.addUserScript(WKUserScript(source: FramePrints.pageScript, injectionTime: .atDocumentStart, forMainFrameOnly: false, in: .page))
        controller.addUserScript(WKUserScript(source: FramePrints.worldScript, injectionTime: .atDocumentStart, forMainFrameOnly: false, in: FramePrints.world))
        controller.addScriptMessageHandler(FramePrints(ui: self), contentWorld: FramePrints.world, name: "exactPrint")
    }

    /// Prints `frame` of `web`, then `done`, which answers the page's post.
    func printFrame(_ web: WKWebView, _ frame: WKFrameInfo, title posted: String?, pdf: String?, done: @escaping () -> Void) {
        let speaker = GuestScheme.speaker(web.url)
        // A frame data code drives prints nothing: no person is there to answer the panel.
        if web === arm?.built, arm?.automation?.controlled == true {
            log("a print from \(speaker) is refused while automation drives the frame")
            return done()
        }
        if web === arm?.built, let sandbox = arm?.sandbox, arm?.direct == true, !sandbox.split(whereSeparator: \.isWhitespace).contains("allow-modals") {
            log("a print from \(speaker) is refused: its sandbox does not allow modals")
            return done()
        }
        let job: Job
        if frame.isMainFrame {
            job = .page
        } else if let pdf {
            guard let data = Data(base64Encoded: pdf), let document = PDFDocument(data: data) else {
                log("a print from \(speaker) is cancelled: its frame's PDF could not be read")
                return done()
            }
            job = .pdf(document)
        } else {
            log("a print from a frame in \(speaker) is not supported: macOS prints a page's main frame, or a PDF in a frame, and \(frame.request.url?.absoluteString ?? "this frame") is neither")
            return done()
        }
        let title = posted.flatMap { $0.isEmpty ? nil : $0 } ?? web.title ?? ""
        if let arm, arm.agent {
            return arm.ask("print", ["origin": speaker, "title": title], from: web) { [weak self, weak web] answer in
                guard let self, let web, let path = answer?["path"] as? String else { return done() }
                self.run(web, job, title: title, saving: URL(fileURLWithPath: path), done: done)
            }
        }
        // A page whose dialogs a person stopped prints nothing either.
        guard !asked(web, Self.origin(web.url)).suppressed else { return done() }
        guard !printing else {
            log("a print from \(speaker) is refused: another print is up")
            return done()
        }
        run(web, job, title: title, saving: nil, done: done)
    }

    enum Job { case page, pdf(PDFDocument) }

    /// The print operation for `job`: the panel as a sheet on the frame's
    /// window, or, with `saving`, the job written there as a PDF unshown.
    private func run(_ web: WKWebView, _ job: Job, title: String, saving: URL?, done: @escaping () -> Void) {
        guard let window = web.window else {
            log("a print is cancelled: the frame is in no window")
            return done()
        }
        let info = (NSPrintInfo.shared.copy() as? NSPrintInfo) ?? NSPrintInfo()
        if let saving {
            try? FileManager.default.createDirectory(at: saving.deletingLastPathComponent(), withIntermediateDirectories: true)
            info.jobDisposition = .save
            info.dictionary()[NSPrintInfo.AttributeKey.jobSavingURL] = saving
        }
        let operation: NSPrintOperation
        switch job {
        case .page:
            operation = web.printOperation(with: info)
            // WebKit's printing view lays the pages out at its own size.
            operation.view?.frame = web.bounds
        case .pdf(let document):
            // The paper is turned to the PDF's first page, each page scaled to fit and turned as Preview does.
            if let page = document.page(at: 0) {
                let size = page.bounds(for: .mediaBox).size, turned = page.rotation % 180 != 0
                if size.width != size.height { info.orientation = (size.width > size.height) != turned ? .landscape : .portrait }
            }
            guard let made = document.printOperation(for: info, scalingMode: .pageScaleToFit, autoRotate: true) else {
                log("a print is cancelled: PDFKit made no print operation for the frame's PDF")
                return done()
            }
            operation = made
        }
        if !title.isEmpty { operation.jobTitle = title }
        operation.showsPrintPanel = saving == nil
        operation.showsProgressPanel = saving == nil
        let ending = FramePrint(done: done) { [weak self] in self?.printing = false }
        ending.ui = self
        printing = true
        operation.runModal(for: window, delegate: ending, didRun: #selector(FramePrint.didRun(_:success:contextInfo:)), contextInfo: Unmanaged.passRetained(ending).toOpaque())
        if saving == nil, let sheet = window.attachedSheet {
            ending.shown = showing(web) { [weak window, weak sheet] in if let sheet { window?.endSheet(sheet, returnCode: .cancel) } }
        }
    }
}

/// One print's end: AppKit's `didRun` selector frees it, then the page's
/// post is answered.
final class FramePrint: NSObject {
    let done: () -> Void
    let ended: () -> Void
    weak var ui: FrameUI?
    var shown: Int?

    init(done: @escaping () -> Void, ended: @escaping () -> Void) {
        self.done = done
        self.ended = ended
    }

    @objc func didRun(_ operation: NSPrintOperation, success: Bool, contextInfo: UnsafeMutableRawPointer?) {
        if let shown { ui?.ended(shown) }
        ended()
        done()
        if let contextInfo { Unmanaged<FramePrint>.fromOpaque(contextInfo).release() }
    }
}
#else
extension FrameUI {
    /// iOS answers no print yet (@ref LLP 1020.002 D5).
    func takePrints(on controller: WKUserContentController) {}
}
#endif
