import Foundation

/// Prepare every participant before deciding whether the generation can commit.
/// Native image loads must start together, including sessions after a waiter.
func prepareGeneration<Session, Candidate>(_ sessions: [Session], prepare: (Session) -> Candidate?) -> [(Session, Candidate)]? {
    var candidates: [(Session, Candidate)] = []
    var ready = true
    for session in sessions {
        if let candidate = prepare(session) { candidates.append((session, candidate)) }
        else { ready = false }
    }
    return ready ? candidates : nil
}
