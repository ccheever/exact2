// The app composition is chosen by the baked target's store level (LLP 1030 D4).
import ExactKit
public enum ExactComposition {
    public static let app: ExactApp = {
        let app = ExactApp.shared
        return app
    }()
}
