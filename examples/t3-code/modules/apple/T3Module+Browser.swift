// T3Module's Browser ops (T3Module.swift routes them; browser-surface part 1): the data module's live sessions,
// a tab's navigation and its chrome row's commands, answered by the page registry (T3BrowserSessions.swift).
import Foundation

extension T3Module: T3BrowserSessionOwner {
    func browserOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        guard let answer = browserSessions.perform(request) else { return next() }
        reply.send(answer)
    }
}
