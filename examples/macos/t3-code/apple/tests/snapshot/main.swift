import AppKit
import Carbon
import Foundation
let bounds = CGRect(x: 10, y: 20, width: 300, height: 200)
func expect(_ value: Bool, _ name: String) { guard value else { fatalError(name) } }
expect(T3SnapshotAccessibility.uniqueWindow([(nil, bounds), (nil, bounds)], windowId: 12, bounds: bounds) == nil, "ambiguous geometry must not attach other window text")
expect(T3SnapshotAccessibility.uniqueWindow([(12, nil), (nil, bounds)], windowId: 12, bounds: bounds) == 0, "exact window ID takes precedence over geometry")
expect(T3SnapshotAccessibility.uniqueWindow([(13, bounds)], windowId: 12, bounds: bounds) == nil, "known different window ID cannot use geometry fallback")
expect(T3SnapshotAccessibility.uniqueWindow([(nil, bounds)], windowId: 12, bounds: bounds) == 0, "unique unknown-ID geometry can match")
expect(T3SnapshotAccessibility.uniqueWindow([(12, nil), (12, nil)], windowId: 12, bounds: bounds) == nil, "duplicate exact identity is unavailable")
print("5 selected-window identity checks passed")

let active = T3SnapshotLifetime(owner: "draft-a")
expect(active.canPublish(owner: "draft-a", enabled: true, closing: false), "active capture may publish")
expect(!active.cancel(owner: "draft-b") && !active.cancelled, "another draft cannot cancel active AX work")
expect(!active.canPublish(owner: "draft-b", enabled: true, closing: false), "switched draft cannot adopt capture")
expect(!active.canPublish(owner: "draft-a", enabled: false, closing: false), "disable blocks late publication")
expect(!active.canPublish(owner: "draft-a", enabled: true, closing: true), "destroy blocks late publication")
expect(active.cancel(owner: "draft-a") && active.cancelled, "owner cancellation reaches AX phase")
expect(!active.canPublish(owner: "draft-a", enabled: true, closing: false), "re-enable never revives cancelled result")
var instant: TimeInterval = 10
let timeout = T3SnapshotBudget(now: { instant })
expect(timeout.remaining == 3, "timeout starts before window identity reads")
instant = 12.9
expect(timeout.remaining > 0 && timeout.remaining < 0.11, "late AX call receives only remaining budget")
instant = 13
expect(timeout.remaining == 0, "deadline rejects further AX reads")
let cancelledBudget = T3SnapshotBudget(now: { instant }, cancelled: { active.cancelled })
expect(cancelledBudget.remaining == 0, "cancelled AX traversal stops immediately")
let semaphore = DispatchSemaphore(value: 0)
DispatchQueue.global().async { expect(active.cancelled, "background sees cancellation"); semaphore.signal() }
expect(semaphore.wait(timeout: .now() + 1) == .success, "background cancellation read finishes")
print("12 capture lifetime/budget checks passed")
let fixtureRoot = URL(fileURLWithPath: ProcessInfo.processInfo.environment["T3_SNAPSHOT_TEST_ROOT"] ?? "")
expect(fixtureRoot.path.hasPrefix(FileManager.default.currentDirectoryPath + "/target/"), "snapshot fixture must be checkout-local")
let draftId = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"
let drafts = fixtureRoot.appendingPathComponent("snapshots/drafts", isDirectory: true)
try FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl6QXkAAAAASUVORK5CYII=")!
try png.write(to: drafts.appendingPathComponent("\(draftId).png"), options: .atomic)
func operation(_ bridge: T3SnapShot, _ op: String, _ id: String) -> [String: Any] {
    var result: [String: Any] = [:]
    bridge.perform(["op": op, "id": id]) { result = $0 }
    return result
}
let first = T3SnapShot(directory: fixtureRoot, agent: true, changed: { _ in })
expect(operation(first, "snapshotDraftRead", draftId)["ok"] as? Bool == true, "stored draft image is readable without capture grant")
first.destroy()
let reopened = T3SnapShot(directory: fixtureRoot, agent: true, changed: { _ in })
expect(operation(reopened, "snapshotDraftRead", draftId)["ok"] as? Bool == true, "bridge recreation retains draft file")
expect(operation(reopened, "snapshotDraftRemove", "../other")["ok"] as? Bool == false, "non-UUID path cannot remove outside image")
expect(operation(reopened, "snapshotDraftRemove", draftId)["ok"] as? Bool == true, "only owned UUID draft removed")
expect(operation(reopened, "snapshotDraftRemove", draftId)["ok"] as? Bool == true, "cleanup retry is idempotent")
expect(!FileManager.default.fileExists(atPath: drafts.appendingPathComponent("\(draftId).png").path), "released draft file absent")
reopened.destroy()
print("6 native draft-file persistence/cleanup checks passed")

let original = T3SnapshotImage.boundedPNG(png, cancelled: { false })
expect(original?.data == png && original?.width == 1 && original?.height == 1, "small PNG retains bytes and pixel dimensions")
expect(T3SnapshotImage.boundedPNG(png, cancelled: { true }) == nil, "cancelled image work cannot publish")
expect(T3SnapshotImage.boundedPNG(Data([0, 1, 2]), cancelled: { false }) == nil, "invalid image rejected")
print("3 image validation/cancellation checks passed")

