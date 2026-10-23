// Documents kept across launches on the Mac (@ref LLP 1069.010.000 D3–D6).
//
// For an app granted `doc.keep`, what the person chose — in a picker, from
// Finder or ⌘O or Open Recent, or by a drop — is kept as a security-scoped
// bookmark, the platform's documented way to keep a user's choice and what
// the App Sandbox would require (LLP 1069.010 Q2). The kept set itself is
// the library's (`exact_data::documents::kept`), in the app's own storage
// outside `app:/`; this side makes and resolves the bookmarks and reports
// `NSDocumentController`'s recents, the list that decides what stays kept:
// Clear Menu, or an entry falling off the end of File ▸ Open Recent, ends
// its reopening (D6).
#if os(macOS)
import AppKit
import CExact

public enum KeptDocuments {
    /// What resolved bookmarks gave, held under their security scope while
    /// the process runs: a kept document belongs to the app, not to one
    /// session (D4).
    nonisolated(unsafe) static var scoped: [URL] = []

    /// The sessions whose apps keep documents, for a change to the list.
    nonisolated(unsafe) static var keeping: [WeakKeeper] = []

    struct WeakKeeper { weak var session: ExactSession? }

    /// A security-scoped bookmark of `url`; nil when there is nothing there.
    public static func bookmark(_ url: URL) -> Data? {
        try? url.bookmarkData(options: .withSecurityScope, includingResourceValuesForKeys: nil, relativeTo: nil)
    }

    /// The object at `path`: its device and inode, as `stat` gives them and
    /// as the library records them.
    public static func identity(_ path: String) -> (device: UInt64, inode: UInt64)? {
        var s = stat()
        guard stat(path, &s) == 0 else { return nil }
        return (UInt64(bitPattern: Int64(s.st_dev)), s.st_ino)
    }

    /// Where a kept bookmark's object is now, under its security scope, and
    /// a fresh bookmark when the old one was stale or did not lead there;
    /// nil when the object is gone. A bookmark finds its path before its
    /// file: one whose path now holds something else (another program
    /// replaced the file, or moved it and put another in its place) is
    /// followed to the object itself, by its inode, as D4's table has it.
    public static func resolve(_ data: Data, device: UInt64, inode: UInt64) -> (url: URL, bookmark: Data?)? {
        var stale = false
        let found = try? URL(resolvingBookmarkData: data, options: [.withSecurityScope, .withoutUI, .withoutMounting],
                             relativeTo: nil, bookmarkDataIsStale: &stale)
        var url = found
        if url.flatMap({ identity($0.path) }).map({ $0 != (device, inode) }) ?? true {
            guard let moved = path(device: device, inode: inode, near: found) else { return nil }
            url = moved
            stale = true
        }
        guard let url else { return nil }
        if url.startAccessingSecurityScopedResource() { scoped.append(url) }
        return (url, stale ? bookmark(url) : nil)
    }

    /// The path of inode `inode` on device `device` (`fsgetpath(2)`), on the
    /// volume `near` is on, or the startup volume.
    static func path(device: UInt64, inode: UInt64, near: URL?) -> URL? {
        var volume = statfs()
        guard statfs(near?.deletingLastPathComponent().path ?? "/", &volume) == 0,
              UInt64(bitPattern: Int64(volume.f_fsid.val.0)) == device else { return nil }
        var fsid = volume.f_fsid
        var buffer = [CChar](repeating: 0, count: Int(MAXPATHLEN))
        guard fsgetpath(&buffer, buffer.count, &fsid, inode) > 0 else { return nil }
        return URL(fileURLWithPath: String(cString: buffer))
    }

    /// What File ▸ Open Recent holds, as paths, newest first: while a clear
    /// is on its way (`toNote`), what it will hold once noted again.
    static var recent: [String] { (toNote.map { $0.urls.reversed() } ?? NSDocumentController.shared.recentDocumentURLs).map(\.path) }

