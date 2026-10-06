import AppKit
import Carbon
import ApplicationServices

func pause(_ seconds: Double) { RunLoop.current.run(until: Date().addingTimeInterval(seconds)) }
func sourceID(_ source: TISInputSource) -> String {
 guard let pointer = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else { return "unknown" }
 return Unmanaged<CFString>.fromOpaque(pointer).takeUnretainedValue() as String
}
func currentID() -> String { sourceID(TISCopyCurrentKeyboardInputSource().takeRetainedValue()) }
let targetPID = pid_t(CommandLine.arguments.dropFirst().first ?? "40550")!
let tokens = CommandLine.arguments.count > 2 ? CommandLine.arguments[2].split(separator: ",").map(String.init) : ["5","40","1","15","46","3","49"]
let keys: [(UInt16, CGEventFlags)] = tokens.compactMap { token in
 let modifier = token.first
 let flags: CGEventFlags = modifier == "M" ? .maskCommand : modifier == "C" ? .maskControl : modifier == "A" ? .maskAlternate : []
 let number = flags.isEmpty ? token : String(token.dropFirst())
 return UInt16(number).map { ($0, flags) }
}
let koreanID = "com.apple.inputmethod.Korean.2SetKorean"
var failure: String?
if !AXIsProcessTrusted() { print("BLOCKED accessibility false"); exit(2) }
guard let target = NSRunningApplication(processIdentifier: targetPID) else { print("BLOCKED isolated target absent"); exit(3) }
target.activate(options: [.activateAllWindows])
pause(0.5)
guard NSWorkspace.shared.frontmostApplication?.processIdentifier == targetPID else { print("BLOCKED isolated target not frontmost"); exit(4) }
let original = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
let sources = TISCreateInputSourceList(nil, false).takeRetainedValue() as! [TISInputSource]
guard let korean = sources.first(where: { sourceID($0) == koreanID }) else { print("BLOCKED Korean source absent"); exit(5) }
print("original=\(sourceID(original))")
let switchDown = CGEvent(keyboardEventSource: nil, virtualKey: 49, keyDown: true)!
let switchUp = CGEvent(keyboardEventSource: nil, virtualKey: 49, keyDown: false)!
switchDown.flags = .maskControl; switchUp.flags = .maskControl
switchDown.post(tap: .cghidEventTap); pause(0.15); switchUp.post(tap: .cghidEventTap)
print("sourceSwitch=physical Control+Space")
pause(0.7)
print("selectedSource=\(currentID())")
let eventSource = CGEventSource(stateID: .hidSystemState)
for (key, flags) in keys {
 if NSWorkspace.shared.frontmostApplication?.processIdentifier != targetPID { failure = "frontmost changed"; break }
 if currentID() != koreanID { failure = "input source changed: \(currentID())"; break }
 guard let down = CGEvent(keyboardEventSource: eventSource, virtualKey: key, keyDown: true), let up = CGEvent(keyboardEventSource: eventSource, virtualKey: key, keyDown: false) else { failure = "event allocation"; break }
 down.flags = flags; up.flags = flags
 down.keyboardSetUnicodeString(stringLength: 0, unicodeString: nil); up.keyboardSetUnicodeString(stringLength: 0, unicodeString: nil)
 down.setIntegerValueField(.keyboardEventAutorepeat, value: 0)
 down.post(tap: .cghidEventTap)
 pause(0.12)
 up.post(tap: .cghidEventTap)
 pause(0.35)
 print("key=\(key) flags=\(flags.rawValue) source=\(currentID()) targetStillFrontmost=\(NSWorkspace.shared.frontmostApplication?.processIdentifier == targetPID)")
}
pause(0.3)
let restored = TISSelectInputSource(original)
pause(0.3)
print("restoreStatus=\(restored) restoredSource=\(currentID())")
if let failure { print("BLOCKED \(failure)"); exit(6) }
print("DONE physical keys only; no Enter or shell command")
