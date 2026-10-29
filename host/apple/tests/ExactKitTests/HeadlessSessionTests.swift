import XCTest
import ExactCore
import CExact

final class HeadlessSessionTests: XCTestCase {
    func testFactsKeepEpochWhileOffsetFollowsTheVirtualInstantAcrossDST() throws {
        let epoch = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-03-08T09:30:00Z"))
        let zone = try XCTUnwrap(TimeZone(identifier: "America/Los_Angeles"))
        let before = SessionFacts(displayScale: 1, preferences: [], date: epoch, timeZone: zone)
        let after = SessionFacts(displayScale: 1, preferences: [], date: epoch.addingTimeInterval(3600), clock: 3_600_000, timeZone: zone)
        XCTAssertEqual(before.utcOffset, -480)
        XCTAssertEqual(after.utcOffset, -420)
        XCTAssertEqual(after.epochAtZero, before.epochAtZero)
    }
    func testPlanLayoutsWithoutAView() throws {
        let plan = try fixturePlan("HEADLESS")
        let session = try HeadlessSession(plan: plan, size: CGSize(width: 200, height: 400),
                                          facts: SessionFacts(displayScale: 2, preferences: []))
        defer { session.close() }
        func node(_ id: String, in snapshot: HeadlessSession.Snapshot) throws -> HeadlessSession.Node {
            try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == id })
        }

        let initial = try session.snapshot()
        let root = try node("root", in: initial), box = try node("box", in: initial)
        let label = try node("label", in: initial)
        XCTAssertEqual(initial.roots, [root.id])
        XCTAssertEqual(box.frame, CGRect(x: 10, y: 10, width: 40, height: 20))
        XCTAssertEqual(label.frame.origin, CGPoint(x: 10, y: 36))
        XCTAssertEqual(label.contentBox.width, 180)
        XCTAssertFalse(label.hasComputedLines)
        XCTAssertFalse(try node("padded", in: session.snapshot()).hasComputedLines)
        XCTAssertEqual(label.lines.map(\.text).joined(), "alpha beta gamma")
        XCTAssertEqual(label.lines.count, 1)
        XCTAssertTrue(try node("label", in: session.snapshot()).hasComputedLines)
        let padded = try node("padded", in: initial)
        XCTAssertEqual(padded.contentBox.origin, CGPoint(x: 8, y: 8))
        #if os(macOS)
        XCTAssertEqual(padded.lines.first?.origin.y, 16)
        #else
        XCTAssertEqual(padded.lines.first?.origin.y, label.lines.first?.origin.y)
        #endif
        XCTAssertEqual(try node("hairline", in: initial).frame.width, SessionFacts.defaultRootFontSize * 2)
        XCTAssertEqual(try node("hairline", in: initial).frame.height, 0.5)
        XCTAssertEqual(try node("body", in: initial).style["font_size"]?.number, Double(SessionFacts.defaultRootFontSize))

        let narrow = try session.resize(to: CGSize(width: 90, height: 180))
        let wrapped = try node("label", in: narrow)
        XCTAssertEqual(narrow.size.width, 90)
        XCTAssertEqual(wrapped.contentBox.width, 70)
        XCTAssertEqual(wrapped.lines.map(\.text).joined(), "alpha beta gamma")
        XCTAssertGreaterThan(wrapped.lines.count, label.lines.count)
        XCTAssertEqual(wrapped.frame.height, CGFloat(wrapped.lines.count) * 20)
        #if os(macOS)
        XCTAssertEqual(wrapped.lines.map(\.origin.y), [16, 36, 56])
        #else
        // UIKit's SF ascent differs from AppKit's; padding must add no offset.
        let baseline = try XCTUnwrap(label.lines.first).origin.y
        XCTAssertEqual(wrapped.lines.map(\.origin.y), [baseline, baseline + 20, baseline + 40])
        #endif

        let refreshed = try session.refresh()
        XCTAssertEqual(try node("label", in: refreshed).lines.map(\.text), wrapped.lines.map(\.text))
        let restored = try session.resize(to: initial.size)
        XCTAssertEqual(try node("label", in: restored).frame, label.frame)
        XCTAssertEqual(try node("label", in: restored).lines.map(\.text), label.lines.map(\.text))

        let changed = try session.press(box.id)
        let changedLabel = try node("label", in: changed)
        XCTAssertEqual(changedLabel.lines.map(\.text).joined(), "alpha delta epsilon zeta")
        XCTAssertEqual(label.lines.map(\.text).joined(), "alpha beta gamma", "an old snapshot keeps its lines")
        XCTAssertEqual(try node("box", in: changed).frame, box.frame)
    }
    func testClampedLinesAndSoftHyphensKeepShapedRanges() throws {
        let session = try HeadlessSession(plan: fixturePlan("HEADLESS"), size: CGSize(width: 200, height: 600),
                                          facts: SessionFacts(displayScale: 2, preferences: []))
        defer { session.close() }
        let snapshot = try session.snapshot()
        func node(_ name: String) throws -> HeadlessSession.Node {
            try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == name })
        }
        let clamped = try XCTUnwrap(node("clamped").lines.first)
        XCTAssertEqual(clamped.range, NSRange(location: 0, length: 6))
        XCTAssertEqual(clamped.text, "Hello…", "as the ellipsized line paints it: no space before the token")
        XCTAssertEqual(clamped.segments.map(\.text).joined(), clamped.text)
        let emoji = try XCTUnwrap(node("clamped-emoji").lines.first)
        XCTAssertEqual(emoji.range, NSRange(location: 0, length: "👨‍👩‍👧‍👦 ".utf16.count))
        XCTAssertEqual(emoji.text, "👨‍👩‍👧‍👦…")
        XCTAssertEqual(emoji.segments.map(\.text).joined(), emoji.text)
        // Where the token does not fit after the broken text, it displaces glyphs.
        let tight = try XCTUnwrap(node("clamped-tight").lines.first)
        XCTAssertTrue(tight.text.hasSuffix("…"))
        let kept = String(tight.text.dropLast())
        XCTAssertTrue("fox jumps over the lazy dog".hasPrefix(kept))
        XCTAssertFalse(kept.hasSuffix(" "))
        XCTAssertLessThanOrEqual(tight.width, 90.5)
        XCTAssertEqual(tight.segments.map(\.text).joined(), tight.text)
        // The kept text is what Core Text's truncation kept: the longest
        // prefix of the broken text that fits the line's width with the
        // token (a truncated run still reports the whole run's string range).
        let font = try XCTUnwrap(node("clamped-tight").runs.first?.font)
        func width(_ text: String) -> CGFloat {
            let attributed = NSAttributedString(string: text, attributes: [kCTFontAttributeName as NSAttributedString.Key: font])
            return CGFloat(CTLineGetTypographicBounds(CTLineCreateWithAttributedString(attributed), nil, nil, nil))
        }
        let broken = "fox jumps over the lazy dog"
        XCTAssertLessThanOrEqual(width(kept + "…"), tight.width + 0.5, "the kept prefix fits with the token")
        var longer = kept
        for ch in broken.dropFirst(kept.count) { longer.append(ch); if ch != " " { break } }
        XCTAssertGreaterThan(width(longer + "…"), tight.width + 0.5, "one more character would not: \(longer)")
        // A clamp through a ligature (`ffi`, `fi`): the kept glyph covers every
        // character it drew, so the text never ends between an f and its i.
        let ligature = try XCTUnwrap(node("clamped-ligature").lines.first)
        XCTAssertTrue(ligature.text.hasSuffix("…"))
        let keptLigature = String(ligature.text.dropLast())
        XCTAssertTrue("efficiency efficiency".hasPrefix(keptLigature), keptLigature)
        let rest = "efficiency efficiency".dropFirst(keptLigature.count)
        XCTAssertFalse(keptLigature.hasSuffix("f") && (rest.hasPrefix("i") || rest.hasPrefix("f")), "cut inside a ligature: \(keptLigature)|\(rest)")
        XCTAssertLessThanOrEqual(width(keptLigature + "…"), ligature.width + 0.5)
        // A trailing ligature before the token keeps every character when the
        // line already fits: "The staff…", not "The staf…".
        let ff = try XCTUnwrap(node("clamped-ff").lines.first)
        XCTAssertEqual(ff.text, "The staff…")
        let hyphen = try XCTUnwrap(node("hyphen").lines.first)
        XCTAssertEqual(hyphen.range, NSRange(location: 0, length: 3))
        XCTAssertEqual(hyphen.text, "ab-")
        XCTAssertEqual(hyphen.segments.map(\.text).joined(), "ab-")
        let styled = try node("styled")
        XCTAssertEqual(styled.runs.map(\.weight), [400, 700])
        XCTAssertEqual(styled.lines.flatMap(\.segments).map(\.run), [0, 1])
        XCTAssertNotNil(styled.runs.last?.color)
        XCTAssertGreaterThan(clamped.ascent, 0)
        XCTAssertGreaterThan(clamped.descent, 0)
    }

    func testRefusedFactsFailBoot() throws {
        let plan = try fixturePlan("HEADLESS")
        for facts in [SessionFacts(displayScale: 0, preferences: []),
                      SessionFacts(displayScale: .infinity, preferences: []),
                      SessionFacts(displayScale: 1, preferences: [], rootFontSize: .nan),
                      SessionFacts(displayScale: 1, preferences: [], rootFontSize: -1),
                      SessionFacts(displayScale: 1, preferences: [], locale: "bad_locale"),
                      SessionFacts(displayScale: 1, preferences: [], seed: 1 << 32)] {
            XCTAssertThrowsError(try HeadlessSession(plan: plan, size: CGSize(width: 200, height: 400), facts: facts))
        }
        let accepted = try HeadlessSession(plan: plan, size: CGSize(width: 200, height: 400),
            facts: SessionFacts(displayScale: 2, preferences: [], seed: UInt64(UInt32.max)))
        defer { accepted.close() }
        accepted.core.text.refusedMarkdown = 999
        XCTAssertNoThrow(try accepted.resize(to: CGSize(width: 200, height: 450)), "each turn clears the previous refusal")
        // An app's plan is no entry: the decoder refuses it by its header (LLP 1126 D2).
        XCTAssertThrowsError(try HeadlessSession(plan: plan, entry: "widget", size: CGSize(width: 200, height: 400))) { error in
            XCTAssertTrue(String(describing: error).contains("WrongEntry"), String(describing: error))
        }
    }

    func testCloseReleasesResidencyWithRetainedLazySnapshot() throws {
        let session = try HeadlessSession(plan: fixturePlan("HEADLESS"), size: CGSize(width: 200, height: 600),
            facts: SessionFacts(displayScale: 2, preferences: []))
        let snapshot = try session.snapshot()
        weak var engine = session.core.text
        let before = Owner.shared.sync { () -> TextResidencyStats in
            let engine = session.core.text
            for n in 0..<500 {
                let spec = TextStyle.spec(runs: [TextStyle.run(String(repeating: "memory ", count: 30) + String(n), style: [:])], style: [:], props: [:], strut: TextStyle.run("", style: [:]))
                _ = engine.paragraph(spec, width: 200)
            }
            return engine.residencyStats
        }
        XCTAssertEqual(before.softTargetBytes, 2 * 1024 * 1024)
        XCTAssertGreaterThan(before.coldEstimatedBytes, 0)
        XCTAssertLessThanOrEqual(before.coldEstimatedBytes, before.softTargetBytes)
        session.close()
        XCTAssertNil(engine, "a retained snapshot must not keep the session's engine alive")
        XCTAssertEqual(Owner.shared.sync { session.core.text.residencyStats.coldEstimatedBytes }, 0)
        let label = try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == "label" })
        XCTAssertEqual(label.lines.map(\.text).joined(), "alpha beta gamma", "uncomputed lines survive close")
        print("HEADLESS_RESIDENCY target=\(before.softTargetBytes) coldBefore=\(before.coldEstimatedBytes) coldAfter=0 engineReleased=true")
        let small = try HeadlessSession(plan: fixturePlan("HEADLESS"), size: CGSize(width: 200, height: 600),
            facts: SessionFacts(displayScale: 2, preferences: [], textResidencyBytes: 1024))
        defer { small.close() }
        XCTAssertEqual(Owner.shared.sync { small.core.text.residencyStats.softTargetBytes }, 1024)
    }

    func testMarkdownIsRefusedAtBootIncludingUnmeasuredNodes() throws {
        for (key, name) in [("HEADLESS_MARKDOWN", "markdown-body"), ("HEADLESS_HIDDEN_MARKDOWN", "hidden-markdown")] {
            let plan = try fixturePlan(key)
            XCTAssertThrowsError(try HeadlessSession(plan: plan, size: CGSize(width: 200, height: 400),
                facts: SessionFacts(displayScale: 1, preferences: []))) { error in
                XCTAssertTrue(String(describing: error).contains(name), "\(error)")
                XCTAssertTrue(String(describing: error).contains("TextPresentation"), "\(error)")
            }
        }
    }

    func testPackageCompositionCanSupplyMarkdownWithoutAView() throws {
        let adapter = MarkdownExpansion()
        let session = try HeadlessSession(plan: fixturePlan("HEADLESS_MARKDOWN"), size: CGSize(width: 200, height: 400),
            facts: SessionFacts(displayScale: 1, preferences: []), assets: AssetResolver(root: Bundle.main.bundleURL), presentation: adapter)
        defer { session.close() }
        let node = try XCTUnwrap(session.snapshot().nodes.values.first { $0.props["testId"] == "markdown-body" })
        XCTAssertEqual(node.lines.map(\.text).joined(), "expanded")
        XCTAssertGreaterThan(adapter.expansions, 1, "both measurement and snapshot use the installed expansion")
    }

}


private final class MarkdownExpansion: TextPresentation {
    var expansions = 0
    func reset() {}
    func checkpoint() -> () -> Void { {} }
    func controlFont(_ engine: TextEngine, size: CGFloat, weight: Int, family: Int, italic: Bool) -> PlatformFont? { nil }
    func measure(_ engine: TextEngine, _ request: ExactMeasureRequest) -> ExactMetrics? { nil }
    func markdown(_ engine: TextEngine, source: Run, atoms: String, faces: String, rtl: Bool) -> [Run]? {
        expansions += 1
        var run = source; run.text = source.text.replacingOccurrences(of: "**", with: ""); return [run]
    }
    func attributes(_ run: Run, color: [Double], attributes: inout [NSAttributedString.Key: Any]) {}
}
