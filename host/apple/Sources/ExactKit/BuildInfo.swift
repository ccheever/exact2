// What a development menu shows about the build it is in (iOS and macOS):
// the app, its version, when it was built, the commits it came from and its
// kind, from the keys `host/apple/buildinfo.mjs` stamps into Info.plist.
import Foundation

enum BuildInfo {
    /// The menu's first lines, from a bundle's info dictionary, `now` for the
    /// build's age. A key the build did not stamp (an older build, a test
    /// host) is left out, never guessed.
    static func lines(_ info: [String: Any], now: Date = Date()) -> [String] {
        let name = info["CFBundleDisplayName"] as? String ?? info["CFBundleName"] as? String ?? "?"
        var lines = ["\(name) — \(info["CFBundleIdentifier"] as? String ?? "?")",
                     "version \(info["CFBundleShortVersionString"] as? String ?? "?") (\(info["CFBundleVersion"] as? String ?? "?"))"]
        if let stamp = info["ExactBuildTime"] as? String, let built = ISO8601DateFormatter.fractional.date(from: stamp) ?? ISO8601DateFormatter().date(from: stamp) {
            local.timeZone = .current // the device may have moved since the last opening
            lines.append("built \(local.string(from: built)) (\(age(of: built, now: now)))")
        }
        if let sha = info["ExactCommit"] as? String { lines.append("exact2 \(commit(sha, dirty: info["ExactCommitDirty"]))") }
        if let sha = info["ExactAppCommit"] as? String { lines.append("app \(commit(sha, dirty: info["ExactAppCommitDirty"]))") }
        if let kind = info["ExactBuildKind"] as? String { lines.append(kind) }
        return lines
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