    /// File ▸ Open Recent ▸ Clear Menu: the list empties, and with it what
    /// every app here keeps.
    static func clear() {
        assert(Thread.isMainThread, "Open Recent's bookkeeping is the main thread's")
        toNote = nil; late = nil; generation += 1
        NSDocumentController.shared.clearRecentDocuments(nil)
        listChanged()
    }

    /// The list changed: each keeping session's app drops what it no longer
    /// holds.
    static func listChanged() {
        keeping.removeAll { $0.session == nil }
        for keeper in keeping { keeper.session?.keptRecentsChanged() }
    }

    /// The app forgot a document it kept (`forgetDocument`): it leaves Open
    /// Recent too. AppKit removes no single entry, so the list is noted again
    /// without it, oldest first (`noteWhenCleared`); one forgotten while that
    /// is on its way leaves what is waiting to be noted.
    static func forgotten(device: UInt64, inode: UInt64) {
        assert(Thread.isMainThread, "Open Recent's bookkeeping is the main thread's")
        let it = { (url: URL) in identity(url.path).map { $0 == (device, inode) } ?? false }
        if let waiting = toNote { toNote = (waiting.cleared, waiting.urls.filter { !it($0) }); return }
        let controller = NSDocumentController.shared
        let urls = controller.recentDocumentURLs
        guard urls.contains(where: it) else { return }
        controller.clearRecentDocuments(nil)
        late = nil; generation += 1
        toNote = (Set(urls.map(file)), urls.reversed().filter { !it($0) })
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { noteWhenCleared() }
    }

    /// What waits for AppKit's clear to land, oldest first: the list noted
    /// again, then each document the app noted meanwhile (`note`); and what
    /// the list held before (`cleared`). Nil when nothing waits. Like `late`
    /// and `generation`, the main thread's alone (the library's hook hops to
    /// it; menus and opens are on it), asserted where they change.
    nonisolated(unsafe) static var toNote: (cleared: Set<String>, urls: [URL])?
    /// After the 2 s fallback (`noteWhenCleared`): what it noted, and the app's
    /// notes since, watched while a clear that lands later still could drop
    /// them (`watchLate`); the `generation` it began in.
    nonisolated(unsafe) static var late: (cleared: Set<String>, urls: [URL], generation: Int)?
    /// Moved by Clear Menu and by each forget's own clear: a late watch begun
    /// before either stops, so a real Clear Menu is not undone.
    nonisolated(unsafe) static var generation = 0
    /// A URL as compared here: AppKit lists `/private/var/…` where the app
    /// noted `/var/…`.
    static func file(_ url: URL) -> String { url.resolvingSymlinksInPath().path }

    /// A document the app opened or saved, on Open Recent: held while a clear
    /// is on its way (it would go with it), then noted newest.
    static func note(_ url: URL) {
        assert(Thread.isMainThread, "Open Recent's bookkeeping is the main thread's")
        if let waiting = toNote { toNote = (waiting.cleared, waiting.urls.filter { file($0) != file(url) } + [url]); return }
        NSDocumentController.shared.noteNewRecentDocumentURL(url)
        if let watched = late { late = (watched.cleared, watched.urls.filter { file($0) != file(url) } + [url], watched.generation) }
    }

