// T3Module's Browser ops (T3Module.swift routes them; browser-surface part 1): the data module's live sessions,
// a tab's navigation and its chrome row's commands, answered by the page registry (T3BrowserSessions.swift).
import Foundation

extension T3Module: T3BrowserSessionOwner {
    func browserOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if let answer = browserSessions.perform(request) { return reply.send(answer) }
        if browserSessions.performCapture(request, reply: { reply.send($0) }) { return } // part 3 (T3BrowserCapture.swift)
        next()
    }
}
