// @ref llp/1109.010-mobile-browser-devices.decision.md#native-lifetime
#if os(iOS)
import UIKit
import CoreMotion

final class T3MobileDevices: T3MobileBrowser {
    init(transport: T3Transport, changed: @escaping (String) -> Void, agent: Bool, audioSession: T3MobileAudioSession) {
        super.init(transport: transport, changed: changed, agent: agent, audioSession: audioSession, devices: true)
    }
}

/// Source shakeDetector.ts: two jolts >=1.8g within600ms; cooldown1000ms. Active view only.
final class T3MobileDevicesShake {
    private let motion = CMMotionManager()
    private var jolts: [Double] = [], last = -Double.infinity
    func start(_ changed: @escaping () -> Void) {
        guard motion.isAccelerometerAvailable, !motion.isAccelerometerActive else { return }
        motion.accelerometerUpdateInterval = 0.05
        motion.startAccelerometerUpdates(to: .main) { [weak self] data, _ in
            guard let self, let data else { return }; let time = data.timestamp * 1000, a = data.acceleration
            guard time - last >= 1000, sqrt(a.x * a.x + a.y * a.y + a.z * a.z) >= 1.8 else { return }
            jolts = jolts.filter { time - $0 <= 600 }; jolts.append(time)
            if jolts.count >= 2 { jolts.removeAll(); last = time; UISelectionFeedbackGenerator().selectionChanged(); changed() }
        }
    }
    func stop() { motion.stopAccelerometerUpdates(); jolts.removeAll() }
    deinit { stop() }
}
#endif
