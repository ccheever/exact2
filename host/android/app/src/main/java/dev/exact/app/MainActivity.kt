package dev.exact.app

import android.content.pm.ActivityInfo
import android.os.Bundle
import dev.exact.canvas.AgentDrive
import dev.exact.canvas.Drive
import dev.exact.canvas.ExactActivity

/** An app on the Canvas host, edge to edge: its text fields take keys and the software keyboard. */
class MainActivity : ExactActivity() {
    override val input = true
    /** Compose Material 3's, when the plan has a date or time control (`src/compose`). */
    override val pickers get() = materialPickers ?: super.pickers

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        // `natural` arrives as `nosensor` (build.gradle.kts): hold the device's own axis, which a dock or a policy
        // could otherwise turn
        if (requestedOrientation == ActivityInfo.SCREEN_ORIENTATION_NOSENSOR) requestedOrientation = naturalOrientation()
    }

    /** The axis of the display's own panel (its mode's physical size), whatever the rotation and this window's
     * share of the display. */
    private fun naturalOrientation(): Int {
        val mode = display?.mode
        val portrait = mode == null || mode.physicalHeight >= mode.physicalWidth
        return if (portrait) ActivityInfo.SCREEN_ORIENTATION_PORTRAIT else ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
    }

    /** `agent=<name>` at launch: a driver over adb drives this app by the agent API, on a socket of that name
     * ([AgentDrive]; `bun scripts/agent.mjs android --reader`). The process's one: an activity made again in it
     * keeps the driver the first one attached. This shell is a development build. */
    override fun drive(scroll: (Float) -> Unit): Drive? =
        agent ?: intent.getStringExtra("agent")?.let { name -> AgentDrive(name).also { agent = it } }

    private companion object {
        var agent: AgentDrive? = null
    }
}
