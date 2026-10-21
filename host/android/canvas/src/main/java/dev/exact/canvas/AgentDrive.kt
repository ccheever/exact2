package dev.exact.canvas

import android.net.LocalServerSocket
import android.net.LocalSocket
import android.os.Process
import java.io.IOException
import java.util.concurrent.CountDownLatch

/**
 * The agent API's carrier in the reader (LLP 1012; LLP 1107 D1's `--reader`): `scripts/agent.mjs android --reader`
 * drives the app this activity shows by the Linux host's own protocol, one JSON line a request and one its answer.
 * The lines cross a socket in the abstract namespace, [name], which adb forwards (`adb forward tcp:<port>
 * localabstract:<name>`); each request is answered on exact2's thread ([Exact.agent], [Native.agent]).
 *
 * An activity attaches it at the reader's driver seam ([ExactActivity.drive]) when its launch asks for one (the
 * shell's `agent=<name>` extra). Only adb's own user (shell, or root) is served: any app on the phone can reach an
 * abstract socket, and a driver holds the whole app.
 *
 * The first line out is the handler's `ready`, written once the app's first content has been drawn. A `screenshot`
 * is not answered here: the picture is the device's, and the carrier takes it (`screencap`).
 */
class AgentDrive(name: String) : Drive {
    private val server = LocalServerSocket(name)
    private val drawn = CountDownLatch(1)

    init {
        Thread({ serve() }, "exact-agent").apply { isDaemon = true }.start()
    }

    override fun contentReady() = drawn.countDown()

    private fun serve() {
        while (true) {
            val socket = try { server.accept() } catch (e: IOException) { return }
            try { socket.use { talk(it) } } catch (e: IOException) { /* the driver left */ }
        }
    }

    private fun talk(socket: LocalSocket) {
        val uid = socket.peerCredentials.uid
        if (uid != Process.SHELL_UID && uid != Process.ROOT_UID) return
        val lines = socket.inputStream.bufferedReader(Charsets.UTF_8)
        val out = socket.outputStream
        fun say(line: String) { out.write((line + "\n").toByteArray(Charsets.UTF_8)); out.flush() }
        drawn.await()
        say(Exact.agent("{\"op\":\"ready\"}"))
        while (true) {
            val line = lines.readLine() ?: return
            if (line.isNotBlank()) say(if (AX.containsMatchIn(line)) ax(line) else Exact.agent(line))
        }
    }

    /**
     * `tree --ax` (LLP 1080.002; LLP 1115.001 D13): the accessibility tree Android has for the app, which exact2's
     * own handler cannot answer (it has no window). The reader answers a driver as it answers a service: exact2 is
     * asked for the tree, and each node's own `AccessibilityNodeInfo` is read back on the main thread
     * ([ExactActivity.ax]), stamped with the runner's epoch, incarnation and clock as every reply is.
     */
    private fun ax(line: String): String {
        val activity = Exact.reader?.get() ?: return Exact.agent(line)
        val limit = try { org.json.JSONObject(line).optInt("limit", 500) } catch (e: org.json.JSONException) { 500 }
        fun <T> main(f: () -> T): T {
            val task = java.util.concurrent.FutureTask(f)
            activity.runOnUiThread(task)
            return task.get()
        }
        main { activity.axAsk() }
        // A turn of exact2's thread: it publishes the tree, which the main thread takes before the read below.
        val tags = org.json.JSONObject(Exact.agent("{\"op\":\"tags\"}"))
        val ax = main { activity.ax(limit) }
        return org.json.JSONObject().put("epoch", tags.opt("epoch")).put("incarnation", tags.opt("incarnation"))
            .put("clock", tags.opt("clock")).put("ax", ax).toString()
    }

    private companion object {
        /** A request for the platform's accessibility tree: `{"op":"tree","ax":true,…}`. */
        val AX = Regex("""^\s*\{(?=.*"op"\s*:\s*"tree")(?=.*"ax"\s*:\s*true)""")
    }
}
