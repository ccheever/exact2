// @ref LLP 1116.007 D4: what the platform's stepper does to a number input's
// text, by HTML's `stepUp()` and `stepDown()`.
import XCTest
@testable import ExactKit

final class NumberSteppingTests: XCTestCase {
    private func stepped(_ text: String, up: Bool = true, _ props: [String: String] = [:]) -> String? {
        NumberStepping(props).stepped(text, up: up)
    }

    func testAWholeStepGivesAWholeNumberAndAFractionalOneIsExact() {
        XCTAssertEqual(stepped("3"), "4")
        XCTAssertEqual(stepped("3", up: false), "2")
        XCTAssertEqual(stepped("0.1", ["step": "0.1"]), "0.2")
        XCTAssertEqual(stepped("0.2", ["step": "0.1"]), "0.3", "decimal, not 0.30000000000000004")
        XCTAssertEqual(stepped("1.5", ["step": "0.5"]), "2")
        XCTAssertEqual(stepped("2.50", ["step": "0.25"]), "2.75")
        XCTAssertEqual(stepped("-1"), "0")
        XCTAssertEqual(stepped("0", up: false), "-1")
        XCTAssertEqual(stepped("1e2"), "101")
        XCTAssertEqual(stepped("10", ["step": "10"]), "20")
    }

    func testTheStepIsOneUnlessItIsAPositiveNumber() {
        for step in ["any", "ANY", "0", "-2", "abc", ""] { XCTAssertEqual(stepped("5", ["step": step]), "6", step) }
        XCTAssertEqual(stepped("6", ["step": " 2 "]), "8")
    }

    func testAValueOffTheStepGoesToTheNearestOneInTheDirectionFromTheBase() {
        XCTAssertEqual(stepped("2.5"), "3")
        XCTAssertEqual(stepped("2.5", up: false), "2")
        XCTAssertEqual(stepped("4", ["min": "1", "step": "3"]), "7", "1, 4, 7 from the base 1")
        XCTAssertEqual(stepped("5", ["min": "1", "step": "3"]), "7")
        XCTAssertEqual(stepped("5", up: false, ["min": "1", "step": "3"]), "4")
        XCTAssertEqual(stepped("-2.5", up: false), "-3")
        XCTAssertEqual(stepped("-2.5"), "-2")
    }

    func testMinAndMaxBoundSteppingAndAValueBeyondThemComesBackToThem() {
        let bounds = ["min": "1", "max": "99"]
        XCTAssertEqual(stepped("98", bounds), "99")
        XCTAssertNil(stepped("99", bounds), "at the top, up changes nothing")
        XCTAssertNil(stepped("1", up: false, bounds), "at the floor, down changes nothing")
        XCTAssertEqual(stepped("150", up: false, bounds), "99", "above max, down returns to it")
        XCTAssertNil(stepped("150", bounds), "above max, up does not lower it")
        XCTAssertEqual(stepped("-5", bounds), "1", "below min, up returns to it")
        XCTAssertNil(stepped("-5", up: false, bounds), "below min, down does not raise it")
        XCTAssertEqual(stepped("97", ["max": "98", "step": "2"]), "98", "the largest value on the step at or under max")
        XCTAssertNil(stepped("98", ["max": "99", "step": "2"]), "100 is over max, and 98 is where it is")
        XCTAssertEqual(stepped("1", ["min": "5", "max": "3"]), "5", "a max under min is min")
    }

    func testAnEmptyOrNonNumberFieldStepsFromZero() {
        XCTAssertEqual(stepped(""), "1")
        XCTAssertEqual(stepped("", up: false), "-1")
        XCTAssertEqual(stepped("abc"), "1")
        XCTAssertEqual(stepped("1.", ["step": "1"]), "2", "a point on its way to a fraction is the number before it")
        XCTAssertEqual(stepped("\u{2212}3"), "-2", "a typographic minus is a minus")
        XCTAssertEqual(stepped("", ["min": "1"]), "1")
        XCTAssertNil(stepped("", up: false, ["min": "1"]), "from 0, below the floor: nothing, as the web")
        XCTAssertEqual(stepped("", ["min": "5"]), "5")
        XCTAssertEqual(stepped(" 7 "), "8")
    }

    func testTheNumbersAreHTMLsGrammarNotSwiftsLenientOne() {
        for bad in ["5abc", "1,5", "--1", "1e", "e5", ".", "+", "nan", "inf", "0x10", "1 2"] {
            XCTAssertNil(NumberStepping.decimal(bad), bad)
        }
        for good in ["5", "-5", "+5", "5.", ".5", "5.5", "1e3", "1E-3", "-.5e+2"] {
            XCTAssertNotNil(NumberStepping.decimal(good), good)
        }
        XCTAssertEqual(NumberStepping.decimal("1e3"), 1000)
        XCTAssertEqual(NumberStepping.text(1000), "1000")
        XCTAssertEqual(NumberStepping.text(Decimal(string: "2.50")!), "2.5")
    }

    func testTheShownValueIsHeldToTheBounds() {
        let stepping = NumberStepping(["min": "1", "max": "99"])
        XCTAssertEqual(stepping.shown(""), 1)
        XCTAssertEqual(stepping.shown("50"), 50)
        XCTAssertEqual(stepping.shown("500"), 99)
        XCTAssertEqual(NumberStepping([:]).shown("-7"), -7)
    }

    /// LLP 1116.008 D5: a value typed past a bound commits as that bound, as
    /// the stepper holds its own value; a number within them, text that is no
    /// number and a field with no bounds keep what was typed (nil: no change).
    func testAValueTypedPastABoundCommitsAsTheBoundItPassed() {
        let bounds = NumberStepping(["min": "1", "max": "99"])
        XCTAssertEqual(bounds.clamped("250"), "99", "grocery-list: 250 against a max of 99")
        XCTAssertEqual(bounds.clamped("99.5"), "99")
        XCTAssertEqual(bounds.clamped("1e3"), "99")
        XCTAssertEqual(bounds.clamped("0"), "1")
        XCTAssertEqual(bounds.clamped("-7"), "1")
        XCTAssertEqual(bounds.clamped("\u{2212}7"), "1", "a typographic minus is a minus")
        XCTAssertEqual(bounds.clamped(" 250 "), "99")
        XCTAssertNil(bounds.clamped("1"))
        XCTAssertNil(bounds.clamped("42"))
        XCTAssertNil(bounds.clamped("99"))
        XCTAssertNil(bounds.clamped("1."), "a point on its way to a fraction is the number before it")
        for text in ["", "abc", "-", "1,5", "5abc", "."] { XCTAssertNil(bounds.clamped(text), "\"\(text)\" is no number: left alone") }
    }

    func testABoundTheAuthorDidNotWriteIsTheWebsNone() {
        XCTAssertEqual(NumberStepping(["min": "0"]).clamped("-3"), "0")
        XCTAssertNil(NumberStepping(["min": "0"]).clamped("5000"))
        XCTAssertEqual(NumberStepping(["max": "10"]).clamped("11"), "10")
        XCTAssertNil(NumberStepping(["max": "10"]).clamped("-5000"))
        XCTAssertNil(NumberStepping([:]).clamped("250"))
        XCTAssertNil(NumberStepping(["min": "x", "max": ""]).clamped("250"), "bounds that read as no number are none")
        XCTAssertEqual(NumberStepping(["min": "0.5", "max": "2.5"]).clamped("3"), "2.5", "a fractional bound keeps its fraction")
        XCTAssertEqual(NumberStepping(["min": "5", "max": "3"]).clamped("9"), "5", "a max under min is min")
    }
}
