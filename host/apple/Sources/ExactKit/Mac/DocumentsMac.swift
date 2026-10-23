// The documents an app opens, and the one seam they reach it through.
//
// Four routes arrive here and are deliberately the same afterwards: the
// paths after the executable on the command line (`exact run markdown
// README.md`, or `mdview` through the shim), every `file:` URL Launch
// Services delivers (a Finder double-click, an Open With, a second `mdview`
// while this one runs), File ▸ Open… (⌘O), and a link followed inside a
// document that turns out to name a file. The exception is an HTML file
// brought to a web browser, which is a link (`isWebPageFile`).
//
// A path reaches the app the way any embedder's value does (LLP 1031 D1):
// as the value of the one live node whose `testId` is `open-file`. The host
// never reads the file and never parses it; an app that declares no such
// node opens nothing, and says so. What a path *means* is the app's.
//
// What the app opens is `file_handlers` in its manifest — the W3C Web App
// Manifest's own key, which the Apple bake turns into `CFBundleDocumentTypes`
// (`host/apple/build.mjs`). One declaration: the Open panel offers what
// Finder offers, because both read the same plist.
//
// @ref LLP 1033, LLP 1030 D2 (the manifest is the one declaration)
#if os(macOS)
import AppKit
import UniformTypeIdentifiers

public enum ExactDocuments {
    /// The node an opened path is delivered to.
    public static let testId = "open-file"

    /// URLs ⌘O's panel returned, kept under their security scope.
    nonisolated(unsafe) static var scoped: [URL] = []

    /// Whether this app declares that it opens anything at all.
    public static var declared: Bool { !types.isEmpty }

    /// Every `LSItemContentTypes` entry across the declared document types,
    /// and the type this Mac gives each declared extension (`Picker`).
    static var types: [UTType] { Picker.documentTypes }

    /// The adapter that owns the windows hears of a document chosen in a
    /// session (a picker's open or save): the window shows it.
    nonisolated(unsafe) public static var shown: ((ExactSession, URL) -> Void)?

    /// Files the person chose in the app's own picker (studio diary R18):
    /// each joins Open Recent — a save's once it is written, since AppKit
    /// lists only files that exist — and is kept for an app granted
    /// `doc.keep` (LLP 1069.010.000 D3); the window shows the last.
    static func chosen(_ urls: [URL], in session: ExactSession) {
        // One that exists was noted and kept as it was chosen (`keptChoice`).
        for url in urls where !FileManager.default.fileExists(atPath: url.path) {
            noteWhenWritten(url) { [weak session] in session?.keepDocument(url) }
        }
        if let url = urls.last { shown?(session, url) }
    }

    /// AppKit drops a recent file that does not exist yet, which a save's
    /// is until the app writes it: it is noted, then `noted`, once it does,
    /// looked for a few times a second for two minutes.
    static func noteWhenWritten(_ url: URL, tries: Int = 480, noted: @escaping () -> Void = {}) {
        if FileManager.default.fileExists(atPath: url.path) { KeptDocuments.note(url); noted(); return }
        guard tries > 0 else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { noteWhenWritten(url, tries: tries - 1, noted: noted) }
    }

    /// Whether a file at `url` is of a type the app declares: its type
    /// conforms to one, or its extension is one a declaration names — what
    /// a dropped file must be (studio diary R19), as the pickers offer only
    /// these (LLP 1069.010 D2). An extension another app's type owns on this
    /// Mac (`.board`, Freeform's) still counts when the manifest names it.
    public static func accepts(_ url: URL, metadata: [String: Any] = ExactEnv.appMetadata) -> Bool {
        let ext = url.pathExtension.lowercased()
        if !ext.isEmpty, Picker.declarations(in: metadata).contains(where: { ($0["CFBundleTypeExtensions"] as? [String] ?? []).contains { $0.lowercased() == ext } }) { return true }
        let type = (try? url.resourceValues(forKeys: [.contentTypeKey]).contentType) ?? UTType(filenameExtension: ext)
        return type.map { t in Picker.types(in: metadata).contains { t.conforms(to: $0) } } ?? false
    }

    /// The paths in `arguments` that name something on disk, made absolute
    /// against the working directory. AppKit adds its own switches to a
    /// launched process's arguments (`-NSDocumentRevisionsDebugMode`, a
    /// `-psn_…`); naming a file that exists is what separates a document
    /// from those, and from a typo.
    public static func paths(in arguments: [String]) -> [String] {
        arguments.dropFirst().compactMap { argument in
            guard !argument.hasPrefix("-") else { return nil }
            let url = URL(fileURLWithPath: argument).standardizedFileURL
            return FileManager.default.fileExists(atPath: url.path) ? url.path : nil
        }
    }

    /// The `file:` URLs among `urls`.
    public static func paths(of urls: [URL]) -> [String] {
        urls.filter(\.isFileURL).map(\.standardizedFileURL.path)
    }

    /// Where a launch or a document lands (LLP 1069.010 D4): the manifest's
    /// W3C `launch_handler.client_mode`, baked as `ExactLaunchMode` whether
    /// or not the app opens documents — `navigate-new` gives each document,
    /// and File ▸ New Window, its own window and session,
    /// `navigate-existing` (the default) the window in front,
    /// `focus-existing` the window in front unless it already shows one.
    public static var launchMode: String {
        ExactEnv.appMetadata["ExactLaunchMode"] as? String ?? "navigate-existing"
    }

