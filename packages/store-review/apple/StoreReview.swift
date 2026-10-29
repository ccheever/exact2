// @exact/store-review on Apple (LLP 1133.015 D12): `AppStore.requestReview(in:)`
// over the session's window. StoreKit says nothing of whether it showed the
// prompt, so the request answers nothing and journals that it was made.
//
// Under the agent the implementation is its own substitute: it shows
// nothing, journals each request with its origin, and keeps Apple's quota
// on the agent clock (three a 365-day period), so a fourth inside it is
// journaled as over quota. Its record lives in the drive's scratch tree,
// so the quota spans relaunches.
import ExactPackage
import Foundation
import StoreKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class StoreReviewModule: ExactPackage {
    let context: ExactPackageContext
    static let year = 365 * 86_400_000.0

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "storeReviewStatus": reply(["available": true])
        case "requestReview":
            context.agent ? agentRequest() : request()
            reply([:])
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "storeReviewStatusSync" ? ["available": true] : ["error": "NotSupportedError"]
    }

    private func request() {
        guard let anchor = context.presentation else {
            return context.log("requestReview: not requested, no window of the session is in front")
        }
        #if os(macOS)
        // A session window has no content view controller of its own; StoreKit presents over a child of its view.
        guard let content = anchor.window.contentView else { return }
        let controller = NSViewController()
        controller.view = content
        MainActor.assumeIsolated { AppStore.requestReview(in: controller) }
        #else
        MainActor.assumeIsolated { AppStore.requestReview(in: anchor.scene) }
        #endif
        context.log("requestReview: requested (\(context.origin))")
    }

    /// The request journaled, against Apple's quota on the agent clock.
    private func agentRequest() {
        let file = context.file("app:/data/exact-agent-store-review.json")
        let epoch = Double(ProcessInfo.processInfo.environment["EXACT_AGENT_EPOCH"] ?? "") ?? 1_767_225_600_000
        let now = epoch + context.now()
        var asked = file.flatMap { try? Data(contentsOf: $0) }.flatMap { try? JSONDecoder().decode([Double].self, from: $0) } ?? []
        asked = asked.filter { $0 > now - Self.year }
        if asked.count >= 3 {
            context.log("requestReview: over quota (\(context.origin)), three within 365 days")
            return
        }
        asked.append(now)
        if let file, let data = try? JSONEncoder().encode(asked) {
            try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? data.write(to: file, options: .atomic)
        }
        context.log("requestReview: requested (\(context.origin)), \(asked.count) within 365 days")
    }
}
