// The embedded server's runtime on disk (20261005-embedded-server-runtime). The bundle carries
// the official release archive as it was published (Contents/Resources/t3-runtime: the pin, the
// archive and stage-runtime.mjs's manifest); the first launch unpacks it into the product's own
// layout, `<T3 home>/runtime/versions/<version>` with `.install-complete` holding the version
// (T3 Code MIT, see LICENSE-T3; reference 1e2ecbd975: apps/server/src/cloud/pinnedRuntime.ts
// `pinnedRuntimePaths`, scripts/install.sh's staging folder, `t3 --version` and rename).
// A runtime the `t3` installer already put there for the same version is used as it is.
//
// Before the archive is expanded its SHA-256 must equal the pin's; after, every path must match
// the manifest (type, mode, size, SHA-256, link), `t3` must be 0755 and `t3 --version` must print
// the pinned version. A lock keeps two launches from unpacking together; `.staging-*` folders an
// interrupted run left are removed under it. Nothing is written inside the app bundle.
import Foundation
import CryptoKit
import Darwin

struct T3LocalRuntimePin: Decodable, Equatable {
    let version: String
    let asset: String
    let size: Int
    let sha256: String
}

struct T3LocalRuntimeManifest: Decodable {
    struct Entry: Decodable {
        let path: String
        let type: String
        let mode: Int?
        let size: Int?
        let sha256: String?
        let link: String?
    }
    let version: String
    let asset: String
    let sha256: String
    let size: Int
    let files: Int
    let bytes: Int
    let entries: [Entry]
}

enum T3LocalRuntimeResult: Equatable {
    case ready(versionDir: URL, version: String, installed: Bool)
    case missing(String)
    case failed(String)
}

final class T3LocalRuntime {
    /// `pinnedRuntimePaths`.
    struct Paths: Equatable {
        let versionDir: URL
        let entry: URL
        let sentinel: URL
    }

    static func versionsDir(home: URL) -> URL { home.appendingPathComponent("runtime/versions", isDirectory: true) }
    static func paths(home: URL, version: String) -> Paths {
        let versionDir = versionsDir(home: home).appendingPathComponent(version, isDirectory: true)
        return Paths(versionDir: versionDir, entry: versionDir.appendingPathComponent("t3"), sentinel: versionDir.appendingPathComponent(".install-complete"))
    }

    let home: URL
    let bundleDir: URL?
    /// (phase, fraction): "verify" while the archive is hashed, "extract" while it is expanded and checked.
    var progress: (String, Double) -> Void = { _, _ in }
    var tarPath = "/usr/bin/tar"
    var runVersion: (URL) -> String? = T3LocalRuntime.versionOutput
    /// Bytes free on the volume that holds `<T3 home>/runtime/versions` (tests replace it).
    var freeSpace: (URL) -> Int64? = T3LocalRuntime.availableCapacity
    /// Development only (`T3_LOCAL_UNPACK_DELAY_MS`, 20261005-portable-app-download): holds the
    /// first-launch view at each stage (checking, unpacking at 0 %, at 50 %) so it can be looked at.
    var stageHold: TimeInterval = 0

    init(home: URL, bundleDir: URL?) {
        self.home = home; self.bundleDir = bundleDir
    }

    func pin() -> T3LocalRuntimePin? {
        guard let bundleDir, let data = try? Data(contentsOf: bundleDir.appendingPathComponent("runtime-pin.json")) else { return nil }
        return try? JSONDecoder().decode(T3LocalRuntimePin.self, from: data)
    }

    static func installed(_ paths: Paths, version: String) -> Bool {
        guard let sentinel = try? String(contentsOf: paths.sentinel, encoding: .utf8) else { return false }
        return sentinel.trimmingCharacters(in: .whitespacesAndNewlines) == version && FileManager.default.isExecutableFile(atPath: paths.entry.path)
    }

