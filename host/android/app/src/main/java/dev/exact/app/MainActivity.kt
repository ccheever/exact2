package dev.exact.app

import dev.exact.canvas.ExactActivity

/** An app on the Canvas host: the view is the content area (below the status bar, above the navigation bar), and its
 * text fields take keys and the software keyboard. */
class MainActivity : ExactActivity() {
    override val input = true
    override val insetBottom = true
}
