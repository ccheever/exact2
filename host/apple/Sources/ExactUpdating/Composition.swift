// The app composition is chosen by the baked target's store level (LLP 1030 D4).
import ExactKit
#if EXACT_LINK_GROUPED_LISTS
import ExactGroupedLists
#endif
import ExactUpdates
public enum ExactComposition {
    public static let app: ExactApp = {
        // The linked capabilities' Swift halves (LLP 1047.001 D4), before
        // any session is made.
        #if EXACT_LINK_GROUPED_LISTS
        ExactGroupedLists.install()
        #endif
        let app = ExactApp.shared
        ExactUpdates.install(on: app)
        return app
    }()
}
