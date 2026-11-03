# Fullscreen browser and device system bars

The browser and device routes now mark their fullscreen route with `navigationSystemBars="hidden"`. Exact's fullscreen modal owner reads that property and forwards UIKit's status-bar, home-indicator and edge-gesture preferences, so the native presentation owns the system chrome while the route remains Contract-authored. The marker is scoped to the two preview routes; ordinary modals keep the platform defaults.

The focused Contract and iOS modal tests pass. A simulator app build also compiles the native module and generated data-key table.
