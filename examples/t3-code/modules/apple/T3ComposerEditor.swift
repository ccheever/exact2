#if os(macOS)
import AppKit

/// Intercepts OS attachment pastes, folder drops and the open command menus'
/// keys. Exact remains the text/selection delegate and every other command's;
/// its shared field owns every source edit.
final class T3ComposerTextDelegate: NSObject, NSTextViewDelegate {
    weak var inner: NSObject?
    weak var editor: T3ComposerEditor?

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || inner?.responds(to: selector) == true
    }
    override func forwardingTarget(for selector: Selector!) -> Any? {
        inner?.responds(to: selector) == true ? inner : super.forwardingTarget(for: selector)
    }
    func textView(_ textView: NSTextView, shouldChangeTextIn range: NSRange, replacementString text: String?) -> Bool {
        if editor?.shouldChange(textView, range: range, text: text) == false { return false }
        return (inner as? NSTextViewDelegate)?.textView?(textView, shouldChangeTextIn: range, replacementString: text) ?? true
    }
    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if editor?.menuCommand(selector, in: textView) == true { return true }
        return (inner as? NSTextViewDelegate)?.textView?(textView, doCommandBy: selector) ?? false
    }
}

/// Observes the mounted composer and holds OS facts until the shared field
/// accepts or refuses them. It never edits text, selection, styles or undo.
final class T3ComposerEditor {
    private let changed: (String) -> Void
    private(set) weak var textView: NSTextView?
    private weak var element: ExactElement?
    private let proxy = T3ComposerTextDelegate()
    private final class Key { weak var element: ExactElement?; init(_ element: ExactElement) { self.element = element } }
    private var keys: [String: Key] = [:]
    private struct Owner: Equatable {
        let snapshot: String
        let field: String
        var valid: Bool { !snapshot.isEmpty && !field.isEmpty }
    }
    private var owner = Owner(snapshot: "", field: "")
    static let maxRecordID = 9_007_199_254_740_991
    static func nextRecordID(after current: Int) -> Int? {
        current >= 0 && current < maxRecordID ? current + 1 : nil
    }
    private var recordID = 0
    private(set) var folds: [[String: Any]] = []
    private(set) var drops: [[String: Any]] = []
    private(set) var notices: [[String: String]] = []
    private(set) var localEnvironment = false
    private(set) var foldLimit = 0
    var pastingAsText = false
    var pasteDetector: (String) -> Bool = T3ComposerEditor.isPaste
    var dragPasteboard: () -> NSPasteboard = { NSPasteboard(name: .drag) }
    var pasteboard: () -> NSPasteboard = { .general }
    var pasteDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-composer-paste", isDirectory: true)

    init(changed: @escaping (String) -> Void) {
        self.changed = changed
        proxy.editor = self
    }

    func install(_ element: ExactElement) {
        if element.hatch == .t3ComposerKey {
            let name = element.data[.anchor] ?? ""
            if !name.isEmpty { keys[name] = Key(element) }
            return
        }
        guard element.hatch == .t3Composer else { return }
        if self.element !== element || textView !== element.textView {
            detach()
            self.element = element
            if let view = element.textView {
                textView = view
                proxy.inner = view.delegate as? NSObject
                view.delegate = proxy
                T3ComposerPasteHook.attach(self, to: view)
            }
        }
        refreshOwner()
    }

    func remove(_ element: ExactElement) {
        for (name, key) in keys where key.element == nil || key.element === element { keys[name] = nil }
        if self.element === element { detach() }
    }
    func destroy() { detach(); keys.removeAll() }

    private func detach() {
        if let view = textView {
            T3ComposerPasteHook.detach(view)
            if view.delegate === proxy { view.delegate = proxy.inner as? NSTextViewDelegate }
        }
        proxy.inner = nil
        textView = nil
        element = nil
        resetOwner(Owner(snapshot: "", field: ""))
    }

    func key(_ name: String) -> ExactElement? {
        guard let element = keys[name]?.element, element.isLive else { return nil }
        return element
    }

