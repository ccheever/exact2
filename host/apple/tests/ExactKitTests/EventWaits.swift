import XCTest

/// The tests' one wait on an event (SmoothCollectionIOSTests' frame-bounded
/// waits, then the Mac's scroll-end and scroll-rest waits): it runs the main
/// loop in 5 ms turns until `done`, and fails only at `cap` or once `giveUp`
/// says enough has happened without it (a frame budget), never on a short
/// wall deadline alone. A loaded machine delays a main-queue work item or a
/// display link's frames; a short deadline failed tests that got every
/// event right, late. A failure says how long it waited, how many turns ran
/// and the slowest, and `facts` (what the wait itself saw); a pass that took
/// longer than `late` is kept as a LATE-PASS attachment and log line saying the same, so
/// lateness is evidence without failing. It answers how long it took.
extension XCTestCase {
    @discardableResult
    func waitFor(_ what: String, late: TimeInterval = 2, cap: TimeInterval = 30, giveUp: () -> Bool = { false },
                 facts: () -> String = { "" }, _ done: () -> Bool) -> TimeInterval {
        let began = Date(), until = began.addingTimeInterval(cap)
        var turns = 0, slowest = 0.0
        while !done(), Date() < until, !giveUp() {
            let turn = Date()
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.005))
            slowest = max(slowest, Date().timeIntervalSince(turn))
            turns += 1
        }
        let took = Date().timeIntervalSince(began)
        let seen = facts()
        let said = "took \(String(format: "%.3f", took)) s over \(turns) turns, the slowest \(String(format: "%.3f", slowest)) s" + (seen.isEmpty ? "" : ", \(seen)")
        guard done() else {
            XCTFail("\(what): \(said)")
            return took
        }
        if took > late { latePass(what, said) }
        return took
    }

    /// A LATE-PASS record: an attachment a result bundle keeps (the iOS
    /// runs') and the same text as a line in the run's own log, which the
    /// Mac runner's swift xctest keeps where it keeps no attachment, so one
    /// grep finds it on both hosts.
    func latePass(_ what: String, _ said: String) {
        let line = "LATE-PASS \(name) \(what): \(said)"
        print(line)
        let note = XCTAttachment(string: line)
        note.name = "LATE-PASS \(what)"
        note.lifetime = .keepAlways
        add(note)
    }
}
