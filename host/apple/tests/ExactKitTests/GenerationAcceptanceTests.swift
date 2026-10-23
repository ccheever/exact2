import Foundation
import CoreText
import CExact
import XCTest
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

final class GenerationAcceptanceTests: XCTestCase {
    func testRefusedPreparationDoesNotRegisterItsFontsInTheProcess() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf").standardized
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent("candidate.ttf")
        try FileManager.default.copyItem(at: fixture, to: url)
        var registrationAttempted = false
        defer {
            if registrationAttempted { CTFontManagerUnregisterFontsForURL(url as CFURL, .process, nil) }
            try? FileManager.default.removeItem(at: directory)
        }
        let descriptors = try XCTUnwrap(CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor])
        let descriptor = try XCTUnwrap(descriptors.first)
        let name = CTFontCopyPostScriptName(CTFontCreateWithFontDescriptor(descriptor, 16, nil)) as String
        func available() -> Bool { (CTFontManagerCopyAvailablePostScriptNames() as? [String] ?? []).contains(name) }
        try XCTSkipIf(available(), "the fixture face is already installed in this process")
        registrationAttempted = true
        let app = ExactApp.shared, session = ExactApp.shared.makeSession(label: "refused-font")
        defer { session.destroy() }
        let text = TextEngine.pair(resolve: { _ in url }, read: { _ in try? Data(contentsOf: url) }, bundled: { _ in url })
        let family = Array("Candidate".utf8), source = Array("candidate.ttf".utf8)
        family.withUnsafeBufferPointer { family in source.withUnsafeBufferPointer { source in
            var face = ExactFontFace()
            face.family = family.baseAddress; face.family_len = family.count
            face.source = source.baseAddress; face.source_len = source.count
            face.stack = 8; face.weight = 400
            withUnsafePointer(to: &face) { face in
                var catalog = ExactFontCatalog(); catalog.faces = face; catalog.count = 1
                withUnsafePointer(to: &catalog) { text.install($0) }
            }
        } }
        session.configureCommit(ExactSession.Prepared(text: text, resolver: app.resolver, token: 0))
        session.cancelCommit()
        XCTAssertFalse(available(), "a refused generation must leave process font registration unchanged")
        text.commitFonts()
        XCTAssertTrue(available(), "the fixture registers when accepted")
    }

    func testAChangedSessionRefusesTheWholeCompositionBeforeDurableCommit() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let app = ExactApp.shared, plan = try fixturePlan("HEADLESS")
        let sessions = [app.makeSession(label: "generation-a"), app.makeSession(label: "generation-b")]
        defer { for session in sessions { session.destroy() } }
        for session in sessions {
            session.presenter.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 400)
            XCTAssertNil(session.boot(size: CGSize(width: 200, height: 400)).error)
            XCTAssertTrue(session.apply(plan))
            XCTAssertNotNil(session.prepare(plan, resolver: app.resolver))
        }
        let session = sessions[0]
        let box = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "box" })
        session.apply(session.runtime.press(box.id, now: session.now()))
        func slots() throws -> [[String: String]] {
            try sessions.map { session in
                let data = Data(session.agent(#"{"op":"state"}"#).utf8)
                let state = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
                return try XCTUnwrap(state["slots"] as? [String: String])
            }
        }
        let before = try slots()
        XCTAssertEqual(before.map { $0["suffix"] }, ["delta epsilon zeta", "beta gamma"])
        var durableCommits = 0
        let refused = Runtime.commitTogether(sessions.map(\.runtime)) { durableCommits += 1; return true }
        XCTAssertNil(refused.batches)
        XCTAssertTrue(refused.pending)
        XCTAssertEqual(durableCommits, 0)
        XCTAssertEqual(try slots(), before)
        for session in sessions { session.runtime.discardPlan() }
        XCTAssertTrue(app.applyGeneration(ExactGeneration(plan: plan, assets: app.resolver)) {
            XCTAssertTrue(Thread.isMainThread)
            durableCommits += 1; return true
        })
        XCTAssertEqual(durableCommits, 1)
        XCTAssertEqual(try slots(), before)
    }
}
