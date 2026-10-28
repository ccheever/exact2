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

    /// `text` about source `source` with no link's query (`withoutQueries`:
    /// the source, however it is quoted, is one of them).
    static func redacted(_ text: String, source: String) -> String { withoutQueries(text) }
    /// `text` with no query or fragment of any link in it: a CDN's links carry
    /// signatures (Discord's `ex`, `is`, `hm`), and an image's `error` quotes
    /// only the host and path. A `?` with anything but a space after it starts
    /// a query wherever the link sits; a `#` does too in a word with a `/`, a
    /// `:` or a `url(`. The rest of the word goes, all of it, and only the run
    /// of closing marks and punctuation that ended the word comes back. A
    /// question (`why?`) is left be. One pass (the Linux host's
    /// `remote::without_queries`).
    static func withoutQueries(_ text: String) -> String {
        // By Unicode scalar (a combining mark after `?` is its own), and words
        // end at ASCII whitespace only (a no-break space can sit in a link).
        let trailing: (Unicode.Scalar) -> Bool = { ")\"'`>],;:.".unicodeScalars.contains($0) }
        let space: (Unicode.Scalar) -> Bool = { " \t\n\r\u{0B}\u{0C}".unicodeScalars.contains($0) }
        let scalars = Array(text.unicodeScalars)
        var out = String.UnicodeScalarView(), i = 0, linked = false
        var last: [Unicode.Scalar] = ["\0", "\0", "\0"]
        while i < scalars.count {
            let c = scalars[i]
            let continues = i + 1 < scalars.count && !space(scalars[i + 1])
            if continues && (c == "?" || (c == "#" && linked)) {
                var end = i
                while end < scalars.count, !space(scalars[end]) { end += 1 }
                var kept = end
                while kept > i + 1, trailing(scalars[kept - 1]) { kept -= 1 }
                out.append(contentsOf: scalars[kept..<end])
                i = end
                continue
            }
            if space(c) { linked = false; last = ["\0", "\0", "\0"] } else {
                linked = linked || c == "/" || c == ":" || (c == "(" && last == ["u", "r", "l"])
                last = [last[1], last[2], c]
            }
            out.append(c); i += 1
        }
        return String(out)
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
