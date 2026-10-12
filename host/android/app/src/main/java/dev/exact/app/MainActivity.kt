package dev.exact.app

import dev.exact.canvas.ExactActivity

/** An app on the Canvas host, edge to edge: its text fields take keys and the software keyboard. */
class MainActivity : ExactActivity() {
    override val input = true
}
