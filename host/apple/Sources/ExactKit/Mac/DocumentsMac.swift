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

    /// What the window is called once something is open: the app's name and
    /// the folder the opened thing lives in — "LLP — exact2" for a corpus at
    /// `~/projects/exact2/llp`, "Markdown — exact2" for a file in it. The
    /// folder rather than the file because a reader shows the filename in
    /// its own chrome, and because it is the project you have open that you
    /// pick out of a window list. Empty when there is nothing above it.
    public static func windowTitle(for path: String) -> String {
        // An empty path is not the working directory: `URL(fileURLWithPath:)`
        // resolves one against the process's cwd, which would title the
        // window after wherever the app happened to be launched from.
        guard !path.isEmpty else { return ExactEnv.appName }
        let folder = URL(fileURLWithPath: path).standardizedFileURL.deletingLastPathComponent()
        let name = folder.lastPathComponent
        guard !name.isEmpty, name != "/" else { return ExactEnv.appName }
        return "\(ExactEnv.appName) — \(name)"
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
            }
        }
        return delivered
    }

    /// The node the system's appearance is delivered to.
    public static let appearanceTestId = "appearance"

    /// What the *system* is set to — `"dark"` or `"light"` — regardless of
    /// any appearance this app has set on itself. `NSApp.effectiveAppearance`
    /// is the wrong reading: an app that has chosen dark reports dark, so
    /// switching back to "follow the system" would follow the app. The user's
    /// preference is `AppleInterfaceStyle` in the global domain, absent when
    /// the system is light. @ref LLP 1033 D6
    public static var systemAppearance: String {
        UserDefaults.standard.string(forKey: "AppleInterfaceStyle")?.lowercased().contains("dark") == true ? "dark" : "light"
    }

    /// Tell the app what the system is set to, now and whenever it changes.
    /// The same seam a document arrives through: an app that declares no
    /// `appearance` node simply never hears, and draws in what it chose.
    public static func reportAppearance(to session: ExactSession) {
        _ = session.change(testId: appearanceTestId, value: systemAppearance)
    }

    /// Watch the system preference. The notification is the documented one
    /// for this preference and arrives on a distributed centre, so the
    /// delivery hops to the main thread the session lives on.
    public static func watchAppearance(_ session: ExactSession) -> NSObjectProtocol {
        DistributedNotificationCenter.default().addObserver(
            forName: Notification.Name("AppleInterfaceThemeChangedNotification"), object: nil, queue: .main
        ) { [weak session] _ in
            guard let session else { return }
            // The preference is written just before the notification; read it
            // fresh rather than trusting a cached domain.
            UserDefaults.standard.removeVolatileDomain(forName: UserDefaults.argumentDomain)
            reportAppearance(to: session)
        }
    }

    /// File ▸ Open… — the panel, offering exactly what the manifest
    /// declares. Cancelling delivers nothing, which is what keeps the
    /// document already open (LLP 1033's milestone).
    public static func open(into session: ExactSession) {
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
        deliver([url.standardizedFileURL.path], to: session)
    }
}
#endif
