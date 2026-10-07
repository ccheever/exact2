#if os(iOS)
// AttachmentFileScreen native menu and inline HTML document, pinned365aa87982.
// @ref llp/1107.005-composer-and-transcript.decision.md#media-presentation
import UIKit
import WebKit

final class T3MobileDocumentMenu: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let view = T3MobileDocumentMenu(events: events); try view.setProps(props); return view
    }
    private let button = UIButton(type: .system)
    private var alive = true
    override var view: UIView { button }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        button.setImage(UIImage(systemName: "ellipsis"), for: .normal)
        button.showsMenuAsPrimaryAction = true
        button.accessibilityLabel = "File actions"
    }
    override func setProps(_ props: [String: String]) throws {
        guard let data = props["document-menu"]?.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let identifier = config["identifier"] as? String else { throw ExactNativeRefusal("Invalid file menu.") }
        let rows = config["actions"] as? [[String: Any]] ?? []
        button.isEnabled = !identifier.isEmpty
        button.menu = UIMenu(children: rows.compactMap { row in
            guard let id = row["id"] as? String, let title = row["title"] as? String else { return nil }
            return UIAction(title: title, image: UIImage(systemName: row["icon"] as? String ?? ""),
                            attributes: id == "remove" ? .destructive : [],
                            state: row["selected"] as? Bool == true ? .on : .off) { [weak self] _ in
                guard let self, alive, let value = try? JSONSerialization.data(withJSONObject: ["identifier": identifier, "operation": id]) else { return }
                events.change(String(decoding: value, as: UTF8.self))
            }
        })
    }
    override func destroy() { alive = false; button.menu = nil }
}

private final class T3MobileHTMLNavigation: NSObject, WKNavigationDelegate {
    weak var owner: T3MobileDocumentHTML?
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        owner?.webView(webView, didFail: navigation, withError: error)
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        owner?.webView(webView, didFailProvisionalNavigation: navigation, withError: error)
    }
    func webView(_ webView: WKWebView, decidePolicyFor response: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        guard let owner else { decisionHandler(.cancel); return }
        owner.webView(webView, decidePolicyFor: response, decisionHandler: decisionHandler)
    }
}
final class T3MobileDocumentHTML: ExactNativeInstance {
    private let navigation = T3MobileHTMLNavigation()
    private let root = UIView()
    private var web: WKWebView
    private let files: T3MobileMediaFiles
    private let audioSession: T3MobileAudioSession
    private var audioOwner = UUID()
    private var identifier = ""
    private var opening: Task<Void, Never>?
    private var lease: URL?
    private var alive = true
    override var view: UIView { root }
    init(dataRoot: URL, audioSession: T3MobileAudioSession, events: ExactNativeEvents) {
        self.audioSession = audioSession
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        web = WKWebView(frame: .zero, configuration: configuration)
        files = T3MobileMediaFiles(dataRoot: dataRoot)
        super.init(events: events)
        attachWeb()
        audioSession.hold(audioOwner)
    }
    private func attachWeb() {
        navigation.owner = self
        web.navigationDelegate = navigation
        web.allowsBackForwardNavigationGestures = true
        web.isOpaque = false; web.backgroundColor = .clear
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.frame = root.bounds; web.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        root.addSubview(web)
    }
    override func setProps(_ props: [String: String]) throws {
        let source = try T3MobileMediaSource(props["media-source"] ?? "")
        guard source.identifier != identifier else { return }
        opening?.cancel()
        let retired = web, retiredOwner = audioOwner, session = audioSession
        audioOwner = UUID(); audioSession.hold(audioOwner)
        retired.navigationDelegate = nil; retired.stopLoading(); retired.removeFromSuperview()
        retired.setAllMediaPlaybackSuspended(true) { _ = retired; session.release(retiredOwner) }
        T3MobileMediaFiles.release(lease); lease = nil
        let configuration = WKWebViewConfiguration(); configuration.websiteDataStore = .nonPersistent()
        web = WKWebView(frame: root.bounds, configuration: configuration); attachWeb()
        identifier = source.identifier
        if let url = source.url { web.load(URLRequest(url: url)); return }
        opening = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let file = try await files.prepare(source)
                guard alive, !Task.isCancelled, identifier == source.identifier else { T3MobileMediaFiles.release(file); return }
                lease = file
                web.loadFileURL(file, allowingReadAccessTo: file.deletingLastPathComponent())
            } catch {
                guard alive, !Task.isCancelled, identifier == source.identifier,
                      let value = try? JSONSerialization.data(withJSONObject: ["identifier": source.identifier, "message": error.localizedDescription]) else { return }
                events.change(String(decoding: value, as: UTF8.self))
            }
        }
    }
    private func failed(_ message: String) {
        guard alive, let value = try? JSONSerialization.data(withJSONObject: ["identifier": identifier, "message": message]) else { return }
        events.change(String(decoding: value, as: UTF8.self))
    }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        if webView === web, (error as NSError).code != NSURLErrorCancelled { failed(error.localizedDescription) }
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        if webView === web, (error as NSError).code != NSURLErrorCancelled { failed(error.localizedDescription) }
    }
    func webView(_ webView: WKWebView, decidePolicyFor navigationResponse: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        guard webView === web, alive else { decisionHandler(.cancel); return }
        if navigationResponse.isForMainFrame, let response = navigationResponse.response as? HTTPURLResponse, response.statusCode >= 400 {
            failed("The file could not be loaded (HTTP \(response.statusCode)).")
            decisionHandler(.cancel)
        } else { decisionHandler(.allow) }
    }
    override func destroy() {
        web.navigationDelegate = nil
        guard alive else { return }; alive = false; web.removeFromSuperview()
        opening?.cancel(); opening = nil; web.stopLoading()
        let session = audioSession, token = audioOwner, retired = web
        retired.setAllMediaPlaybackSuspended(true) { _ = retired; session.release(token) }
        T3MobileMediaFiles.release(lease); lease = nil
    }
}
#endif
