import Foundation

/// Where remote and `data:` image bytes are spooled (`RasterInput`): one
/// directory per process under `tmp/exact-raster/`, holding an exclusive
/// `flock` on its `.lock` for the process's life. A file is removed when its
/// input is released, but a process that is killed never releases them, so
/// the first spool of a launch removes every directory whose lock no live
/// process holds. On macOS the temporary directory is the user's, shared by
/// every app, which is why a live owner is told by its lock, not by age alone.
enum RasterSpool {
    /// A directory younger than this is never swept: its owner may be between
    /// creating it and taking its lock.
    static let grace: TimeInterval = 60

    static let directory: URL = {
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory()), root = tmp.appendingPathComponent("exact-raster", isDirectory: true)
        sweep(root, now: Date())
        #if os(iOS) || os(tvOS)
        // Before this directory, spools were loose `tmp/exact-raster-<uuid>`
        // files; an app's own sandboxed temporary directory holds no one else's.
        sweepLoose(tmp, now: Date())
        #endif
        let mine = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try? FileManager.default.createDirectory(at: mine, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        // Held, never closed: the lock is released when the process ends.
        let fd = open(mine.appendingPathComponent(".lock").path, O_CREAT | O_RDWR, 0o600)
        if fd >= 0 { _ = flock(fd, LOCK_EX | LOCK_NB) }
        return mine
    }()

    /// A fresh spool file's URL in this process's directory.
    static func file() -> URL { directory.appendingPathComponent(UUID().uuidString) }

    /// Remove loose `exact-raster-<uuid>` files past the grace period.
    static func sweepLoose(_ tmp: URL, now: Date) {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: tmp, includingPropertiesForKeys: [.contentModificationDateKey, .isRegularFileKey]) else { return }
        for file in entries where file.lastPathComponent.hasPrefix("exact-raster-") {
            guard let values = try? file.resourceValues(forKeys: [.contentModificationDateKey, .isRegularFileKey]), values.isRegularFile == true,
                  let modified = values.contentModificationDate, now.timeIntervalSince(modified) > grace else { continue }
            try? fm.removeItem(at: file)
        }
    }

    /// Remove the directories under `root` that no live process owns: past the
    /// grace period, with no lock file or a lock this process can take.
    static func sweep(_ root: URL, now: Date) {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: root, includingPropertiesForKeys: [.contentModificationDateKey, .isDirectoryKey]) else { return }
        for dir in entries {
            guard let values = try? dir.resourceValues(forKeys: [.contentModificationDateKey, .isDirectoryKey]), values.isDirectory == true,
                  let modified = values.contentModificationDate, now.timeIntervalSince(modified) > grace else { continue }
            let fd = open(dir.appendingPathComponent(".lock").path, O_RDWR)
            if fd < 0 { try? fm.removeItem(at: dir); continue }
            if flock(fd, LOCK_EX | LOCK_NB) == 0 { try? fm.removeItem(at: dir) }
            close(fd)
        }
    }
}
