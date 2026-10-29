// @ref LLP 1133.024 D4 — the share extension's compose sheet, as a
// hand-built app's: the system's `SLComposeServiceViewController` on iOS,
// which Xcode's template uses; on the Mac, where AppKit has none, a view
// controller as Xcode's macOS template builds one. Post writes the inbox
// (ShareInbox.swift) and ends; the extension never opens the app.
import Foundation
#if canImport(UIKit)
import Social
import UIKit
import UniformTypeIdentifiers

@objc(ExactShareViewController)
public final class ExactShareViewController: SLComposeServiceViewController {
    private var writer: ShareInboxWriter?
    /// The inbox's target and its container (the App Group's when nil); a test sets both.
    public var info: ShareTargetInfo? = ShareTargetInfo(bundle: .main)
    public var container: URL?

    public override func viewDidLoad() {
        super.viewDidLoad()
        title = Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String
    }

    public override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // The post button's word is the app's (`postTitle`, D6); the class has no property for it.
        if let word = Bundle.main.object(forInfoDictionaryKey: "ExactSharePostTitle") as? String {
            navigationController?.navigationBar.topItem?.rightBarButtonItem?.title = word
        }
    }

    public override func isContentValid() -> Bool { true }

    public override func didSelectPost() {
        let items = extensionContext?.inputItems as? [NSExtensionItem] ?? []
        let title = items.first?.attributedTitle?.string
        let text = contentText
        guard let info, let container = container ?? ShareInboxWriter.container(info),
              let writer = ShareInboxWriter(info: info, container: container) else {
            extensionContext?.cancelRequest(withError: NSError(domain: "exact.share", code: 1, userInfo: [NSLocalizedDescriptionKey: "The app's shared container is not available."]))
            return
        }
        self.writer = writer
        writer.take(items) { [weak self] in
            do { try writer.finish(title: title, text: text) } catch { writer.discard() }
            self?.extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
        }
    }

    public override func didSelectCancel() {
        writer?.discard()
        super.didSelectCancel()
    }
}
#elseif canImport(AppKit)
import AppKit
import UniformTypeIdentifiers

@objc(ExactShareViewController)
public final class ExactShareViewController: NSViewController {
    private let textView = NSTextView()
    private let thumbnail = NSImageView()
    private var writer: ShareInboxWriter?
    /// The inbox's target and its container (the App Group's when nil), and the extension's context; a test sets
    /// all three.
    public var info: ShareTargetInfo? = ShareTargetInfo(bundle: .main)
    public var container: URL?
    public var stubContext: NSExtensionContext?
    private var context: NSExtensionContext? { stubContext ?? extensionContext }
    /// The sheet's text, which the person may edit before Post.
    public var text: String { get { textView.string } set { textView.string = newValue } }

    public override func loadView() {
        let name = Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? ""
        let heading = NSTextField(labelWithString: name)
        heading.font = .boldSystemFont(ofSize: NSFont.systemFontSize)
        let scroll = NSScrollView()
        scroll.documentView = textView
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        textView.isRichText = false
        textView.font = .systemFont(ofSize: NSFont.systemFontSize)
        textView.autoresizingMask = [.width]
        thumbnail.imageScaling = .scaleProportionallyUpOrDown
        let cancel = NSButton(title: NSLocalizedString("Cancel", comment: ""), target: self, action: #selector(cancel(_:)))
        cancel.keyEquivalent = "\u{1b}"
        let post = NSButton(title: Bundle.main.object(forInfoDictionaryKey: "ExactSharePostTitle") as? String ?? NSLocalizedString("Post", comment: ""), target: self, action: #selector(post(_:)))
        post.keyEquivalent = "\r"
        let body = NSStackView(views: [scroll, thumbnail])
        body.orientation = .horizontal
        let buttons = NSStackView(views: [NSView(), cancel, post])
        buttons.orientation = .horizontal
        let stack = NSStackView(views: [heading, body, buttons])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        NSLayoutConstraint.activate([
            scroll.widthAnchor.constraint(greaterThanOrEqualToConstant: 280), scroll.heightAnchor.constraint(equalToConstant: 120),
            thumbnail.widthAnchor.constraint(equalToConstant: 72), thumbnail.heightAnchor.constraint(equalToConstant: 72),
            buttons.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -32),
        ])
        view = stack
    }

    public override func viewDidLoad() {
        super.viewDidLoad()
        let item = context?.inputItems.first as? NSExtensionItem
        textView.string = item?.attributedContentText?.string ?? ""
        // A preview of the first attachment that is a picture.
        if let provider = item?.attachments?.first(where: { $0.canLoadObject(ofClass: NSImage.self) }) {
            _ = provider.loadObject(ofClass: NSImage.self) { [weak self] image, _ in
                DispatchQueue.main.async { self?.thumbnail.image = image as? NSImage }
            }
        }
    }

    @objc public func post(_ sender: Any?) {
        let items = context?.inputItems as? [NSExtensionItem] ?? []
        let title = items.first?.attributedTitle?.string, text = textView.string
        guard let info, let container = container ?? ShareInboxWriter.container(info),
              let writer = ShareInboxWriter(info: info, container: container) else {
            context?.cancelRequest(withError: NSError(domain: "exact.share", code: 1, userInfo: [NSLocalizedDescriptionKey: "The app's shared container is not available."]))
            return
        }
        self.writer = writer
        writer.take(items) { [weak self] in
            do { try writer.finish(title: title, text: text) } catch { writer.discard() }
            self?.context?.completeRequest(returningItems: [], completionHandler: nil)
        }
    }

    @objc public func cancel(_ sender: Any?) {
        writer?.discard()
        context?.cancelRequest(withError: NSError(domain: NSCocoaErrorDomain, code: NSUserCancelledError))
    }
}
#endif
