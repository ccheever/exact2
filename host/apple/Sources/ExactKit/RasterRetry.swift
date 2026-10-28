import Foundation

/// When a picture on screen whose download failed in a way asking again may
/// mend is asked for again, and what counts as one (LLP 1011 §4): the same
/// rules as the Linux, Android and Windows host's (`host/linux/src/image.rs`,
/// `image/remote.rs`).
enum RasterRetry {
    /// After each failure: six times a showing, about a minute in all, then
    /// its `error`; from nothing whenever it leaves the screen and shows again.
    static var steps: [TimeInterval] { base.map { $0 * scale } }
    private static let base: [TimeInterval] = [1, 2, 4, 8, 16, 30]
    /// The longest a picture on screen waits to ask again: a server that asks
    /// for longer ends that showing's retries.
    static var most: TimeInterval { 300 * scale }
    /// The whole load's bound, from when it is asked for: a picture that fills
    /// the screen (a viewer's full-size original, megabytes on a phone's
    /// network: 32 s alone at 3 Mbit/s for 12 MB) has the longer one. Each
    /// response and read keeps the session's 15 s.
    static var whole: TimeInterval { 30 * wholeScale }
    static var urgentWhole: TimeInterval { 120 * wholeScale }
    #if DEBUG
    /// Tests run the ladder, the cap and the bounds in milliseconds.
    nonisolated(unsafe) static var scale: TimeInterval = 1
    nonisolated(unsafe) static var wholeScale: TimeInterval = 1
    #else
    static let scale: TimeInterval = 1
    static let wholeScale: TimeInterval = 1
    #endif

    /// Whether asking again may mend `error`: the network or the server (no
    /// connection, a lookup, a lost or refused connection, a timeout, a TLS
    /// handshake, the spool), a 5xx, 408 or 429, or an empty body; not a
    /// cancel, another 4xx, a bad URL, too many redirects or one to nowhere,
    /// or a body over the cap. The Linux host's classes (`image/remote.rs`).
    static func transient(_ error: Error) -> Bool {
        if let status = error as? RasterHTTPStatus { return status.code >= 500 || status.code == 408 || status.code == 429 }
        if error is RasterFailure { return false }
        guard let error = error as? URLError else { return true }
        switch error.code {
        case .cancelled, .badURL, .unsupportedURL, .fileDoesNotExist, .dataLengthExceedsMaximum,
             .httpTooManyRedirects, .redirectToNonExistentLocation,
             .userAuthenticationRequired, .userCancelledAuthentication, .appTransportSecurityRequiresSecureConnection:
            return false
        default:
            return true
        }
    }

    /// How long a `Retry-After` asks to wait at `now`: delta-seconds or an
    /// HTTP-date (RFC 9110 §10.2.3), held to ten years; nil when it cannot be read.
    static func retryAfter(_ value: String, now: Date) -> TimeInterval? {
        let value = value.trimmingCharacters(in: .whitespaces)
        let wait: TimeInterval
        if !value.isEmpty, value.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }) {
            guard let seconds = UInt64(value) else { return nil }
            wait = TimeInterval(min(seconds, tenYears))
        } else {
            guard let date = httpDate.date(from: value) else { return nil }
            wait = max(0, date.timeIntervalSince(now))
        }
        return min(wait, TimeInterval(tenYears))
    }
    /// An unreadable `Retry-After` waits an hour: past what a picture waits,
    /// so not in this showing.
    static let unreadable: TimeInterval = 3600
    private static let tenYears: UInt64 = 10 * 365 * 86_400
    private static let httpDate = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(identifier: "GMT")
        formatter.dateFormat = "EEE, dd MMM yyyy HH:mm:ss 'GMT'"
        return formatter
    }()

    /// `text` with every link in it (a word with `://`) cut at its query or
    /// fragment: a CDN's links carry signatures (Discord's `ex`, `is`, `hm`),
    /// and an image's `error` quotes only the host and path (the Linux host's
    /// `remote::without_queries`).
    static func withoutQueries(_ text: String) -> String {
        var out = "", word = ""
        func flush() {
            if word.contains("://"), let cut = word.firstIndex(where: { $0 == "?" || $0 == "#" }) { word = String(word[..<cut]) }
            out += word; word = ""
        }
        for character in text {
            if character.isWhitespace { flush(); out.append(character) } else { word.append(character) }
        }
        flush()
        return out
    }

    /// A box on screen at least 90% of its window's width and half its height
    /// (a viewer's picture): its download goes first, with the longer bound.
    static func fills(_ box: CGRect, window: CGRect) -> Bool {
        window.width > 0 && box.width >= window.width * 0.9 && box.height >= window.height * 0.5
    }
}

/// A 2xx response with no body: asking again may mend it.
struct RasterEmptyResponse: Error, CustomStringConvertible {
    var description: String { "an empty response" }
}
