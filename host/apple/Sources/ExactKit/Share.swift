// `share(title=, text=, url=, files=)` (LLP 1069.003): the system share
// sheet, in ExactKit so an embedder (LLP 1031) gets it with the session. The
// runner rules first — bad data refused, and under the agent the request
// held for `tap @t shared|cancel`, never a sheet (D6), unless
// `EXACT_DEVICE=real` asks for the real one (LLP 1069.007 D8) — then iOS opens a
// UIActivityViewController and macOS an NSSharingServicePicker, anchored to
// the node whose input ran the action (D3). The outcome is a journal line
// only (D2). Native hosts share from any action (Q2): a timer's share opens
// at the window's centre. `files=` (LLP 1125 D12): an `app:/` file goes as
// a copy read as `readFile` reads it; an `"#id"` as a PNG of that node's view as it is drawn now,
// at the display's scale, written to a scratch directory removed when the
// sheet closes. Under the agent nothing is drawn: the hold records the
// entries. A drive that shows UIKit's own presentations (`--touch platform`,
// `ExactEnv.agentPaints` false) keeps the hold and presents the real sheet
// too: a window screenshot shows it, `back` or `tap @t cancel` closes it,
// and its outcome answers the hold, journalled as the agent's answer is.
#if canImport(UIKit)
import UIKit
typealias ShareAnchorView = UIView
#else
import AppKit
typealias ShareAnchorView = NSView
#endif

extension ExactSession {
    func share(_ args: [Any], source: UInt32?) {
        var request: [String: Any] = ["command": "share", "agent": PageFacts.substituted]
        if PageFacts.substituted && !ExactEnv.agentPaints { request["present"] = true }
        for (i, key) in ["title", "text", "url"].enumerated() where i < args.count {
            if let value = args[i] as? String { request[key] = value }
        }
        if args.count > 3, let files = args[3] as? [Any] { request["files"] = files }
        if let source { request["source"] = source }
        let ruling = runtime.command(request)
        guard ruling["present"] as? Bool == true else { return }
        // The hold a shown sheet answers with its outcome (`share: shared`
        // is `tap @t shared`; dismissed or failed, `cancel`).
        let ticket = (ruling["ticket"] as? NSNumber)?.uint64Value
        let outcome: (String) -> Void = { [weak self] line in
            guard let self else { return }
            guard let ticket else { self.log(line); return }
            ShareSheet.shown[ShareSheet.Key(self, ticket)] = nil
            if line.hasPrefix("share: refused") { self.log(line) }
            _ = self.agent("{\"op\":\"tap\",\"ticket\":\(ticket),\"choice\":\"\(line == "share: shared" ? "shared" : "cancel")\"}")
        }
        var items: [Any] = []
        if let text = request["text"] as? String { items.append(text) }
        if let url = (request["url"] as? String).flatMap(URL.init(string:)) { items.append(url) }
        // Each entry is written into a folder of its own in a scratch
        // directory removed when the sheet closes: an app file as a copy read
        // through the library's confined path (`fs.read` admitted, no link
        // followed at any component), a picture as its PNG.
        var scratch: URL?
        let discard = { if let scratch { try? FileManager.default.removeItem(at: scratch) } }
        for (index, file) in (ruling["files"] as? [[String: Any]] ?? []).enumerated() {
            do {
                let dir = scratch ?? FileManager.default.temporaryDirectory.appendingPathComponent("exact-share-\(UUID().uuidString)", isDirectory: true)
                scratch = dir
                if let path = file["path"] as? String {
                    let url = try ShareSheet.scratchFile(dir, index, (path as NSString).lastPathComponent)
                    let ask = try JSONSerialization.data(withJSONObject: ["op": "shareFile", "path": path, "to": url.path])
                    let reply = (try? JSONSerialization.jsonObject(with: Data(agent(String(decoding: ask, as: UTF8.self)).utf8))) as? [String: Any]
                    guard reply?["file"] is String else {
                        discard(); outcome("share: refused: \(reply?["error"] as? String ?? "\(path) could not be read")"); return
                    }
                    items.append(url)
                } else if let id = file["id"] as? String, let view = (file["view"] as? NSNumber)?.uint32Value {
                    guard let node = presenter.views[view], let picture = ShareSheet.picture(of: node) else {
                        discard(); outcome("share: refused: #\(id) is not drawn"); return
                    }
                    let url = try ShareSheet.scratchFile(dir, index, ShareSheet.pictureName(id, title: request["title"] as? String))
                    try picture.png.write(to: url)
                    items.append(url)
                    log("share: picture of #\(id): \(picture.width)×\(picture.height) px, \(picture.png.count) bytes")
                }
            } catch {
                discard(); outcome("share: refused: \(error.localizedDescription)"); return
            }
        }
        let (view, rect) = presenter.shareAnchor(source)
        #if os(iOS)
        let barItem = source.flatMap { presenter.navigation.shownBarItem($0) }
        #else
        let barItem: AnyObject? = nil
        #endif
        guard let view, let sheet = ShareSheet.present(items, title: request["title"] as? String, from: view, at: rect, barItem: barItem,
                                                       outcome: { line in discard(); outcome(line) })
        else { discard(); outcome("share: refused: no window to present from"); return }
        if let ticket { ShareSheet.shown[ShareSheet.Key(self, ticket)] = (sheet, { discard(); outcome("share: dismissed") }) }
    }
}

