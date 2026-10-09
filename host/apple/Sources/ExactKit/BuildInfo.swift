// What a development menu shows about the build it is in (iOS and macOS):
// the app, its version, when and where it was built and with which Xcode, the
// commits it came from, its kind, its distribution's revision and its release
// notes, from the keys `host/apple/buildinfo.mjs` stamps into Info.plist; and
// the device it runs on.
import Foundation

enum BuildInfo {
    /// The menu's first lines, from a bundle's info dictionary, `now` for the
    /// build's age. A key the build did not stamp (an older build, a test
    /// host) is left out, never guessed.
    static func lines(_ info: [String: Any], device: String? = nil, now: Date = Date()) -> [String] {
        let name = info["CFBundleDisplayName"] as? String ?? info["CFBundleName"] as? String ?? "?"
        var lines = ["\(name) — \(info["CFBundleIdentifier"] as? String ?? "?")",
                     "version \(info["CFBundleShortVersionString"] as? String ?? "?") (\(info["CFBundleVersion"] as? String ?? "?"))"]
        if let stamp = info["ExactBuildTime"] as? String, let built = ISO8601DateFormatter.fractional.date(from: stamp) ?? ISO8601DateFormatter().date(from: stamp) {
            local.timeZone = .current // the device may have moved since the last opening
            let host = (info["ExactBuildHost"] as? String).map { " on \($0)" } ?? ""
            lines.append("built \(local.string(from: built)) (\(age(of: built, now: now)))\(host)")
        }
        if let xcode = info["ExactBuildXcode"] as? String { lines.append(xcode) }
        if let sha = info["ExactCommit"] as? String { lines.append("exact2 \(commit(sha, dirty: info["ExactCommitDirty"]))") }
        if let sha = info["ExactAppCommit"] as? String {
            let branch = (info["ExactAppBranch"] as? String).map { " on \($0)" } ?? ""
            lines.append("app \(commit(sha, dirty: info["ExactAppCommitDirty"]))\(branch)")
        }
        if let kind = info["ExactBuildKind"] as? String { lines.append(kind) }
        if let revision = info["ExactDistributionRevision"] as? String { lines.append("revision \(revision)") }
        if let device { lines.append(device) }
        return lines
    }

    /// The app's release notes, as the build stamped them; `nil` without any.
    static func notes(_ info: [String: Any]) -> String? {
        (info["ExactReleaseNotes"] as? String).flatMap { $0.isEmpty ? nil : $0 }
    }

    /// The menu's whole text: the lines, then the release notes under a
    /// heading, as Copy takes it.
    static func text(_ lines: [String], notes: String?) -> String {
        lines.joined(separator: "\n") + (notes.map { "\n\nRelease notes\n\($0)" } ?? "")
    }

    /// The hardware model (`iPhone17,1`, `Mac16,10`) and the OS it runs.
    static func device() -> String {
        var size = 0
        let name = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"].map { "\($0) (simulator)" } ?? {
            #if os(macOS)
            let key = "hw.model"
            #else
            let key = "hw.machine"
            #endif
            sysctlbyname(key, nil, &size, nil, 0)
            var bytes = [CChar](repeating: 0, count: max(size, 1))
            sysctlbyname(key, &bytes, &size, nil, 0)
            return String(cString: bytes)
        }()
        return "\(name) · \(ProcessInfo.processInfo.operatingSystemVersionString)"
    }

    private static func commit(_ sha: String, dirty: Any?) -> String {
        String(sha.prefix(10)) + ((dirty as? Bool) == true ? " (dirty)" : "")
    }

    /// "just now", "5 min ago", "2 h ago", "3 d ago".
    static func age(of date: Date, now: Date) -> String {
        let s = max(0, now.timeIntervalSince(date))
        if s < 60 { return "just now" }
        if s < 3600 { return "\(Int(s / 60)) min ago" }
        if s < 86400 { return "\(Int(s / 3600)) h ago" }
        return "\(Int(s / 86400)) d ago"
    }

    /// The device's time zone, on the Gregorian calendar whatever the locale.
    static let local: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd HH:mm"
        return f
    }()
}

private extension ISO8601DateFormatter {
    static let fractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
}
