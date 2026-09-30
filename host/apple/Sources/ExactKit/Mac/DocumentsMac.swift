// The documents an app opens, and the one seam they reach it through.
//
// Four routes arrive here and are deliberately the same afterwards: the
// paths after the executable on the command line (`exact run markdown
// README.md`, or `mdview` through the shim), every `file:` URL Launch
// Services delivers (a Finder double-click, an Open With, a second `mdview`
// while this one runs), File ▸ Open… (⌘O), and a link followed inside a
// document that turns out to name a file.
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

    /// Every `LSItemContentTypes` entry across the declared document types.
    static var types: [UTType] {
        let declarations = ExactEnv.appMetadata["CFBundleDocumentTypes"] as? [[String: Any]] ?? []
        return declarations
            .flatMap { $0["LSItemContentTypes"] as? [String] ?? [] }
            .compactMap { UTType($0) }
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
        NSDocumentController.shared.noteNewRecentDocumentURL(URL(fileURLWithPath: path))
    }

    /// What Open Recent lists, newest first.
    public static var recent: [String] {
        NSDocumentController.shared.recentDocumentURLs.map(\.path)
    }

    /// Hand each path to the app, in order. Returns whether every one
    /// landed: `change` refuses a path when the app has no `open-file` node
    /// mounted, which is the app saying it opens nothing.
    @discardableResult
    public static func deliver(_ paths: [String], to session: ExactSession) -> Bool {
        var delivered = true
        for path in paths where !path.isEmpty {
            if !session.change(testId: testId, value: path) {
                FileHandle.standardError.write(Data("exact: \(ExactEnv.appName) has no `\(testId)` field to open \(path) with\n".utf8))
                delivered = false
            } else if FileManager.default.fileExists(atPath: path) {
                noteRecent(path)
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
