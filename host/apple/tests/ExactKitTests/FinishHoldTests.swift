import XCTest
@testable import ExactKit

/// LLP 1097 D10, LLP 1146 D2: a session holds a background task from the
/// batch that says it must finish work an answer started to the one that
/// says it is done: one task however often the word repeats, ended once,
/// by the system's expiration too, after which the app's return to the
/// foreground begins another while the work is still wanted, as it does
/// after the system refused one.
final class FinishHoldTests: XCTestCase {
    func testOneTaskFromTheWordToItsEnd() {
        var begun = 0, ended = 0
        var expire: (() -> Void)?
        let hold = FinishHold(begin: { expired in begun += 1; expire = expired; return true }, end: { ended += 1 })
        hold.moved(false)
        XCTAssertEqual(begun + ended, 0, "nothing to finish, nothing held")
        hold.moved(true)
        hold.moved(true)
        XCTAssertEqual(begun, 1, "one task")
        hold.moved(false)
        XCTAssertEqual(ended, 1)
        XCTAssertFalse(hold.held)
        hold.moved(true)
        expire?()
        XCTAssertEqual(ended, 2, "the system took it back")
        XCTAssertFalse(hold.held)
        hold.becameActive()
        XCTAssertEqual(begun, 3, "back in the foreground with the work still out, another")
        hold.becameActive()
        XCTAssertEqual(begun, 3, "one at a time")
        hold.moved(false)
        XCTAssertEqual(ended, 3)
        hold.becameActive()
        XCTAssertEqual(begun, 3, "nothing to finish, nothing begun")
        hold.moved(true)
        expire?()
        hold.moved(false)
        XCTAssertEqual(ended, 4, "ended once")
    }

    func testATaskTheSystemRefusesIsAskedForAgainInTheForeground() {
        var begun = 0, ended = 0, gives = false
        let hold = FinishHold(begin: { _ in begun += 1; return gives }, end: { ended += 1 })
        hold.moved(true)
        XCTAssertEqual(begun, 1)
        XCTAssertFalse(hold.held, "refused: nothing is held")
        hold.moved(true)
        XCTAssertEqual(begun, 2, "the next word asks again")
        gives = true
        hold.becameActive()
        XCTAssertEqual(begun, 3)
        XCTAssertTrue(hold.held, "back in the foreground, one is given")
        hold.moved(false)
        XCTAssertEqual(ended, 1, "only a task given is ended")
    }

    func testABatchsWordMovesTheSessionsHold() throws {
        let session = ExactApp.shared.makeSession(label: "finish-hold")
        defer { session.destroy() }
        let say = { (json: String) in Batch.decode(Data(json.utf8)) }
        XCTAssertEqual(say(#"{"ops":[],"mustFinish":true}"#).mustFinish, true)
        XCTAssertTrue(session.changesNothing(say(#"{"ops":[]}"#)))
        XCTAssertFalse(session.changesNothing(say(#"{"ops":[],"mustFinish":true}"#)), "the word is news")
        // What the word asks of the hold; whether the platform gives a task is the injected cases' to check
        session.apply(say(#"{"ops":[],"mustFinish":true}"#))
        XCTAssertTrue(session.finishHold.wanted)
        session.apply(say(#"{"ops":[]}"#))
        XCTAssertTrue(session.finishHold.wanted, "a batch that says nothing leaves it")
        session.apply(say(#"{"ops":[],"mustFinish":false}"#))
        XCTAssertFalse(session.finishHold.wanted)
        session.apply(say(#"{"ops":[],"mustFinish":true}"#))
        session.destroy()
        XCTAssertFalse(session.finishHold.wanted, "the session's end lets it go")
    }
}