extension Presenter {
    /// D3: the pressed node's view; a menu row's popover invoker when the
    /// row itself is not on screen (a menu item); else the window's content,
    /// centred (a timer, an answer, a menu-bar command).
    func shareAnchor(_ source: UInt32?) -> (ShareAnchorView?, CGRect) {
        func shown(_ v: ShareAnchorView) -> Bool {
            var at: ShareAnchorView? = v
            while let a = at { if a.isHidden { return false }; at = a.superview }
            return v.window != nil
        }
        if let id = source, let v = views[id] {
            if shown(v) { return (v, v.bounds) }
            var at: ShareAnchorView? = v.superview
            while let a = at, (a as? NodeView)?.props["popover"] == nil { at = a.superview }
            if let popover = at as? NodeView,
               let invoker = (carrying("popovertarget") + carrying("commandfor")).first(where: { invocation(of: $0)?.target === popover && shown($0) }) {
                return (invoker, invoker.bounds)
            }
        }
        #if canImport(UIKit)
        let content = ViewPresentation.presenter(for: root)?.viewIfLoaded
        #else
        let content = views.values.first(where: { $0.window != nil })?.window?.contentView
        #endif
        guard let content else { return (nil, .zero) }
        return (content, CGRect(x: content.bounds.midX, y: content.bounds.midY, width: 0, height: 0))
    }
}

enum ShareSheet {
    /// A shown sheet's hold: tickets are each session's runner's, and a
    /// process can hold two sessions (the sample host, LLP 1031).
    struct Key: Hashable {
        let session: ObjectIdentifier, ticket: UInt64
        init(_ session: ExactSession, _ ticket: UInt64) { self.session = ObjectIdentifier(session); self.ticket = ticket }
    }
    /// The real sheets a drive shows beside their holds: each one's sheet
    /// and what closes it as dismissed.
    nonisolated(unsafe) static var shown: [Key: (sheet: AnyObject, dismissed: () -> Void)] = [:]

    /// The agent answered a share's hold in `session` (`tap @t shared|cancel`):
    /// its sheet closes, the answer already journalled.
    static func answered(_ reply: [String: Any], in session: ExactSession) {
        guard reply["capability"] as? String == "share", let ticket = (reply["ticket"] as? NSNumber)?.uint64Value,
              let open = shown.removeValue(forKey: Key(session, ticket)) else { return }
        close(open.sheet)
    }