    /// AppKit empties the list a turn or more after `clearRecentDocuments`,
    /// and what is noted before then is dropped with it (measured on macOS
    /// 27: the oldest entry, or all of them, lost under load). So what waits
    /// is noted once none of what was cleared is listed, looked for every
    /// 50 ms from the turn after; after 2 s it is noted all the same, and
    /// watched 10 s more for a clear that lands later still (`watchLate`).
    static func noteWhenCleared(tries: Int = 40) {
        guard let waiting = toNote else { return }
        let controller = NSDocumentController.shared
        guard tries == 0 || !controller.recentDocumentURLs.contains(where: { waiting.cleared.contains(file($0)) }) else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { noteWhenCleared(tries: tries - 1) }
            return
        }
        toNote = nil
        waiting.urls.forEach(controller.noteNewRecentDocumentURL)
        guard tries == 0 else { return }
        late = (waiting.cleared, waiting.urls, generation)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { watchLate() }
    }

    /// A clear that landed after the fallback took what it noted: once none
    /// of what was cleared is listed, it is all noted again, oldest first (so
    /// the app's latest note stays newest). Every 250 ms for 10 s, and not
    /// past a Clear Menu or another forget's clear (`generation`).
    static func watchLate(tries: Int = 40) {
        guard let watched = late, watched.generation == generation else { return }
        let controller = NSDocumentController.shared
        guard tries > 0 else { late = nil; return }
        guard !controller.recentDocumentURLs.contains(where: { watched.cleared.contains(file($0)) }) else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { watchLate(tries: tries - 1) }
            return
        }
        late = nil
        watched.urls.forEach(controller.noteNewRecentDocumentURL)
    }
    /// The library's hook, set once.
    static let installed: Void = {
        exact_on_document_forgotten { device, inode in
            DispatchQueue.main.async { KeptDocuments.forgotten(device: device, inode: inode) }
        }
    }()
}

extension ExactSession {
    /// The session's data is about to activate: its app's kept documents are
    /// found where they are now, before any answer reads one (D4).
    func restoreKeptDocuments() {
        _ = KeptDocuments.installed
        let reply = documentCall(["op": "keptDocuments"])
        guard reply["keep"] as? Bool == true else { return }
        KeptDocuments.keeping.removeAll { $0.session == nil || $0.session === self }
        KeptDocuments.keeping.append(.init(session: self))
        let found: [[String: Any]] = (reply["restore"] as? [[String: Any]] ?? []).compactMap { record in
            guard let n = record["n"], let device = (record["device"] as? NSNumber)?.uint64Value,
                  let inode = (record["inode"] as? NSNumber)?.uint64Value,
                  let data = Data(base64Encoded: record["bookmark"] as? String ?? ""),
                  let (url, refreshed) = KeptDocuments.resolve(data, device: device, inode: inode) else { return nil }
            var found: [String: Any] = ["n": n, "path": url.path]
            if let refreshed { found["bookmark"] = refreshed.base64EncodedString() }
            return found
        }
        _ = documentCall(["op": "keptResolved", "found": found, "recent": KeptDocuments.recent])
    }

    /// What the person chose, kept for an app granted `doc.keep` (D3), once
    /// it is on Open Recent; `noting` puts it there when the app keeps it
    /// (a drop, which an app that keeps nothing takes as content, not as a
    /// document opened).
    @discardableResult
    func keepDocument(_ url: URL, noting: Bool = false) -> String? {
        guard let bookmark = KeptDocuments.bookmark(url) else { return nil }
        let reply = documentCall(["op": "keepDocument", "path": url.standardizedFileURL.path,
                                  "bookmark": bookmark.base64EncodedString(),
                                  "recent": KeptDocuments.recent + (noting ? [url.path] : [])])
        let doc = reply["doc"] as? String
        if doc != nil, url.startAccessingSecurityScopedResource() { KeptDocuments.scoped.append(url) }
        if noting, doc != nil {
            ExactDocuments.noteRecent(url.path)
            keptRecentsChanged()
        }
        return doc
    }

    /// A picker's choice that exists, on Open Recent and kept for an app that
    /// keeps documents: the path that names it for good (a document chosen
    /// again is the path it was kept by), under its security scope. Nil for
    /// one this app does not keep.
    func keptChoice(_ url: URL) -> String? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        ExactDocuments.noteRecent(url.path)
        return keepDocument(url)
    }

    /// Open Recent changed: its app keeps only what it still holds.
    func keptRecentsChanged() {
        _ = documentCall(["op": "keptRecents", "recent": KeptDocuments.recent])
    }
}
#endif
