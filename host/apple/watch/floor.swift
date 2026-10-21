// The watch floor: an empty hand-built SwiftUI watch app, the launch a watch app
// costs before any of Exact's (LLP 1126.002 §4, the startup budget). It prints its
// first frame in ms after the kernel started the process, as the watch app's
// `exact-watch phases` line does: `swiftc -O -parse-as-library -target
// arm64-apple-watchos26.0 -sdk <watchos> -o <bundle>/ExactFloor host/apple/watch/floor.swift`.
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