    /// An open command or stash menu's keys (ChatComposer onComposerCommandKey):
    /// its hidden key buttons are mounted only while it is open, and take ↑, ↓,
    /// Return, Tab (the highlighted item) and Escape before the field's own command.
    func menuCommand(_ selector: Selector, in view: NSTextView) -> Bool {
        guard view === textView, view.isEditable, !view.hasMarkedText() else { return false }
        switch selector {
        case #selector(NSResponder.moveDown(_:)): return press("ArrowDown") || press("stash-ArrowDown")
        case #selector(NSResponder.moveUp(_:)): return press("ArrowUp") || press("stash-ArrowUp")
        case #selector(NSResponder.insertNewline(_:)), #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:)):
            return press("Enter") || press("stash-Enter")
        case #selector(NSResponder.insertTab(_:)): return press("Enter")
        case #selector(NSResponder.cancelOperation(_:)): return press("Escape")
        case #selector(NSResponder.deleteToBeginningOfLine(_:)): return press("stash-Delete")
        default: return false
        }
    }
    private func press(_ name: String) -> Bool {
        guard let key = key(name) else { return false }
        key.click()
        return true
    }

    private func refreshOwner() {
        let mounted = element.flatMap { $0.isLive ? $0 : nil }
        let next = Owner(snapshot: mounted?.data[.snapshotOwner] ?? "", field: mounted?.data[.owner] ?? "")
        if next != owner { resetOwner(next) }
    }
    private func resetOwner(_ next: Owner) {
        discardPastedFiles()
        folds.removeAll(); drops.removeAll(); notices.removeAll()
        localEnvironment = false; foldLimit = 0
        owner = next
    }
    private func matches(_ request: [String: Any]) -> Bool {
        owner.valid && request["snapshotOwner"] as? String == owner.snapshot && request["fieldOwner"] as? String == owner.field
    }

    func sync(_ request: [String: Any]) -> [String: Any] {
        refreshOwner()
        if matches(request) {
            if let local = request["localEnvironment"] as? Bool { localEnvironment = local }
            if let limit = request["foldLimit"] as? Int { foldLimit = max(0, limit) }
        }
        let result: [String: Any] = ["snapshotOwner": owner.snapshot, "fieldOwner": owner.field,
            "focused": textView.map { $0.window?.firstResponder === $0 } ?? false,
            "composing": textView?.hasMarkedText() ?? false,
            "notices": notices, "folds": folds, "drops": drops, "pastes": pastedFiles.count]
        if matches(request) { notices.removeAll() }
        return result
    }

    func forgetFold(_ id: Int) {
        refreshOwner()
        folds.removeAll { $0["id"] as? Int == id }
        drops.removeAll { $0["id"] as? Int == id }
    }

    private func refuse(_ title: String, _ description: String) {
        notices.append(["kind": "error", "title": title, "description": description])
        changed("t3.editor")
    }

    /// The observed full source and UTF-16 range are evidence for a later
    /// guarded shared-field edit, never authority to mutate a replacement field.
    private func record(_ view: NSTextView, range: NSRange) -> [String: Any]? {
        refreshOwner()
        let value = view.string as NSString
        guard view === textView, owner.valid, range.location >= 0, range.length >= 0,
              range.location <= value.length, range.length <= value.length - range.location else { return nil }
        guard folds.count + drops.count < 4 else {
            refuse("Too many pending pastes or drops", "Wait for the previous attachments, then try again.")
            return nil
        }
        guard let next = Self.nextRecordID(after: recordID) else {
            refuse("Attachment queue is unavailable", "Close and reopen this window before pasting or dropping more attachments.")
            return nil
        }
        recordID = next
        return ["id": recordID, "snapshotOwner": owner.snapshot, "fieldOwner": owner.field,
            "value": view.string, "start": range.location, "end": NSMaxRange(range), "expect": value.substring(with: range)]
    }

