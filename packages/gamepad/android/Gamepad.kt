// @exact/gamepad on Android and Android TV (LLP 1133.022 D1-D7, D12): the process's pads (the canvas library's
// `Pads`, through the context's `pads`), which a world canvas reads too, so an index names one pad to both. The build
// makes one per host when the app binds the extension, outside the agent; under the agent the runner's substitute
// answers and none is made.
//
// - The list: each slot's `InputDevice`, `mapping` `standard` (the registry reads every pad as standard), `family` by
//   USB vendor (D2's table), `labels` the family's; `current` is the pad whose button last went down. A reader with
//   `interval` also reads buttons and axes as the registry last read them, announced at most once per interval when
//   they changed.
// - Effects and lights are claims the process keeps, whichever instance made them (D4): a new effect on a pad
//   preempts the one playing there; a disconnect ends every claim on the pad; `destroy`, or `setLight()`, gives back
//   only this instance's. `dual-rumble` vibrates the device's `VibratorManager` (API 31): the strong magnitude on its
//   first vibrator and the weak on its second, or the larger on its only one, in one `CombinedVibration`; below API
//   31, `InputDevice.getVibrator()`. `trigger-rumble` is no pad's here. Android needs `VIBRATE`, a normal permission
//   with no prompt, which the interface's `playEffect` derives into the manifest wherever the app sends it.
// - A light is the device's RGB light through a `LightsManager.LightsSession` (API 31): the newest surviving claim's
//   colour; closing the session gives the light back to the system, as the app's background does until it returns.
// - Tier 3 (D8): `GamepadModule.hatch` is given each pad's device id when it takes a slot.
// - A drive's pad (`PadDevice.test`, the drive's `EXACT_AGENT_PADS`) has two motors and a light, which are a
//   recorder: each request is a `context.log` line, as the Linux and Apple hosts journal theirs.
package exact.gamepad

import android.os.Build
import android.os.CombinedVibration
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import dev.exact.canvas.PadDevice
import org.json.JSONArray
import org.json.JSONObject

class GamepadModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D8): each pad's device id when it takes a slot. */
        @JvmStatic var hatch: ((Int) -> Unit)? = null

        /** D2's labels by family, by standard index: the interface's `tables.labels`. */
        private val LABELS = mapOf(
            "xbox" to listOf("A", "B", "X", "Y", "LB", "RB", "LT", "RT", "View", "Menu", "LS", "RS", "Up", "Down", "Left", "Right", "Xbox"),
            "playstation" to listOf("Cross", "Circle", "Square", "Triangle", "L1", "R1", "L2", "R2", "Create", "Options", "L3", "R3", "Up", "Down", "Left", "Right", "PS"),
            "nintendo" to listOf("B", "A", "Y", "X", "L", "R", "ZL", "ZR", "−", "+", "LS", "RS", "Up", "Down", "Left", "Right", "Home"),
            "generic" to listOf("A", "B", "X", "Y", "L1", "R1", "L2", "R2", "Select", "Start", "L3", "R3", "Up", "Down", "Left", "Right", "Home"),
        )
        private const val LONGEST = 5000L

        fun family(device: PadDevice): String = when (device.vendorId) {
            0x045E -> "xbox"
            0x054C -> "playstation"
            0x057E -> "nintendo"
            else -> "generic"
        }

        private val main = Handler(Looper.getMainLooper())
        private val effects = ArrayList<Effect>()
        private val lights = ArrayList<Light>()
        private val sessions = HashMap<Int, Any>()
        private var order = 0
        private var hidden = false
        private val instances = ArrayList<GamepadModule>()
    }

    /** An effect an instance plays on a device. */
    private class Effect(val owner: GamepadModule, val index: Int, val device: Int, var done: ((String) -> Unit)?) {
        val timers = ArrayList<Runnable>()
        var started = false
    }

    /** An instance's colour on a device. */
    private class Light(val owner: GamepadModule, val index: Int, val device: Int, val argb: Int, val order: Int)

    private var interval: Long? = null
    private var shown = true
    private var lastValues = ""
    private val poll = object : Runnable {
        override fun run() {
            val ms = interval ?: return
            val values = context.pads.list().joinToString(";") { (i, _) -> context.pads.reading(i)?.joinToString(",") ?: "" }
            if (shown && values != lastValues) { lastValues = values; context.changed("gamepad.getGamepads") }
            main.postDelayed(this, ms)
        }
    }
    /** The pads as last told: a change raises the events for those that came and went. */
    private var told: List<Pair<Int, Int>> = emptyList()

    init {
        main.post {
            instances.add(this)
            told = context.pads.list().map { (i, d) -> i to d.id }
            context.pads.list().forEach { (_, d) -> hatch?.invoke(d.id) }
            context.pads.watch(this) { padsChanged() }
        }
    }

    private fun padsChanged() {
        val now = context.pads.list()
        val ids = now.map { (i, d) -> i to d.id }
        for ((i, id) in told.filter { it !in ids }) {
            endDevice(id)
            context.emit("gamepaddisconnected", JSONArray().put(gone(i, id)))
        }
        for ((i, d) in now.filter { (i, d) -> (i to d.id) !in told }) {
            hatch?.invoke(d.id)
            context.emit("gamepadconnected", JSONArray().put(shape(i, d, false)))
        }
        told = ids
        context.changed("gamepad.getGamepads")
    }

    /** A pad that went, as the events name it: its device can no longer be read. */
    private val names = HashMap<Int, JSONObject>()
    private fun gone(index: Int, id: Int): JSONObject = names.remove(id) ?: JSONObject().put("index", index).put("id", "").put("mapping", "standard")
        .put("family", "generic").put("labels", JSONArray(LABELS["generic"])).put("effects", JSONArray()).put("light", false)

    private fun vibrators(pad: PadDevice): Int {
        if (pad.test) return 2
        val device = pad.device ?: return 0
        return if (Build.VERSION.SDK_INT >= 31) device.vibratorManager.vibratorIds.size
            else if (@Suppress("DEPRECATION") device.vibrator.hasVibrator()) 1 else 0
    }

    private fun hasLight(pad: PadDevice): Boolean =
        pad.test || Build.VERSION.SDK_INT >= 31 && pad.device?.lightsManager?.lights?.any { it.hasRgbControl() } == true

    /** Slot [index]'s test pad, by its device id; null for a real one. */
    private fun test(id: Int): Int? = context.pads.list().firstOrNull { it.second.id == id && it.second.test }?.first

    private fun shape(index: Int, device: PadDevice, values: Boolean): JSONObject {
        val family = family(device)
        val out = JSONObject().put("index", index).put("id", device.name).put("mapping", "standard").put("family", family)
            .put("labels", JSONArray(LABELS[family])).put("effects", JSONArray(if (vibrators(device) > 0) listOf("dual-rumble") else emptyList()))
            .put("light", hasLight(device)).put("buttons", JSONObject.NULL).put("axes", JSONObject.NULL).put("timestamp", JSONObject.NULL)
            .put("apple", JSONObject.NULL).put("windows", JSONObject.NULL).put("linux", JSONObject.NULL)
            .put("android", JSONObject().put("vendorId", device.vendorId).put("productId", device.productId)
                .put("descriptor", device.descriptor).put("name", device.name))
        val r = if (values) context.pads.reading(index) else null
        if (r != null) {
            out.put("buttons", JSONArray((0 until 17).map { JSONObject().put("pressed", r[it] >= 0.5f).put("touched", r[it] > 0f).put("value", r[it].toDouble()) }))
            out.put("axes", JSONArray((17 until 21).map { r[it].toDouble() }))
            out.put("timestamp", context.now())
        }
        names[device.id] = identity(out)
        return out
    }
    /** [o] without its reading: what a disconnect names. */
    private fun identity(o: JSONObject) = JSONObject(o.toString()).put("buttons", JSONObject.NULL).put("axes", JSONObject.NULL).put("timestamp", JSONObject.NULL)

    private fun list(values: Boolean): JSONObject {
        val pads = JSONArray(context.pads.list().map { (i, d) -> shape(i, d, values) })
        return JSONObject().put("available", true).put("gamepads", pads).put("current", context.pads.current ?: JSONObject.NULL)
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        main.post {
            fun refuse(code: String) = reply(JSONObject().put("error", code))
            // A send without `index` is for the pad in use when it arrives (D2).
            val index = if (args.isNull("index")) context.pads.current else args.optDouble("index").toInt()
            val device = context.pads.list().firstOrNull { it.first == index }?.second
            when (name) {
                "getGamepads" -> {
                    val values = !args.isNull("interval")
                    if (values) { interval = maxOf(50L, args.optDouble("interval").toLong()); main.removeCallbacks(poll); main.post(poll) }
                    reply(list(values))
                }
                "playEffect" -> {
                    if (device == null || index == null) return@post refuse("NotFoundError")
                    if (args.optString("type") != "dual-rumble" || vibrators(device) == 0) return@post refuse("NotSupportedError")
                    play(index, device, args) { reply(JSONObject().put("result", it)) }
                }
                "resetEffects" -> {
                    if (device == null || index == null) return@post refuse("NotFoundError")
                    preempt(device.id)
                    reply(JSONObject().put("result", "complete"))
                }
                "setLight" -> {
                    val color = args.optJSONObject("color")
                    if (color == null && args.isNull("index")) { giveBack(null); return@post reply(JSONObject()) }
                    if (device == null || index == null) return@post refuse("NotFoundError")
                    if (!hasLight(device)) return@post refuse("NotSupportedError")
                    if (color == null) giveBack(device.id) else {
                        lights.removeAll { it.owner === this && it.device == device.id }
                        val argb = (0xff shl 24) or (color.optInt("r") shl 16) or (color.optInt("g") shl 8) or color.optInt("b")
                        lights.add(Light(this, index, device.id, argb, ++order))
                        show(device.id)
                    }
                    reply(JSONObject())
                }
                else -> refuse("NotSupportedError")
            }
        }
    }

    // Effects.

    private fun play(index: Int, device: PadDevice, args: JSONObject, done: (String) -> Unit) {
        var duration = args.optDouble("duration", 0.0).toLong()
        if (duration > LONGEST) { context.log("gamepad.playEffect: duration $duration capped at $LONGEST"); duration = LONGEST }
        val delay = maxOf(0L, args.optDouble("startDelay", 0.0).toLong())
        val strong = args.optDouble("strongMagnitude", 0.0).coerceIn(0.0, 1.0)
        val weak = args.optDouble("weakMagnitude", 0.0).coerceIn(0.0, 1.0)
        preempt(device.id)
        val effect = Effect(this, index, device.id, done)
        effects.add(effect)
        val start = Runnable {
            if (effect !in effects) return@Runnable
            effect.started = true
            vibrate(device, strong, weak, duration)
        }
        val end = Runnable { finish(effect, "complete") }
        effect.timers += start
        effect.timers += end
        main.postDelayed(start, delay)
        main.postDelayed(end, delay + duration)
    }

    /** As the other hosts' recorders write a magnitude: 1 and 0.5. */
    private fun number(x: Double) = if (x == Math.floor(x)) x.toLong().toString() else x.toString()

    private fun amplitude(m: Double) = (m * 255).toInt().coerceIn(0, 255)

    private fun vibrate(pad: PadDevice, strong: Double, weak: Double, ms: Long) {
        if (ms <= 0) return
        if (pad.test) return context.log("pad ${test(pad.id)}: rumble ${number(strong)} ${number(weak)} for $ms ms")
        val device = pad.device ?: return
        if (Build.VERSION.SDK_INT >= 31) {
            val manager = device.vibratorManager
            val ids = manager.vibratorIds
            val levels = if (ids.size >= 2) listOf(strong, weak) else listOf(maxOf(strong, weak))
            val combined = CombinedVibration.startParallel()
            for ((id, level) in ids.zip(levels)) if (level > 0) combined.addVibrator(id, VibrationEffect.createOneShot(ms, amplitude(level)))
            if (levels.any { it > 0 }) manager.vibrate(combined.combine())
        } else {
            val level = maxOf(strong, weak)
            @Suppress("DEPRECATION")
            if (level > 0) device.vibrator.vibrate(VibrationEffect.createOneShot(ms, amplitude(level)))
        }
    }

    private fun stopMotors(id: Int) {
        test(id)?.let { return context.log("pad $it: motors stopped") }
        val device = android.view.InputDevice.getDevice(id) ?: return
        if (Build.VERSION.SDK_INT >= 31) device.vibratorManager.cancel() else @Suppress("DEPRECATION") device.vibrator.cancel()
    }

    /** [effect] ends [how]: its motors stop and its sender hears it. */
    private fun finish(effect: Effect, how: String) {
        if (!effects.remove(effect)) return
        effect.timers.forEach { main.removeCallbacks(it) }
        if (effect.started) stopMotors(effect.device)
        val done = effect.done
        effect.done = null
        done?.invoke(how)
    }

    private fun preempt(device: Int) { for (e in effects.filter { it.device == device }) finish(e, "preempted") }

    /** A device went: every claim on it ends. */
    private fun endDevice(device: Int) {
        for (e in effects.filter { it.device == device }) finish(e, "preempted")
        lights.removeAll { it.device == device }
        closeSession(device)
    }

    // Lights.

    /** Device [id] shows its newest surviving claim, or the system's light. */
    private fun show(id: Int) {
        val claim = if (hidden) null else lights.filter { it.device == id }.maxByOrNull { it.order }
        test(id)?.let { slot ->
            val hex = claim?.let { "#%06x".format(it.argb and 0xffffff) }
            return context.log("pad $slot: light ${hex ?: "given back"}")
        }
        if (Build.VERSION.SDK_INT < 31) return
        if (claim == null) return closeSession(id)
        val device = android.view.InputDevice.getDevice(id) ?: return
        val manager = device.lightsManager
        val light = manager.lights.firstOrNull { it.hasRgbControl() } ?: return
        val session = sessions.getOrPut(id) { manager.openSession() } as android.hardware.lights.LightsManager.LightsSession
        val state = android.hardware.lights.LightState.Builder().setColor(claim.argb).build()
        session.requestLights(android.hardware.lights.LightsRequest.Builder().addLight(light, state).build())
    }

    private fun closeSession(id: Int) {
        if (Build.VERSION.SDK_INT < 31) return
        (sessions.remove(id) as? android.hardware.lights.LightsManager.LightsSession)?.close()
    }

    /** This instance's light on device [id], or on every device, given back. */
    private fun giveBack(id: Int?) {
        val touched = lights.filter { it.owner === this && (id == null || it.device == id) }.map { it.device }.toSet()
        lights.removeAll { it.owner === this && it.device in touched }
        touched.forEach { show(it) }
    }

    // The instance.

    override fun release(name: String) {
        if (name == "getGamepads") { interval = null; main.removeCallbacks(poll) }
    }

    /** The background gives every light back; the return writes the claims again. */
    override fun foreground(shown: Boolean) {
        this.shown = shown
        val hide = instances.none { it.shown }
        if (hide == hidden) return
        hidden = hide
        lights.map { it.device }.toSet().forEach { show(it) }
    }

    override fun destroy(done: () -> Unit) {
        main.post {
            context.pads.unwatch(this)
            main.removeCallbacks(poll)
            for (e in effects.filter { it.owner === this }) finish(e, "preempted")
            giveBack(null)
            instances.remove(this)
            done()
        }
    }
}