    func ensure() -> T3LocalRuntimeResult {
        guard let pin = pin() else { return .missing("The app bundle has no runtime-pin.json.") }
        let paths = Self.paths(home: home, version: pin.version)
        if Self.installed(paths, version: pin.version) { return .ready(versionDir: paths.versionDir, version: pin.version, installed: false) }
        guard let bundleDir else { return .missing("The app bundle has no server runtime.") }
        let archive = bundleDir.appendingPathComponent(pin.asset), manifestURL = bundleDir.appendingPathComponent("runtime-manifest.json")
        guard FileManager.default.fileExists(atPath: archive.path), let manifestData = try? Data(contentsOf: manifestURL) else {
            return .missing("This build has no server runtime: run `bun stage-runtime.mjs` (app.json commands.runtime) before the bundle build.")
        }
        guard let manifest = try? JSONDecoder().decode(T3LocalRuntimeManifest.self, from: manifestData),
              manifest.version == pin.version, manifest.sha256 == pin.sha256, manifest.asset == pin.asset else {
            return .failed("The bundled runtime manifest does not match the pin.")
        }
        let versions = Self.versionsDir(home: home)
        do { try FileManager.default.createDirectory(at: versions, withIntermediateDirectories: true) } catch {
            return .failed("Could not create \(versions.path): \(error.localizedDescription)")
        }
        let lockFd = open(versions.appendingPathComponent(".install.lock").path, O_CREAT | O_RDWR | O_CLOEXEC, 0o644)
        guard lockFd >= 0 else { return .failed("Could not open the install lock in \(versions.path).") }
        defer { flock(lockFd, LOCK_UN); close(lockFd) }
        flock(lockFd, LOCK_EX)
        // Another launch may have finished while this one waited for the lock.
        if Self.installed(paths, version: pin.version) { return .ready(versionDir: paths.versionDir, version: pin.version, installed: false) }
        for name in (try? FileManager.default.contentsOfDirectory(atPath: versions.path)) ?? [] where name.hasPrefix(".staging-") {
            try? FileManager.default.removeItem(at: versions.appendingPathComponent(name))
        }

        progress("verify", 0)
        hold()
        guard let size = (try? FileManager.default.attributesOfItem(atPath: archive.path)[.size]) as? Int, size == pin.size else {
            return .failed("The bundled \(pin.asset) is not \(pin.size) bytes.")
        }
        guard let digest = Self.sha256(archive, total: size, progress: { self.progress("verify", $0) }), digest == pin.sha256 else {
            return .failed("The bundled \(pin.asset) does not match its pinned SHA-256.")
        }
        progress("verify", 1)

        var template = Array(versions.appendingPathComponent(".staging-XXXXXX").path.utf8CString)
        guard let made = mkdtemp(&template) else { return .failed("Could not create a staging folder in \(versions.path).") }
        let staging = URL(fileURLWithPath: String(cString: made), isDirectory: true)
        let fail: (String) -> T3LocalRuntimeResult = { reason in
            try? FileManager.default.removeItem(at: staging)
            return .failed(reason)
        }
        if let error = Self.spaceShortfall(needed: manifest.bytes, free: freeSpace(versions), folder: versions) { return fail(error) }
        progress("extract", 0)
        hold()
        if let error = extract(archive, into: staging, entries: manifest.entries.count) {
            // Measured once the partial tree is gone: a full disk is said as such, not as tar's write error.
            try? FileManager.default.removeItem(at: staging)
            return .failed(Self.spaceShortfall(needed: manifest.bytes, free: freeSpace(versions), folder: versions) ?? error)
        }
        progress("extract", 0.5)
        hold()
        if let error = Self.check(staging, against: manifest, progress: { self.progress("extract", 0.5 + $0 / 2) }) { return fail(error) }
        let entry = staging.appendingPathComponent("t3")
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: entry.path),
              ((attributes[.posixPermissions] as? NSNumber)?.intValue ?? 0) & 0o777 == 0o755 else { return fail("The unpacked t3 is not mode 0755.") }
        guard runVersion(entry) == "t3 v\(pin.version)" else { return fail("The unpacked t3 does not run.") }
        do {
            try "\(pin.version)\n".write(to: staging.appendingPathComponent(".install-complete"), atomically: true, encoding: .utf8)
            if FileManager.default.fileExists(atPath: paths.versionDir.path) { try FileManager.default.removeItem(at: paths.versionDir) }
            try FileManager.default.moveItem(at: staging, to: paths.versionDir)
        } catch {
            return fail("Could not install the runtime: \(error.localizedDescription)")
        }
        progress("extract", 1)
        return .ready(versionDir: paths.versionDir, version: pin.version, installed: true)
    }

    /// `tar -xzvf <archive> --strip-components=1`; each listed name advances the fraction (to 0.5).
    private func extract(_ archive: URL, into staging: URL, entries: Int) -> String? {
        let process = Process(), listing = Pipe()
        process.executableURL = URL(fileURLWithPath: tarPath)
        process.arguments = ["-xzvf", archive.path, "-C", staging.path, "--strip-components=1"]
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        process.standardError = listing
        do { try process.run() } catch { return "Could not run tar: \(error.localizedDescription)" }
        var seen = 0, tail = Data(), lastReport = 0
        let handle = listing.fileHandleForReading
        while true {
            let chunk = handle.availableData
            if chunk.isEmpty { break }
            seen += chunk.reduce(0) { $0 + ($1 == 0x0A ? 1 : 0) }
            tail = Data((tail + chunk).suffix(2048))
            if seen - lastReport >= 64 { lastReport = seen; progress("extract", min(0.5, Double(seen) / Double(max(entries, 1)) / 2)) }
        }
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { return Self.tarFailure(status: process.terminationStatus, output: String(decoding: tail, as: UTF8.self)) }
        return nil
    }

    private func hold() { if stageHold > 0 { Thread.sleep(forTimeInterval: stageHold) } }

    /// tar's listing (`x <path>`) and its errors share stderr: the reason is the last `tar:` line.
    static func tarFailure(status: Int32, output: String) -> String {
        let line = output.split(separator: "\n").last { $0.hasPrefix("tar: ") }.map { String($0.dropFirst(5)) }
        return "The server files could not be unpacked (tar exited \(status))" + (line.map { ": \($0)" } ?? ".")
    }

    /// The unpacked tree needs `needed` bytes in `folder`; nil when that much is free (or unknown).
    static func spaceShortfall(needed: Int, free: Int64?, folder: URL) -> String? {
        guard let free, free < Int64(needed) else { return nil }
        let mb = { (bytes: Int64) in ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file) }
        return "There is not enough disk space to set up T3 Code: it needs \(mb(Int64(needed))) in \(folder.path), and \(mb(max(0, free))) is free."
    }

    static func availableCapacity(_ url: URL) -> Int64? {
        let values = try? url.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey, .volumeAvailableCapacityKey])
        if let important = values?.volumeAvailableCapacityForImportantUsage, important > 0 { return important }
        return values?.volumeAvailableCapacity.map(Int64.init)
    }

    static func sha256(_ url: URL, total: Int = 0, progress: (Double) -> Void = { _ in }) -> String? {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? handle.close() }
        var hasher = SHA256(), read = 0, lastReport = 0
        while let chunk = try? handle.read(upToCount: 1 << 20), !chunk.isEmpty {
            hasher.update(data: chunk)
            read += chunk.count
            if total > 0, read - lastReport >= 8 << 20 { lastReport = read; progress(Double(read) / Double(total)) }
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }

    /// Every manifest entry exists with its type, mode, size and hash or link, and nothing else is there.
    static func check(_ root: URL, against manifest: T3LocalRuntimeManifest, progress: (Double) -> Void = { _ in }) -> String? {
        var expected = Set<String>(), hashed = 0
        for entry in manifest.entries {
            expected.insert(entry.path)
            let path = root.appendingPathComponent(entry.path).path
            var info = stat()
            guard lstat(path, &info) == 0 else { return "\(entry.path) is missing after unpacking." }
            let kind = info.st_mode & S_IFMT, mode = Int(info.st_mode) & 0o777
            switch entry.type {
            case "link":
                guard kind == S_IFLNK, (try? FileManager.default.destinationOfSymbolicLink(atPath: path)) == entry.link else { return "\(entry.path) is not the link the manifest names." }
            case "dir":
                guard kind == S_IFDIR, mode == (entry.mode ?? 0) & 0o777 else { return "\(entry.path) is not a folder of mode \(String(entry.mode ?? 0, radix: 8))." }
            default:
                guard kind == S_IFREG, mode == (entry.mode ?? 0) & 0o777 else { return "\(entry.path) is not a file of mode \(String(entry.mode ?? 0, radix: 8))." }
                guard Int(info.st_size) == entry.size, sha256(URL(fileURLWithPath: path)) == entry.sha256 else { return "\(entry.path) does not match its manifest hash." }
                hashed += entry.size ?? 0
                progress(min(1, Double(hashed) / Double(max(manifest.bytes, 1))))
            }
        }
        guard let walker = FileManager.default.enumerator(atPath: root.path) else { return "Could not list the unpacked runtime." }
        for case let path as String in walker where !expected.contains(path) { return "\(path) is not in the manifest." }
        return nil
    }

    /// `t3 --version` with an empty environment (install.sh: "the downloaded executable does not run").
    static func versionOutput(_ entry: URL) -> String? {
        let process = Process(), output = Pipe()
        process.executableURL = entry
        process.arguments = ["--version"]
        process.environment = [:]
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = output
        process.standardError = FileHandle.nullDevice
        do { try process.run() } catch { return nil }
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        return process.terminationStatus == 0 ? String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines) : nil
    }
}
