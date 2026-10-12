package dev.exact.app

import dev.exact.canvas.AgentDrive
import dev.exact.canvas.Drive
import dev.exact.canvas.ExactActivity

/** An app on the Canvas host, edge to edge: its text fields take keys and the software keyboard. */
class MainActivity : ExactActivity() {
    override val input = true

    /** `agent=<name>` at launch: a driver over adb drives this app by the agent API, on a socket of that name
     * ([AgentDrive]; `bun scripts/agent.mjs android --reader`). The process's one: an activity made again in it
     * keeps the driver the first one attached. This shell is a development build. */
    override fun drive(scroll: (Float) -> Unit): Drive? =
        agent ?: intent.getStringExtra("agent")?.let { name -> AgentDrive(name).also { agent = it } }

    private companion object {
        var agent: AgentDrive? = null
    }
}
