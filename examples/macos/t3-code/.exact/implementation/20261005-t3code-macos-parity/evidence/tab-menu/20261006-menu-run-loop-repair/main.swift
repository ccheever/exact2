import AppKit
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let window = NSWindow(contentRect: NSRect(x: 200,y:200,width:500,height:300),styleMask:[.titled,.closable],backing:.buffered,defer:false)
window.title="T3 menu regression"
window.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps:true)
let mode = CommandLine.arguments.dropFirst().first ?? "select"
let start = Date()
var settleTimedOut = false
func log(_ s:String) { print(String(format:"%.3f",Date().timeIntervalSince(start)),s); fflush(stdout) }
var complete = false
DispatchQueue.main.asyncAfter(deadline:.now()+1) {
 log("open")
 T3ContextMenu.perform(["generation":1,"anchor":"focus","items":[["id":"copy","label":"Copy path"]]]) { reply in
 let picked = (reply["value"] as? [String: Any])?["clicked"]
 let correct = mode == "escape" ? picked is NSNull : picked as? String == "copy"
 guard correct else { log("wrong choice \(reply)"); exit(3) }
 log("reply \(reply)")
 DispatchQueue.main.async {
  complete = true
  log("main-queue continuation")
  guard mode == "settle" || Date().timeIntervalSince(start) < 3.5 else { exit(5) }
  exit(mode == "settle" && !settleTimedOut ? 4 : 0)
 }
 }
}
DispatchQueue.global().asyncAfter(deadline:.now()+2) {
 for code:UInt16 in (mode == "escape" ? [53] : [125,36]) {
  CGEvent(keyboardEventSource:nil,virtualKey:code,keyDown:true)?.post(tap:.cghidEventTap)
  CGEvent(keyboardEventSource:nil,virtualKey:code,keyDown:false)?.post(tap:.cghidEventTap)
  Thread.sleep(forTimeInterval:0.005)
 }
 if mode == "settle" {
  CFRunLoopPerformBlock(CFRunLoopGetMain(),CFRunLoopMode.commonModes.rawValue) {
   log("settle enter")
   let deadline=Date(timeIntervalSinceNow:3)
   while !complete && Date()<deadline { RunLoop.main.run(until:Date(timeIntervalSinceNow:0.02)) }
   settleTimedOut = !complete
   log("settle exit complete=\(complete)")
  }
  CFRunLoopWakeUp(CFRunLoopGetMain())
 }
}
DispatchQueue.global().asyncAfter(deadline:.now()+8) { log("timeout"); exit(2) }
app.run()
log("done")
exit(complete ? 0:1)