    /// `back` in `session` with a sheet of its shown: the newest closes,
    /// dismissed, as a person's swipe down or Close button closes it. False
    /// with none.
    static func agentBack(in session: ExactSession) -> Bool {
        let mine = ObjectIdentifier(session)
        guard let key = shown.keys.filter({ $0.session == mine }).max(by: { $0.ticket < $1.ticket }),
              let open = shown.removeValue(forKey: key) else { return false }
        close(open.sheet)
        open.dismissed()
        return true
    }

    /// Whether a sheet `session` shows has yet to finish arriving: UIKit
    /// presents it on a later turn and then animates it in, and a picture
    /// taken before would show the app alone or the sheet half in.
    static func arriving(in session: ExactSession) -> Bool {
        #if canImport(UIKit)
        let mine = ObjectIdentifier(session)
        return shown.contains { key, open in
            guard key.session == mine, let sheet = open.sheet as? UIViewController else { return false }
            return sheet.presentingViewController == nil || sheet.isBeingPresented
        }
        #else
        return false
        #endif
    }

    private static func close(_ sheet: AnyObject) {
        #if canImport(UIKit)
        (sheet as? UIViewController)?.dismiss(animated: !ExactEnv.agentFreezes)
        #else
        _ = sheet // no drive shows the Mac's picker beside a hold (`--touch platform` is iOS's)
        #endif
    }

