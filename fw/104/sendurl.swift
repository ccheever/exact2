import Foundation
import Carbon
let pid = pid_t(CommandLine.arguments[1])!, url = CommandLine.arguments[2]
let target = NSAppleEventDescriptor(processIdentifier: pid)
let event = NSAppleEventDescriptor(eventClass: AEEventClass(kInternetEventClass), eventID: AEEventID(kAEGetURL), targetDescriptor: target, returnID: AEReturnID(kAutoGenerateReturnID), transactionID: AETransactionID(kAnyTransactionID))
event.setParam(NSAppleEventDescriptor(string: url), forKeyword: AEKeyword(keyDirectObject))
var reply = AppleEvent()
let err = AESendMessage(event.aeDesc!, &reply, AESendMode(kAENoReply), kAEDefaultTimeout)
print("AESendMessage pid \(pid): \(err)")
