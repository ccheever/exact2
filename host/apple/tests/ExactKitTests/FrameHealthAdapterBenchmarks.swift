import Foundation
import QuartzCore
import XCTest
@testable import ExactKit

/// Opt-in release measurement, not an automatic performance gate or display/energy qualification.
final class FrameHealthAdapterBenchmarks: XCTestCase {
    func testSwiftAdapterClockAndReducerCost() throws {
        guard ProcessInfo.processInfo.environment["EXACT_FRAME_HEALTH_BENCHMARK"] == "1" else {
            throw XCTSkip("set EXACT_FRAME_HEALTH_BENCHMARK=1 in a release test run")
        }
        guard !_isDebugAssertConfiguration() else { throw XCTSkip("adapter cost must be measured with optimized Swift") }
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        XCTAssertTrue(health.enabled)
        var arms: [[String: Any]] = []
        for detail in [false, true] {
            health.setHUDVisible(detail)
            health.reset()
            var means: [Double] = []
            for _ in 0..<31 {
                let began = DispatchTime.now().uptimeNanoseconds
                for _ in 0..<100_000 {
                    let arrival = CACurrentMediaTime()
                    health.record(arrival: arrival, target: arrival + 1 / 120, period: 1 / 120, finished: CACurrentMediaTime())
                }
                means.append(Double(DispatchTime.now().uptimeNanoseconds - began) / 100_000)
            }
            means.sort()
            arms.append(["detail": detail, "medianBatchMeanNs": means[15], "maximumBatchMeanNs": means[30]])
        }
        let report: [String: Any] = ["benchmark": "Swift-adapter-two-clock-reads-and-Rust-reducer/1", "arms": arms,
            "qualification": "unqualified", "excludes": ["frame workload", "UI formatting and composition", "observer", "report export", "energy"],
            "os": ProcessInfo.processInfo.operatingSystemVersionString]
        let data = try JSONSerialization.data(withJSONObject: report, options: [.sortedKeys])
        XCTAssertLessThanOrEqual(data.count + 1, 4000)
        print(String(decoding: data, as: UTF8.self))
    }
}
