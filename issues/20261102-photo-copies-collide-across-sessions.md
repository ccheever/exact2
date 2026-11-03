# Photo copies in separate sessions overwrite one another

**Status:** Closed
**Severity:** P2
**Systems:** Photos capability, Apple package lifecycle, app temporary storage
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

Two sessions of one app each copy their first photo to
`app:/tmp/photos/1.<extension>`. `PhotoKitLibrary` owns an instance-local
`copies` counter and uses it to allocate a path (`Library.swift:285-289`).
The app's sessions share temporary roots: `host/apple/src/picker.rs:22-66`
derives them from the app id, and `NativeModule.swift:654-657` passes the
session's app roots to its module. The later copy removes and replaces
the file an earlier session was given. No overlapping calls are required.

The agent implementation has the same allocation in
`packages/photos/apple/Photos.swift:143`. Use unique names across all
sessions sharing the root, or allocate names from a shared owner. An
omitted path should not overwrite another copy still in use.

The local reproduction compiles all three unchanged Apple Photos sources
and uses their existing image fixtures. It invokes only the agent branch;
no PhotoKit call, permission request, live library, or device is involved.
Two module instances use the same scratch root, as production sessions of
one app do. Sequential copies of `agent-1` and `agent-2` answer the same
path, and the second changes the first path's bytes.

Create `/tmp/exact-review-photo-copies`, then save this context shim as
`ExactPackage.swift` there:

```swift
import Foundation
public protocol ExactPackage {}
public struct ExactPackageContext {
 public let base:URL
 public var agent=true
 public var origin="app"
 public init(base:URL) {self.base=base}
 public func file(_ path:String)->URL? {
  guard path.hasPrefix("app:/") else {return nil}
  return base.appendingPathComponent(String(path.dropFirst(5)))
 }
 public func changed(_ topic:String) {}
 public func log(_ line:String) {}
 public func now()->Double {0}
 public func revokePicture(_ source:String) {}
 public func pictureChanged(_ source:String) {}
}
```

Save this driver as `main.swift` there:

```swift
import Foundation
import ExactPackage
let root=URL(fileURLWithPath:CommandLine.arguments[1],isDirectory:true)
try! FileManager.default.createDirectory(at:root,withIntermediateDirectories:true)
defer {try? FileManager.default.removeItem(at:root)}
let context=ExactPackageContext(base:root)
let first=PhotosModule(context:context)
first.preferred(["photoLibraryStatus":["access":"full"]])
var answerA:[String:Any]=[:]
first.later("copyPhotoAsset",["asset":"agent-1"],preferred:nil){answerA=$0}
let pathA=answerA["path"] as! String
let before=try! Data(contentsOf:context.file(pathA)!)
let second=PhotosModule(context:context)
second.preferred(["photoLibraryStatus":["access":"full"]])
var answerB:[String:Any]=[:]
second.later("copyPhotoAsset",["asset":"agent-2"],preferred:nil){answerB=$0}
let pathB=answerB["path"] as! String
let after=try! Data(contentsOf:context.file(pathA)!)
print("first copy=\(pathA), second copy=\(pathB)")
print("first copy changed=\(before != after)")
precondition(pathA == pathB && before != after)
```

From the reviewed checkout:

```sh
swiftc -emit-module -emit-library -module-name ExactPackage /tmp/exact-review-photo-copies/ExactPackage.swift -o /tmp/exact-review-photo-copies/libExactPackage.dylib
swiftc -I /tmp/exact-review-photo-copies -L /tmp/exact-review-photo-copies -lExactPackage -Xlinker -rpath -Xlinker /tmp/exact-review-photo-copies packages/photos/apple/Photos.swift packages/photos/apple/AgentLibrary.swift packages/photos/apple/Library.swift /tmp/exact-review-photo-copies/main.swift -o /tmp/exact-review-photo-copies/probe
/tmp/exact-review-photo-copies/probe /tmp/exact-review-photo-copies/storage
```

Observed:

```text
first copy=app:/tmp/photos/1.jpg, second copy=app:/tmp/photos/1.jpg
first copy changed=true
```

This proves the collision in the actual agent-copy implementation with
shared app roots. The PhotoKit path has the same counter and naming
scheme, but was source-reviewed only. The probe creates and removes only
its scratch storage directory.

Duplicate check: searched current issues and both related issue corpora,
including closed tickets. The related
[photo editor save issue](20261102-two-photo-saves-share-one-temp-path.md)
is a separate app-level race: it counts `app:/tmp/edits` across overlapping
awaits. Fixing that app cannot fix this package-level collision between
separate modules, including sequential copies in unrelated consumers.

**Resolution.** Use UUID names for omitted copy destinations in both PhotoKit and agent implementations, so separate sessions sharing a temporary root do not overwrite each other.

**Verification after the fix.** Compiled all three actual Apple photo sources and ran two module instances against one root. Copies returned distinct paths and the first copy’s bytes remained unchanged. The PhotoKit branch typechecked; no live photo library was used.
