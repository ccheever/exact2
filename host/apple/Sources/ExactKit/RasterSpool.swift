import Foundation

/// Where remote and `data:` image bytes are spooled (`RasterInput`): one
/// directory per process under `tmp/exact-raster/`, holding an exclusive
/// `flock` on its `.lock` for the process's life. A file is removed when its
/// input is released, but a process that is killed never releases them, so
/// the first spool of a launch removes every directory whose lock no live
/// process holds. On macOS the temporary directory is the user's, shared by
/// every app, which is why a live owner is told by its lock, not by age.
/// Creating a directory and sweeping both hold `.namespace.lock`, so a sweep
/// never sees a directory between its creation and its lock.
enum RasterSpool {
    private static let guarded = NSLock()
    private static var established: URL?

    /// This process's spool directory, made (and the root swept) on first
    /// use. A failure throws and the next spool tries again.
    static func directory() throws -> URL {
        guarded.lock(); defer { guarded.unlock() }
        if let established { return established }
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory())
        #if os(iOS) || os(tvOS)
        // Before this directory, spools were loose `tmp/exact-raster-<uuid>`
        // files; an app's own sandboxed temporary directory holds no one else's.
        sweepLoose(tmp, now: Date())
        #endif
        let made = try establish(in: tmp.appendingPathComponent("exact-raster", isDirectory: true))
        established = made
        return made
    }

    /// A fresh spool file's URL in this process's directory.
    static func file() throws -> URL { try directory().appendingPathComponent(UUID().uuidString) }

    /// Sweep `root`, then make a directory in it whose lock this process
    /// holds from now on (the descriptor is never closed; it closes at exit
    /// and is not inherited across exec).
    static func establish(in root: URL) throws -> URL {
        let fm = FileManager.default
        // A symlink in the root's place (a shared macOS tmp) is never
        // followed: it goes, and a real directory takes its place.
        if (try? root.resourceValues(forKeys: [.isSymbolicLinkKey]))?.isSymbolicLink == true { try fm.removeItem(at: root) }
        try fm.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let namespace = try lockFile(root.appendingPathComponent(".namespace.lock"), create: true, wait: true)
        defer { close(namespace) }
        sweep(root)
        let mine = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try fm.createDirectory(at: mine, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        do {
            _ = try lockFile(mine.appendingPathComponent(".lock"), create: true, wait: false)
        } catch {
            try? fm.removeItem(at: mine)
            throw error
        }
        return mine
    }

    /// Remove the directories under `root` that no live process owns: one
    /// whose lock file is gone, or whose lock this process can take. The
    /// caller holds the namespace lock. Any other failure (a lock that cannot
    /// be opened says nothing of its owner) keeps the directory.
    static func sweep(_ root: URL) {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: root, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey]) else { return }
        for dir in entries {
            // A real directory only: a symlink is left alone, never followed.
            guard let values = try? dir.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey]),
                  values.isDirectory == true, values.isSymbolicLink != true else { continue }
            let fd = open(dir.appendingPathComponent(".lock").path, O_RDWR | O_CLOEXEC | O_NOFOLLOW)
            if fd < 0 {
                if errno == ENOENT { try? fm.removeItem(at: dir) }
                continue
            }
            if flock(fd, LOCK_EX | LOCK_NB) == 0 { try? fm.removeItem(at: dir) }
            close(fd)
        }
    }

    /// Remove loose `exact-raster-<uuid>` files older than a minute.
    static func sweepLoose(_ tmp: URL, now: Date) {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: tmp, includingPropertiesForKeys: [.contentModificationDateKey, .isRegularFileKey]) else { return }
        for file in entries where file.lastPathComponent.hasPrefix("exact-raster-") {
            guard let values = try? file.resourceValues(forKeys: [.contentModificationDateKey, .isRegularFileKey]), values.isRegularFile == true,
                  let modified = values.contentModificationDate, now.timeIntervalSince(modified) > 60 else { continue }
            try? fm.removeItem(at: file)
        }
    }

    /// An open, exclusively locked descriptor on `url`, or a thrown errno.
    private static func lockFile(_ url: URL, create: Bool, wait: Bool) throws -> Int32 {
        let fd = open(url.path, O_RDWR | O_CLOEXEC | O_NOFOLLOW | (create ? O_CREAT : 0), 0o600)
        guard fd >= 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        guard flock(fd, LOCK_EX | (wait ? 0 : LOCK_NB)) == 0 else {
            let code = errno
            close(fd)
            throw POSIXError(POSIXErrorCode(rawValue: code) ?? .EIO)
        }
        return fd
    }
}
