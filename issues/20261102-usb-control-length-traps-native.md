# An oversized USB control transfer crashes the macOS host

**Status:** Closed
**Severity:** P2
**Systems:** USB capability, macOS native bridge, numeric validation
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

`controlTransferIn({connection, setup, length: 65536})` reaches a trapping
integer conversion in `packages/usb/apple/USB.swift:265`. The interface
(`packages/usb/exact.json`, `controlTransferIn.options.length`) accepts an
unconstrained `number`. The implementation allocates the requested buffer
and converts its length with `UInt16(data.length)` without checking its
range. Swift terminates the process instead of answering a rejected call.
The same conversion handles an outgoing control payload over 65,535 bytes.

Validate the control-transfer buffer size before allocation and before
constructing `IOUSBDeviceRequest`. Invalid input should return an error
through the package reply. This report concerns the measured length
boundary, not other unchecked numeric conversions in the device bridges.

Reproduced on this Mac using the unchanged `control` method and local
stand-ins for its device and request structure. No USB device is required,
and the oversized case traps before the stand-in's send method. Save the
following as `/tmp/exact-review-usb-size.swift`:

```swift
import Foundation
struct IOUSBDeviceRequest {var bmRequestType:UInt8=0;var bRequest:UInt8=0;var wValue:UInt16=0;var wIndex:UInt16=0;var wLength:UInt16=0}
final class Device {
 func __send(_ request:IOUSBDeviceRequest,data:NSMutableData,bytesTransferred:inout Int,completionTimeout:Int) throws {bytesTransferred=0}
}
final class Shared {let device=Device()}
final class Connection {let shared=Shared();var claimed:Set<Int>=[]}
final class Probe {
 static let pipeStalled=Int(Int32(bitPattern:0xE000404F))
    private func control(_ c: Connection, _ args: [String: Any], in incoming: Bool) throws -> [String: Any] {
        let setup = args["setup"] as? [String: Any] ?? [:]
        let types = ["standard": 0, "class": 1, "vendor": 2], recipients = ["device": 0, "interface": 1, "endpoint": 2, "other": 3]
        let index = setup["index"] as? Int ?? 0
        if setup["recipient"] as? String == "interface", !c.claimed.contains(index & 0xFF) {
            return ["error": "InvalidStateError", "message": "the interface is not claimed"]
        }
        let requestType = (incoming ? 0x80 : 0) | ((types[setup["requestType"] as? String ?? ""] ?? 0) << 5) | (recipients[setup["recipient"] as? String ?? ""] ?? 0)
        let data = incoming ? NSMutableData(length: args["length"] as? Int ?? 0)! : NSMutableData(data: Data(base64Encoded: args["data"] as? String ?? "") ?? Data())
        var request = IOUSBDeviceRequest()
        request.bmRequestType = UInt8(requestType)
        request.bRequest = UInt8(setup["request"] as? Int ?? 0)
        request.wValue = UInt16(setup["value"] as? Int ?? 0)
        request.wIndex = UInt16(index)
        request.wLength = UInt16(data.length)
        var transferred = 0
        do {
            try c.shared.device.__send(request, data: data, bytesTransferred: &transferred, completionTimeout: 5)
        } catch let e as NSError where e.code == Self.pipeStalled {
            return incoming ? ["status": "stall", "data": ""] : ["status": "stall", "bytesWritten": 0]
        }
        return incoming ? ["status": "ok", "data": Data(data.prefix(transferred)).base64EncodedString()] : ["status": "ok", "bytesWritten": transferred]
    }


 func run(_ length:Int) throws -> [String:Any] {
  try control(Connection(),["setup":["requestType":"vendor","recipient":"device","request":0,"value":0,"index":0],"length":length],in:true)
 }
}
print(try Probe().run(Int(CommandLine.arguments[1])!))
```

Compile and run both sides of the boundary in child processes:

```sh
swiftc /tmp/exact-review-usb-size.swift -o /tmp/exact-review-usb-size
python3 - <<'PY'
import resource, subprocess
for length in ['65535', '65536']:
    result = subprocess.run(['/tmp/exact-review-usb-size', length],
        text=True, capture_output=True, timeout=5,
        preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_CORE, (0, 0)))
    first = result.stderr.splitlines()[0] if result.stderr else ''
    print(length, result.returncode, result.stdout.strip(), first)
PY
```

Observed: 65,535 exits normally with `status: ok`; 65,536 exits `-5`
(SIGTRAP), with `Fatal error: Not enough bits to represent the passed
value`. The method body in the fixture is copied verbatim from the
reviewed source. This proves its parameter-preparation crash; no app was
launched and no physical transfer was attempted. The ordinary call path
requires an open, admitted USB connection before invoking this method.

Duplicate check: no matching USB control-length/native numeric crash ticket
in this checkout or either related issue corpus, including closed tickets.

**Resolution.** Validate incoming and outgoing control-transfer lengths before allocation or UInt16 conversion. Accept only lengths from 0 through 65535 and reject oversized base64 payloads before decoding.

**Verification after the fix.** The actual USB.swift typechecked. A Swift harness of the incoming method checked -1, 0, 65535 and 65536; accepted boundaries returned normally and invalid values returned TypeError. Physical USB transfers were not exercised.
