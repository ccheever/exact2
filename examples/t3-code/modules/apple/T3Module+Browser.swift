// T3Module's Browser ops (T3Module.swift routes them; browser-surface part 1): the data module's live sessions,
// a tab's navigation and its chrome row's commands, answered by the page registry (T3BrowserSessions.swift).
import Foundation

extension T3Module: T3BrowserSessionOwner {
    func browserOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if browserSessions.performProfiles(request, reply: reply) { return } // part 4: clearing, the cookie import (T3BrowserSessions+Profiles.swift)
        guard let answer = browserSessions.perform(request) else { return next() }
        reply.send(answer)
    }
}