let noise = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 128, pixelsHigh: 128, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 128 * 4, bitsPerPixel: 32)!
var seed: UInt32 = 123456
for index in 0..<(128 * 128 * 4) {
    seed = seed &* 1664525 &+ 1013904223
    noise.bitmapData![index] = index % 4 == 3 ? 255 : UInt8(truncatingIfNeeded: seed >> 16)
}
let noisyPNG = noise.representation(using: .png, properties: [:])!
let compressed = T3SnapshotImage.boundedPNG(noisyPNG, limit: 1024, cancelled: { false })
expect(noisyPNG.count > 1024 && compressed != nil, "oversized PNG gets a usable bounded image")
expect(compressed!.data.count <= 1024 && compressed!.width < 128 && compressed!.height < 128, "bounded PNG reports resized pixel geometry")
print("2 oversized-image resize checks passed")

expect(T3SnapshotShortcut.canonical("Command+Shift+A") == "meta+shift+a", "case and command alias")
expect(T3SnapshotShortcut.canonical("CTRL+ALT++") == "ctrl+alt+plus", "literal plus remains a key")
expect(T3SnapshotShortcut.canonical("meta+") == nil, "unfinished trailing plus is not a key")
expect(T3SnapshotShortcut.canonical("shift+shift") == "shift+shift", "modifier pair precedes chord parsing")
expect(T3SnapshotShortcut.canonical("shift+a") == "shift+a", "single modifier letter is a chord")
expect(T3SnapshotShortcut.canonical("meta+mod+a") == nil, "duplicate aliased modifiers rejected")
expect(T3SnapshotShortcut.chord("ctrl+alt+a") != nil, "actual current layout translates printable letter")
expect(T3SnapshotShortcut.chord("ctrl+alt+2") != nil, "actual current layout translates number")
expect(T3SnapshotShortcut.chord("ctrl+alt++") != nil, "actual current layout translates plus")
let shortcuts = T3SnapshotShortcut(changed: {})
let contenders = ["meta+ctrl+alt+f11", "meta+ctrl+alt+f12", "ctrl+alt+shift+f10"]
let old = contenders.first { shortcuts.check($0, saved: "") == nil }!
try shortcuts.install(old, fire: {})
expect(shortcuts.installed == old, "first registration installed")
let competing = T3SnapshotShortcut(changed: {})
expect(competing.check(old, saved: "") != nil, "occupied OS shortcut unavailable")
let alternative = contenders.first { $0 != old && competing.check($0, saved: "") == nil }!
try competing.install(alternative, fire: {})
do { try shortcuts.install(alternative, fire: {}); fatalError("conflicting replacement must fail") } catch {}
expect(shortcuts.installed == old, "failed edit preserves saved registration")
expect(competing.check(old, saved: "") != nil, "old registration still owns chord after failed edit")
competing.destroy()
try shortcuts.install(alternative, fire: {})
expect(shortcuts.installed == alternative, "successful replacement adopts new chord")
expect(competing.check(old, saved: "") == nil, "successful edit releases only old chord")
shortcuts.startRecording(); expect(shortcuts.recording, "recording starts without OS permission prompt")
shortcuts.cancel(); expect(!shortcuts.recording && shortcuts.candidate == "", "cancel leaves candidate unsaved")
shortcuts.startRecording()
NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: NSWindow())
expect(!shortcuts.recording, "blur cancels recorder")
shortcuts.destroy()
expect(competing.check(alternative, saved: "") == nil, "destroy releases owned registration")
print("20 shortcut normalization/layout/transaction/recorder checks passed")
let plus = T3SnapshotShortcut.chord("meta+plus")!, equals = T3SnapshotShortcut.chord("meta+shift+=")!
expect(plus.0 == equals.0 && plus.1 == equals.1, "plus matches effective shifted-equals physical chord on current layout")
expect(plus.1 & UInt32(shiftKey) != 0 && plus.1 & UInt32(cmdKey) != 0, "translation-required Shift is included with requested Command")
let letter = T3SnapshotShortcut.chord("ctrl+alt+a")!, number = T3SnapshotShortcut.chord("ctrl+alt+2")!
expect(letter.0 != number.0 && letter.1 == UInt32(controlKey | optionKey), "letter and number return distinct physical codes and exact requested flags")
shortcuts.startRecording(); let expired = shortcuts.recordEpoch
shortcuts.cancel(); shortcuts.startRecording(); shortcuts.expireRecording(epoch: expired)
expect(shortcuts.recording, "old timeout cannot cancel reopened recorder")
shortcuts.expireRecording(epoch: shortcuts.recordEpoch)
expect(!shortcuts.recording, "current timeout cancels without changing saved registration")
shortcuts.destroy()
print("5 physical modifier and timeout checks passed")
expect(T3SnapshotFeedback.screenFrame(CGRect(x: 10, y: 20, width: 300, height: 200), screenHeight: 900) == CGRect(x: 10, y: 680, width: 300, height: 200), "capture overlay preserves original screen geometry")
print("1 capture animation screen-geometry check passed (no OS capture)")
runFeedbackChecks()
runComposerFocusChecks()