    func droppedFolders(_ text: String) -> [URL]? {
        if let event = NSApp.currentEvent, event.type == .keyDown { return nil }
        guard let urls = dragPasteboard().readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL], !urls.isEmpty else { return nil }
        let paths = urls.map(\.path)
        guard paths.contains(text) || paths.joined(separator: "\n") == text || paths.joined(separator: " ") == text else { return nil }
        return urls.allSatisfy { url in
            var directory: ObjCBool = false
            return FileManager.default.fileExists(atPath: url.path, isDirectory: &directory) && directory.boolValue
        } ? urls : nil
    }

    private func dropFolders(_ folders: [URL], view: NSTextView, range: NSRange) {
        refreshOwner()
        guard localEnvironment else {
            refuse("Folders can't be dropped into remote environments", "Type the folder path with @ instead.")
            return
        }
        guard folders.count <= 100 else {
            refuse("Too many folders", "Drop at most 100 folders at a time.")
            return
        }
        guard var fact = record(view, range: range) else { return }
        fact["paths"] = folders.map(\.path)
        drops.append(fact)
        changed("t3.editor")
    }

    func shouldChange(_ view: NSTextView, range: NSRange, text: String?) -> Bool {
        guard view === textView, let text, view.isEditable, !view.hasMarkedText() else { return true }
        if let folders = droppedFolders(text) { dropFolders(folders, view: view, range: range); return false }
        if !pastingAsText, !text.isEmpty, let allowed = foldPaste(view, range: range, text: text) { return allowed }
        return true
    }

    static let promptLimit = 120_000
    static let foldThreshold = 32 * 1024

    func foldPaste(_ view: NSTextView, range: NSRange, text: String) -> Bool? {
        refreshOwner()
        let source = view.string as NSString
        guard range.location >= 0, range.length >= 0, range.location <= source.length,
              range.length <= source.length - range.location else { return nil }
        let length = (text as NSString).length, bytes = text.utf8.count
        let exceeds = source.length - range.length + length > Self.promptLimit
        let large = length >= Self.foldThreshold || bytes >= Self.foldThreshold
        guard large || exceeds, pasteDetector(text) else { return nil }
        if foldLimit <= 0 || bytes > foldLimit {
            guard exceeds else { return nil }
            let tooLarge = foldLimit > 0
            refuse(tooLarge ? "Pasted text is too large to attach" : "Pasted text is too large for this message",
                tooLarge ? "Reduce the clipboard contents or save a smaller excerpt as a file." : "Remove some text or an attachment, then paste again.")
            return false
        }
        guard var fact = record(view, range: range) else { return false }
        fact["text"] = text
        folds.append(fact)
        changed("t3.editor")
        return false
    }

    struct PastedFile {
        let url: URL
        let asText: Bool
        fileprivate let snapshotOwner: String
        fileprivate let fieldOwner: String
        fileprivate let temporary: Bool
    }
    private(set) var pastedFiles: [PastedFile] = []

    static func pasteClaim(_ pasteboard: NSPasteboard) -> (urls: [URL], image: Data?)? {
        let urls = (pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]) ?? []
        if !urls.isEmpty {
            let files = urls.filter { url in
                var directory: ObjCBool = false
                return FileManager.default.fileExists(atPath: url.path, isDirectory: &directory) && !directory.boolValue
            }
            guard !files.isEmpty else { return nil }
            if files.contains(where: { T3ComposerAttach.kind(of: $0) != "file" }) { return (files, nil) }
            let text = pasteboard.string(forType: .string) ?? ""
            let names = Set(urls.flatMap { [$0.lastPathComponent, $0.path] })
            let lines = text.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
            return lines.allSatisfy(names.contains) ? (files, nil) : nil
        }
        if let png = pasteboard.data(forType: .png) { return ([], png) }
        guard let tiff = pasteboard.data(forType: .tiff), let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else { return nil }
        return ([], png)
    }

    var takesPastedFiles: Bool { true }

    func pasteFiles(from pasteboard: NSPasteboard, asText: Bool = false) -> Bool {
        refreshOwner()
        guard owner.valid, let claim = Self.pasteClaim(pasteboard) else { return false }
        guard pastedFiles.count + claim.urls.count + (claim.image == nil ? 0 : 1) <= 100 else {
            refuse("Too many pasted files", "Paste at most 100 files at a time.")
            return true
        }
        var urls = claim.urls
        if let image = claim.image {
            let folder = pasteDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            let file = folder.appendingPathComponent("image.png")
            do {
                try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                try image.write(to: file, options: .atomic)
            } catch {
                try? FileManager.default.removeItem(at: folder)
                refuse("Image could not be pasted", "Try copying the image again.")
                return true
            }
            urls = [file]
        }
        pastedFiles += urls.map { PastedFile(url: $0, asText: asText, snapshotOwner: owner.snapshot,
            fieldOwner: owner.field, temporary: claim.image != nil) }
        changed("t3.editor")
        return true
    }

    func takePastedFiles(_ request: [String: Any], stage: (URL) -> [String: Any]) -> [[String: Any]] {
        refreshOwner()
        guard matches(request) else { return [] }
        let held = pastedFiles
        pastedFiles.removeAll()
        defer { cleanTemporaryFiles(held) }
        return held.filter { $0.snapshotOwner == owner.snapshot && $0.fieldOwner == owner.field }.map { file in
            var staged = stage(file.url)
            if file.asText { staged["asText"] = true }
            return staged
        }
    }

    private func discardPastedFiles() {
        cleanTemporaryFiles(pastedFiles)
        pastedFiles.removeAll()
    }
    private func cleanTemporaryFiles(_ files: [PastedFile]) {
        for file in files where file.temporary {
            try? FileManager.default.removeItem(at: file.url.deletingLastPathComponent())
        }
    }

    static func isPaste(_ text: String) -> Bool {
        guard let event = NSApp.currentEvent, event.type == .keyDown,
              event.modifierFlags.intersection([.command, .control, .option]) == .command,
              event.charactersIgnoringModifiers?.lowercased() == "v" else {
            return NSPasteboard.general.string(forType: .string) == text
        }
        return true
    }

    deinit { destroy() }
}

