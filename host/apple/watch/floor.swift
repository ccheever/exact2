// The watch floor: an empty hand-built SwiftUI watch app, the launch a watch app
// costs before any of Exact's (LLP 1126.002 §4, the startup budget: Exact's share
// is the watch app's first frame less this one's, same watch, same build kind).
// It prints its first frame in ms after the kernel started the process, as the
// watch app's launch phases line does. To build and run it on a watch:
//   1. `xcrun --sdk watchos swiftc -O -parse-as-library -target arm64-apple-watchos26.0
//      -sdk $(xcrun --sdk watchos --show-sdk-path) -o ExactFloor.app/ExactFloor floor.swift`
//   2. `ExactFloor.app/Info.plist`: CFBundleIdentifier `com.exact.floor.watchkitapp`,
//      CFBundleExecutable `ExactFloor`, CFBundlePackageType `APPL`, CFBundleVersion `1`,
//      CFBundleShortVersionString `0.1.0`, CFBundleSupportedPlatforms `[WatchOS]`,
//      DTPlatformName `watchos`, MinimumOSVersion `26.0`, UIDeviceFamily `[4]`,
//      WKApplication and WKWatchOnly `true`.
//   3. Copy a development profile naming the watch to `embedded.mobileprovision`; sign
//      with entitlements `application-identifier` (`<prefix>.com.exact.floor.watchkitapp`),
//      `com.apple.developer.team-identifier` and `get-task-allow`: `codesign --force
//      --sign <identity> --entitlements <plist> ExactFloor.app`.
//   4. `xcrun devicectl device install app --device <watch> ExactFloor.app`, then ten or
//      more `xcrun devicectl device process launch --console --terminate-existing
//      --device <watch> com.exact.floor.watchkitapp`, the watch awake; take the median.
import SwiftUI

private func processStart() -> Date {
    var info = kinfo_proc(), size = MemoryLayout<kinfo_proc>.stride
    var name: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
    guard sysctl(&name, 4, &info, &size, nil, 0) == 0 else { return Date() }
    let start = info.kp_proc.p_starttime
    return Date(timeIntervalSince1970: Double(start.tv_sec) + Double(start.tv_usec) / 1_000_000)
}

@main struct ExactFloor: App {
    init() { setvbuf(stdout, nil, _IOLBF, 0) }
    var body: some Scene {
        WindowGroup {
            Color.clear.onAppear {
                print("exact-floor first frame \(Int((Date().timeIntervalSince(processStart()) * 1000).rounded()))")
            }
        }
    }
}
