# A blocked serial write prevents close and session teardown

**Status:** Closed
**Severity:** P2
**Systems:** Serial capability, macOS package lifecycle
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

`packages/serial/apple/Serial.swift:279-293` retries `EAGAIN` forever,
polling one second at a time when a connected device stops accepting
bytes. This runs on the module's sole serial queue (`:151`). Both `close`
(`:154`) and `destroy` (`:331-337`) enqueue their work on the same queue.
They cannot cancel that write or release the port until the peer reads
again. A stalled peripheral therefore prevents the app from closing its
connection and prevents the module's teardown completion from running.

Move pending output to cancellable asynchronous work, or let close and
teardown signal cancellation independently of the blocked queue. Waiting
for output backpressure is reasonable; blocking the operation that ends
the connection is the defect.

Reproduced on this Mac with the unchanged Swift module and an owned
pseudoterminal. The development fixture path selects the PTY but uses the
same production open/write/close code. A 1 MiB write fills the unread PTY;
after two seconds neither the queued close nor destroy callback has run.
Reading the PTY then permits all three callbacks, in order:

```text
close and destroy enqueued
after 2 seconds: PTY has unread bytes=true
write callback
close callback
destroy callback
drained bytes=1048576
close and destroy completed only after draining peer
```

The probe directory is `/tmp/exact-review-serial`. Create it and put this
minimal host context in `ExactPackage.swift`:

```swift
import AppKit
public protocol ExactPackage {}
public struct Presentation { public var window: NSWindow? = nil }
public struct ExactPackageContext {
 public var presentation: Presentation? = nil
 public var changed: (String)->Void = { _ in }
 public init() {}
}
```

Put this driver in `main.swift`:

```swift
import AppKit
import Darwin
import ExactPackage
import Foundation
var master:Int32 = -1, slave:Int32 = -1
var name=[CChar](repeating:0,count:1024)
precondition(openpty(&master,&slave,&name,nil,nil)==0)
let path=String(cString:name)
let fixture=CommandLine.arguments[1]
try! JSONSerialization.data(withJSONObject:["serial":[["id":"probe","name":"Probe PTY","path":path]]]).write(to:URL(fileURLWithPath:fixture))
setenv("EXACT_DEVICE_FIXTURE",fixture,1)
let module=SerialModule(context:ExactPackageContext())
let opened=DispatchSemaphore(value:0)
var handle=""
module.later("open",["port":"probe","baudRate":9600],preferred:nil) { answer in
 handle=answer["connection"] as? String ?? ""
 print("open callback: \(answer)");fflush(stdout);opened.signal()
}
precondition(opened.wait(timeout:.now()+2) == .success && !handle.isEmpty)
Darwin.close(slave)
module.later("write",["connection":handle,"data":Data(repeating:1,count:1024*1024).base64EncodedString()],preferred:nil) { _ in
 print("write callback");fflush(stdout)
}
Thread.sleep(forTimeInterval:0.2)
let closed=DispatchSemaphore(value:0), destroyed=DispatchSemaphore(value:0)
module.later("close",["connection":handle],preferred:nil) { _ in
 print("close callback");fflush(stdout);closed.signal()
}
module.destroy { print("destroy callback");fflush(stdout);destroyed.signal() }
print("close and destroy enqueued");fflush(stdout)
Thread.sleep(forTimeInterval:2)
var p = pollfd(fd: master, events: Int16(POLLIN), revents:0)
_ = poll(&p,1,0)
print("after 2 seconds: PTY has unread bytes=\(p.revents & Int16(POLLIN) != 0)");fflush(stdout)
precondition(closed.wait(timeout:.now()) == .timedOut)
precondition(destroyed.wait(timeout:.now()) == .timedOut)
let readerDone=DispatchSemaphore(value:0)
DispatchQueue.global().async {
 let flags=fcntl(master,F_GETFL)
 _ = fcntl(master,F_SETFL,flags | O_NONBLOCK)
 let due=Date().addingTimeInterval(4)
 var bytes=[UInt8](repeating:0,count:65536)
 var total=0
 while Date() < due {
  let n=read(master,&bytes,bytes.count)
  if n > 0 { total += n; continue }
  if n < 0 && errno == EIO { break }
  var p=pollfd(fd:master,events:Int16(POLLIN),revents:0)
  _ = poll(&p,1,10)
 }
 print("drained bytes=\(total)");fflush(stdout);readerDone.signal()
}
precondition(closed.wait(timeout:.now()+3) == .success)
precondition(destroyed.wait(timeout:.now()+3) == .success)
precondition(readerDone.wait(timeout:.now()+5) == .success)
Darwin.close(master)
print("close and destroy completed only after draining peer");fflush(stdout)
```

From the reviewed checkout, compile and run it:

```sh
swiftc -emit-module -emit-library -module-name ExactPackage /tmp/exact-review-serial/ExactPackage.swift -o /tmp/exact-review-serial/libExactPackage.dylib
swiftc -D EXACT_DEVELOPMENT -I /tmp/exact-review-serial -L /tmp/exact-review-serial -lExactPackage -Xlinker -rpath -Xlinker /tmp/exact-review-serial packages/serial/apple/Serial.swift /tmp/exact-review-serial/main.swift -o /tmp/exact-review-serial/probe
/tmp/exact-review-serial/probe /tmp/exact-review-serial/fixture.json
```

The context shim only supplies callbacks and a presentation property; the
serial implementation and POSIX operations are the repository's actual
code. No physical serial device or app was used. The probe drains its
peer and exits normally.

Duplicate check: no matching blocked-write/close issue in current issues
or either related issue corpus, including closed tickets. The separate
serial enumeration ticket concerns opening ports while listing them.

**Resolution.** Queue pending writes and resume them from DispatchSourceWrite readiness. EAGAIN yields the serial queue; close, destroy and unplug finish pending writes instead of waiting for output capacity.

**Verification after the fix.** The actual Serial.swift typechecked and passed a pseudoterminal harness: a 1 MiB write into an unread peer stayed pending, and close and destroy each completed it with AbortError without draining the peer. Physical serial hardware was not used.