/// Wrap only installed composer views; every other view keeps its own paste
/// implementation. Paste as Text still attaches files but leaves text inline.
enum T3ComposerPasteHook {
    private static var installed: Set<ObjectIdentifier> = []
    private static var key = 0
    private final class Owner { weak var editor: T3ComposerEditor?; init(_ editor: T3ComposerEditor) { self.editor = editor } }

    static func attach(_ editor: T3ComposerEditor, to view: NSTextView) {
        install(on: view)
        objc_setAssociatedObject(view, &key, Owner(editor), .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }
    static func detach(_ view: NSTextView) { objc_setAssociatedObject(view, &key, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }

    private static func editor(of view: NSTextView) -> T3ComposerEditor? { (objc_getAssociatedObject(view, &key) as? Owner)?.editor }

    /// KVO's notifying subclass inherits the original class's wrapped methods.
    private static func baseClass(_ object: AnyObject) -> AnyClass? {
        var cls: AnyClass? = object_getClass(object)
        while let current = cls, NSStringFromClass(current).hasPrefix("NSKVONotifying_") { cls = class_getSuperclass(current) }
        return cls
    }

    private static func install(on view: NSTextView) {
        guard let cls = baseClass(view), !installed.contains(ObjectIdentifier(cls)) else { return }
        installed.insert(ObjectIdentifier(cls))
        for (selector, asText) in [(#selector(NSText.paste(_:)), false), (#selector(NSTextView.pasteAsPlainText(_:)), true)] {
            guard let method = class_getInstanceMethod(cls, selector) else { continue }
            typealias Paste = @convention(c) (AnyObject, Selector, AnyObject?) -> Void
            let own = unsafeBitCast(method_getImplementation(method), to: Paste.self)
            let block: @convention(block) (AnyObject, AnyObject?) -> Void = { object, sender in
                guard let view = object as? NSTextView, let editor = editor(of: view), editor.textView === view else { return own(object, selector, sender) }
                if view.isEditable, !view.hasMarkedText(), editor.pasteFiles(from: editor.pasteboard(), asText: asText) { return }
                let was = editor.pastingAsText
                editor.pastingAsText = was || asText
                own(object, selector, sender)
                editor.pastingAsText = was
            }
            replace(cls, selector, method, imp_implementationWithBlock(block))
        }
        let readable = #selector(getter: NSTextView.readablePasteboardTypes)
        guard let types = class_getInstanceMethod(cls, readable) else { return }
        typealias Readable = @convention(c) (AnyObject, Selector) -> NSArray
        let ownTypes = unsafeBitCast(method_getImplementation(types), to: Readable.self)
        let readableBlock: @convention(block) (AnyObject) -> NSArray = { object in
            let listed = ownTypes(object, readable)
            guard let view = object as? NSTextView, let editor = editor(of: view), editor.textView === view, editor.takesPastedFiles,
                  let list = listed as? [String] else { return listed }
            return (list + [NSPasteboard.PasteboardType.png.rawValue, NSPasteboard.PasteboardType.tiff.rawValue].filter { !list.contains($0) }) as NSArray
        }
        replace(cls, readable, types, imp_implementationWithBlock(readableBlock))
    }

    /// `implementation` as `selector` on `cls`: in place of the class's own method, else added over the inherited one.
    private static func replace(_ cls: AnyClass, _ selector: Selector, _ method: Method, _ implementation: IMP) {
        var count: UInt32 = 0
        let methods = class_copyMethodList(cls, &count)
        defer { free(methods) }
        if let methods, (0..<Int(count)).contains(where: { method_getName(methods[$0]) == selector }) { method_setImplementation(method, implementation) }
        else { class_addMethod(cls, selector, implementation, method_getTypeEncoding(method)) }
    }
}
#endif
