// @ref LLP 1133.024 D22 item 7 — the embedder's decision on leaving the app:
// a package asks (`ExactPackageContext.leave`) before it shows a page over
// the app or hands a URL to another app, and the session's delegate answers.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// A session delegate that decides whether the app may leave for a URL. A
/// delegate that does not adopt it lets every one go, as the stock adapters
/// do; a refusal answers the package's send `NotAllowedError`.
public protocol ExactLeaveDelegate: AnyObject {
    /// `how` is `browser` (a page shown over the app) or `external` (another app).
    func exactSession(_ session: ExactSession, shouldLeaveFor url: String, how: String) -> Bool
}

extension ExactSession {
    /// The sessions that observe, once each, whatever their activations.
    nonisolated(unsafe) private static var leaving = Set<ObjectIdentifier>()

    /// Answers the packages of this session that ask to leave: the ask names
    /// the session by its view, the context's.
    func observeLeaving() {
        guard Self.leaving.insert(ObjectIdentifier(self)).inserted else { return }
        NotificationCenter.default.addObserver(forName: Notification.Name("exact.leave"), object: nil, queue: nil) { [weak self] note in
            guard let self, let ask = note.object as? NSMutableDictionary, let mine = view, ask["view"] as AnyObject? === mine,
                  let url = ask["url"] as? String, let how = ask["how"] as? String else { return }
            ask["allow"] = (delegate as? ExactLeaveDelegate)?.exactSession(self, shouldLeaveFor: url, how: how) ?? true
        }
    }
}