    /// Whether the app outlives its one window (LLP 1069.010 D8): the
    /// manifest's `host.macos.window.keepRunning`, baked into `ExactWindow`.
    /// A `navigate-new` app outlives its last window without it, and its
    /// windows close as documents do.
    public static func keepsRunning(_ metadata: [String: Any] = ExactEnv.appMetadata) -> Bool {
        (metadata["ExactWindow"] as? [String: Any])?["keepRunning"] as? Bool == true
            && metadata["ExactLaunchMode"] as? String != "navigate-new"
    }

    /// Whether the app ends once no window shows
    /// (`applicationShouldTerminateAfterLastWindowClosed`): a one-window app
    /// does, unless it keeps running and its window is only hidden (`hiding`:
    /// a window is open that its app has not closed); a `navigate-new` app
    /// stays for File ▸ New Window, Open and the Dock (LLP 1115 D8).
    public static func endsWithoutWindows(hiding: Bool, _ metadata: [String: Any] = ExactEnv.appMetadata) -> Bool {
        metadata["ExactLaunchMode"] as? String != "navigate-new" && !(hiding && keepsRunning(metadata))
    }

    /// The person closes `window` (⌘W, File ▸ Close Window, the red button):
    /// the only window of an app that keeps running is hidden rather than
    /// closed, with its session and view as they were, and nothing is asked
    /// (`beforeunload`), as nothing unloads. The page reads
    /// `visibilityState` `hidden` until the window shows again. Whether it
    /// was hidden, which the window's `windowShouldClose` answers `false` for.
    public static func hidesOnClose(_ window: NSWindow, only: Bool, keepsRunning: Bool = keepsRunning()) -> Bool {
        guard keepsRunning, only else { return false }
        PageFacts.hideForClose(window)
        return true
    }

    /// What the adapter tells a window it has just made (LLP 1115.000 D7.9).
    ///
    /// It opens with AppKit's own animation: `animationBehavior` is left as
    /// AppKit made it (it was `.none`). Under the agent, whose windows appear
    /// as its transitions do, at once, it is none.
    ///
    /// Window tabs are for an app whose documents have windows of their own
    /// (`navigate-new`): its windows share a tabbing identifier and File ▸
    /// New Window answers the tab bar's +. A one-window app offers none:
    /// AppKit adds Show Tab Bar and Merge All Windows to the menus of any app
    /// that allows automatic tabbing, and a + that would open nothing.
    public static func furnish(_ window: NSWindow, tabs: Bool = launchMode == "navigate-new") {
        if ExactEnv.agentMode { window.animationBehavior = .none }
        NSWindow.allowsAutomaticWindowTabbing = tabs
        window.tabbingMode = tabs ? .automatic : .disallowed
        if tabs { window.tabbingIdentifier = ExactEnv.appMetadata["CFBundleIdentifier"] as? String ?? ExactEnv.appName }
    }

    /// The host's router: every route in (Launch Services, the command
    /// line, ⌘O, Open Recent) hands its paths here, and the adapter that
    /// owns the windows decides which session each lands in. Unset, a path
    /// goes to the session the menu serves.
    nonisolated(unsafe) public static var route: (([String]) -> Void)?

    /// File ▸ Open Recent (LLP 1069.010 D5): AppKit's own list, kept by
    /// `NSDocumentController` with no `NSDocument` behind it. A file URL the
    /// user chose is exactly what a security-scoped bookmark is made from,
    /// so a sandboxed build can keep this list the same way (ruled: the Mac
    /// App Store stays possible; no sandbox work now).
    public static func noteRecent(_ path: String) {
        KeptDocuments.note(URL(fileURLWithPath: path))
    }

    /// What Open Recent lists, newest first.
    public static var recent: [String] {
        KeptDocuments.recent
    }

    /// Hand each path to the app, in order. Returns whether every one
    /// landed: `change` refuses a path when the app has no `open-file` node
    /// mounted, which is the app saying it opens nothing. A route's file is
    /// kept for an app granted `doc.keep`, the file alone (LLP 1069.010.000
    /// D3); a followed link (`keep: false`) is not.
    @discardableResult
    public static func deliver(_ paths: [String], to session: ExactSession, keep: Bool = true) -> Bool {
        var delivered = true
        for path in paths where !path.isEmpty {
            if !session.change(testId: testId, value: path) {
                let why = session.changeRefusal ?? "it delivered nothing"
                FileHandle.standardError.write(Data("exact: \(ExactEnv.appName) did not open \(path): \(why)\n".utf8))
                session.log("open-file: refused: \(why)")
                delivered = false
            } else if FileManager.default.fileExists(atPath: path) {
                noteRecent(path)
                if keep { session.keepDocument(URL(fileURLWithPath: path)) }
            }
        }
        return delivered
    }

    /// File ▸ Open… — the panel, offering exactly what the manifest
    /// declares. Cancelling delivers nothing, which is what keeps the
    /// document already open (LLP 1033's milestone).
    public static func open(into session: ExactSession?) {
        let declared = types
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = declared.contains(.folder)
        panel.canChooseFiles = declared.contains { $0 != .folder }
        // A folder-only app must not have the panel treat a package as a
        // file it can descend into and return.
        panel.treatsFilePackagesAsDirectories = false
        let files = declared.filter { $0 != .folder }
        if !files.isEmpty && panel.canChooseFiles {
            panel.allowedContentTypes = files
        }
        panel.prompt = "Open"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        // The URL the person chose, under its security scope while this
        // process runs: a no-op unsandboxed, what a sandboxed build needs
        // (LLP 1069.010 D1; ruled: the Mac App Store stays possible).
        if url.startAccessingSecurityScopedResource() { scoped.append(url) }
        let path = url.standardizedFileURL.path
        if let route { route([path]) } else if let session { deliver([path], to: session) }
    }
}
#endif