    /// Where entry `index` of a share is written under `dir`: a folder of its
    /// own, so two entries whose names come out alike (`#card.one` and
    /// `#card-one`, or two files of one name) stay two files, each with a
    /// readable name for whoever receives it.
    static func scratchFile(_ dir: URL, _ index: Int, _ name: String) throws -> URL {
        let folder = dir.appendingPathComponent(String(index), isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        return folder.appendingPathComponent(name)
    }
    /// A picture's file name, which the share sheet's header and whoever
    /// receives it show: the share's title, as a person names a file they
    /// save (`/` and `:`, which a file name cannot hold, and control
    /// characters as `-`); with no title, the id's letters, digits, `-` and
    /// `_`, others as `-`. Either is cut at a character to at most 251 bytes
    /// of UTF-8, so that it and `.png` fit APFS's 255.
    static func pictureName(_ id: String, title: String? = nil) -> String {
        func fit(_ text: String, _ keep: (Unicode.Scalar) -> Bool) -> String {
            var scalars = String.UnicodeScalarView()
            for s in text.unicodeScalars { scalars.append(keep(s) ? s : "-") }
            var name = ""
            for c in String(scalars) {
                guard name.utf8.count + c.utf8.count <= 251 else { break }
                name.append(c)
            }
            return name
        }
        // Control characters only: `CharacterSet.controlCharacters` also
        // holds the format characters, the joiner of an emoji sequence.
        let named = fit((title ?? "").trimmingCharacters(in: .whitespacesAndNewlines)) {
            $0 != "/" && $0 != ":" && $0.properties.generalCategory != .control
        }
        if !named.isEmpty { return named + ".png" }
        return fit(id) { CharacterSet.alphanumerics.contains($0) || $0 == "-" || $0 == "_" } + ".png"
    }
    /// A PNG of `view` and its subviews as they are drawn now, at the
    /// display's scale, and its size in pixels; nil for a view with no area.
    static func picture(of view: ShareAnchorView) -> (png: Data, width: Int, height: Int)? {
        let bounds = view.bounds
        guard bounds.width > 0, bounds.height > 0 else { return nil }
        #if canImport(UIKit)
        let format = UIGraphicsImageRendererFormat.preferred()
        let scale = view.traitCollection.displayScale
        format.scale = scale > 0 ? scale : format.scale
        format.opaque = false
        let image = UIGraphicsImageRenderer(bounds: bounds, format: format).image { context in
            // On screen, the render server's composite (what a person sees);
            // off it, the layer tree drawn here.
            if view.window == nil || !view.drawHierarchy(in: bounds, afterScreenUpdates: false) {
                view.layer.render(in: context.cgContext)
            }
        }
        guard let png = image.pngData(), let cg = image.cgImage else { return nil }
        return (png, cg.width, cg.height)
        #else
        guard let rep = view.bitmapImageRepForCachingDisplay(in: bounds) else { return nil }
        view.cacheDisplay(in: bounds, to: rep)
        guard let png = rep.representation(using: .png, properties: [:]) else { return nil }
        return (png, rep.pixelsWide, rep.pixelsHigh)
        #endif
    }

    /// Open the platform's sheet with `items` off `view`'s `rect`, or off
    /// the bar item the pressed control is drawn as; `outcome` gets the
    /// journal line. Returns what was shown, nil when nothing could present it.
    @discardableResult
    static func present(_ items: [Any], title: String?, from view: ShareAnchorView, at rect: CGRect, barItem: AnyObject? = nil,
                        outcome: @escaping (String) -> Void) -> AnyObject? {
        #if os(tvOS)
        // tvOS has no share sheet.
        return nil
        #elseif canImport(UIKit)
        guard let controller = ViewPresentation.presenter(for: view) else { return nil }
        let sheet = UIActivityViewController(activityItems: items.map { ShareItem($0, title: title) }, applicationActivities: nil)
        if let barItem = barItem as? UIBarButtonItem {
            // The sheet grows out of its button, as a hand-built bar's does (iOS 26).
            sheet.popoverPresentationController?.sourceItem = barItem
            if #available(iOS 26.0, *) { sheet.preferredTransition = .zoom(sourceBarButtonItemProvider: { _ in barItem }) }
        } else {
            sheet.popoverPresentationController?.sourceView = view
            sheet.popoverPresentationController?.sourceRect = rect
        }
        sheet.completionWithItemsHandler = { _, completed, _, error in
            outcome(error.map { "share: refused: \($0.localizedDescription)" } ?? (completed ? "share: shared" : "share: dismissed"))
        }
        controller.present(sheet, animated: !ExactEnv.agentFreezes)
        return sheet
        #else
        _ = title  // the picker has no title; the services take the items
        guard view.window != nil else { return nil }
        let picker = NSSharingServicePicker(items: items)
        let delegate = ShareOutcome(outcome)
        picker.delegate = delegate
        objc_setAssociatedObject(picker, &ShareOutcome.key, delegate, .OBJC_ASSOCIATION_RETAIN)
        picker.show(relativeTo: rect, of: view, preferredEdge: .minY)
        return picker
        #endif
    }
}

#if os(tvOS)
#elseif canImport(UIKit)
/// One shared item; the title is the subject a mail or message takes.
private final class ShareItem: NSObject, UIActivityItemSource {
    let item: Any, title: String?
    init(_ item: Any, title: String?) { self.item = item; self.title = title }
    func activityViewControllerPlaceholderItem(_: UIActivityViewController) -> Any { item }
    func activityViewController(_: UIActivityViewController, itemForActivityType _: UIActivity.ActivityType?) -> Any? { item }
    func activityViewController(_: UIActivityViewController, subjectForActivityType _: UIActivity.ActivityType?) -> String { title ?? "" }
}
#else
/// The picker's outcome into the journal: no service chosen is `dismissed`;
/// a chosen one reports `shared`, or its failure.
private final class ShareOutcome: NSObject, NSSharingServicePickerDelegate, NSSharingServiceDelegate {
    nonisolated(unsafe) static var key = 0
    let outcome: (String) -> Void
    init(_ outcome: @escaping (String) -> Void) { self.outcome = outcome }
    func sharingServicePicker(_: NSSharingServicePicker, didChoose service: NSSharingService?) {
        if service == nil { outcome("share: dismissed") }
    }
    func sharingServicePicker(_: NSSharingServicePicker, delegateFor _: NSSharingService) -> NSSharingServiceDelegate? { self }
    func sharingService(_: NSSharingService, didShareItems _: [Any]) { outcome("share: shared") }
    func sharingService(_: NSSharingService, didFailToShareItems _: [Any], error: Error) {
        outcome((error as NSError).code == NSUserCancelledError ? "share: dismissed" : "share: refused: \(error.localizedDescription)")
    }
}
#endif
