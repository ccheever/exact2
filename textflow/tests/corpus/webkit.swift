// record.mjs's WebKit: one WKWebView on the system WebKit.framework, the engine
// the installed Safari runs, without opening Safari or needing safaridriver.
// After Pretext's harness/webkit-host (MIT, © Pretext contributors).
//
//   webkit <page.html>
//
// Loads the page as a string and prints the one message it posts to
// `window.webkit.messageHandlers.result`, then exits 0; 1 on a failed load.
import AppKit
import WebKit

final class Host: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    var view: WKWebView?
    func start(_ html: String) {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.userContentController.add(self, name: "result")
        let view = WKWebView(frame: NSRect(x: 0, y: 0, width: 1440, height: 900), configuration: config)
        view.navigationDelegate = self
        view.loadHTMLString(html, baseURL: URL(string: "http://127.0.0.1/"))
        self.view = view
    }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        print(message.body as? String ?? "")
        exit(0)
    }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        FileHandle.standardError.write(Data("webkit: \(error)\n".utf8))
        exit(1)
    }
}

let arguments = CommandLine.arguments
guard arguments.count == 2, let html = try? String(contentsOfFile: arguments[1], encoding: .utf8) else {
    FileHandle.standardError.write(Data("usage: webkit <page.html>\n".utf8))
    exit(2)
}
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let host = Host()
host.start(html)
app.run()
