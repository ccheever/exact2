package dev.exact.canvas

import android.content.Context
import android.hardware.input.InputManager
import android.os.Handler
import android.os.Looper
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent

/**
 * The process's gamepads (LLP 1127.003 D2; LLP 1133.022 D4): a gamepad's or joystick's buttons ([KeyEvent]) and axes
 * ([MotionEvent]) in the W3C Gamepad API's standard mapping (https://w3c.github.io/gamepad/#remapping), as Chrome on
 * Android reads them, sent to a world canvas ([Native.pad]) when a button or a quantized axis changed, and as
 * disconnected when Android removes the device. A pad's index is its slot: a device takes the lowest free one when
 * Android adds it or at its first event, and keeps it until Android removes it.
 *
 * It is the process's, not an activity's: an activity the system makes again while the process lives finds the
 * pads, `current` and the claims a module made through them (@exact/gamepad, [ExtensionPads]) where they were. The
 * activity that reads the host hands it every key and motion event first, whatever holds the focus, as a console's
 * game hears its pad. When that activity goes to the background each pad keeps its slot and the canvas hears it
 * all released, so a world's holds drop; Android has no call that reads a pad's state, so a pad reports itself
 * again with its next event: a motion event carries every axis, and a button held through the background reads up
 * until it is pressed again, as a key does.
 *
 * **A drive's pads.** Under the agent (`EXACT_AGENT`, as the host reads it), `EXACT_AGENT_PADS` (the drive's
 * `x_agent_pads` launch extra) is a JSON list of `{"index", "id", "vendor", "product", "pressed", "leaves"}`: those pads take their
 * slots at the registry's first use, each pressing its `pressed` buttons once and disconnecting `leaves` ms later
 * when it says. They have no [InputDevice] ([PadDevice.test]); a module records their motors and light instead of
 * moving them.
 */
object Pads : InputManager.InputDeviceListener {
    private var input: InputManager? = null
    /** The device id in each slot, -1 when free. */
    private val slots = IntArray(MAX) { -1 }
    /** Each slot's reading, as last sent. */
    private val readings = arrayOfNulls<PadReading>(MAX)
    /** The slot whose button last went down, -1 for none: the pad in use (LLP 1133.022 D2). */
    var current = -1
        private set
    private val watchers = LinkedHashMap<Any, () -> Unit>()

    /** Listen for devices, once a process; the pads already connected take their slots. Main thread. */
    fun attach(context: Context): Pads {
        if (input != null) return this
        val manager = context.applicationContext.getSystemService(Context.INPUT_SERVICE) as InputManager
        input = manager
        manager.registerInputDeviceListener(this, Handler(Looper.getMainLooper()))
        for (id in manager.inputDeviceIds) if (isPad(id)) reading(id)
        inject(context.applicationContext)
        return this
    }

    /** The test pads by their device ids, which are negative: no [InputDevice] has one. */
    private val injected = HashMap<Int, PadDevice>()

    private fun inject(context: Context) {
        val text = android.system.Os.getenv("EXACT_AGENT_PADS")
        if (android.system.Os.getenv("EXACT_AGENT") != "1" || text == null) return
        val list = try { org.json.JSONArray(text) } catch (e: org.json.JSONException) { return }
        for (i in 0 until list.length()) {
            val pad = list.optJSONObject(i) ?: continue
            val id = -1000 - i
            val slot = pad.optInt("index", -1).takeIf { it in 0 until MAX && slots[it] == -1 } ?: slots.indexOf(-1)
            if (slot < 0) return
            injected[id] = PadDevice(id, pad.optString("id", "Test pad"), pad.optString("vendor", "0").toInt(16),
                pad.optString("product", "0").toInt(16), "test-$i", true)
            slots[slot] = id
            val r = PadReading(analogLeft = true, analogRight = true)
            readings[slot] = r
            val pressed = pad.optJSONArray("pressed")
            for (j in 0 until (pressed?.length() ?: 0)) r.key(pressed!!.optInt(j), true)
            send(slot, r)
            if (pressed != null && pressed.length() > 0) current = slot
            if (pad.has("leaves")) Handler(Looper.getMainLooper()).postDelayed({ onInputDeviceRemoved(id) }, pad.optLong("leaves"))
        }
    }

    /** The app went to the background (said by the activity that reads the host): each pad's canvas reading is
     * all released, and the slot kept. */
    fun background() {
        for (slot in 0 until MAX) if (slots[slot] != -1) {
            val device = InputDevice.getDevice(slots[slot])
            readings[slot] = PadReading.of { axis -> device?.getMotionRange(axis) != null }
            send(slot, readings[slot])
        }
    }

    /** A gamepad button's edge; whether it was one. */
    fun key(e: KeyEvent): Boolean {
        val button = BUTTONS[e.keyCode] ?: return false
        if (!isPad(e.deviceId) || (e.action != KeyEvent.ACTION_DOWN && e.action != KeyEvent.ACTION_UP)) return false
        if (e.repeatCount > 0) return true
        val (slot, r) = reading(e.deviceId) ?: return true
        if (r.key(button, e.action == KeyEvent.ACTION_DOWN)) send(slot, r)
        if (e.action == KeyEvent.ACTION_DOWN && current != slot) { current = slot; tell() }
        return true
    }

    /** A gamepad's or joystick's axes; whether they were its. */
    fun motion(e: MotionEvent): Boolean {
        if (e.source and InputDevice.SOURCE_JOYSTICK != InputDevice.SOURCE_JOYSTICK || e.action != MotionEvent.ACTION_MOVE) return false
        val (slot, r) = reading(e.deviceId) ?: return true
        val changed = r.motion(
            e.getAxisValue(MotionEvent.AXIS_X), e.getAxisValue(MotionEvent.AXIS_Y),
            e.getAxisValue(MotionEvent.AXIS_Z), e.getAxisValue(MotionEvent.AXIS_RZ),
            // Triggers: some pads report LTRIGGER/RTRIGGER, others BRAKE/GAS.
            maxOf(e.getAxisValue(MotionEvent.AXIS_LTRIGGER), e.getAxisValue(MotionEvent.AXIS_BRAKE)),
            maxOf(e.getAxisValue(MotionEvent.AXIS_RTRIGGER), e.getAxisValue(MotionEvent.AXIS_GAS)),
            e.getAxisValue(MotionEvent.AXIS_HAT_X), e.getAxisValue(MotionEvent.AXIS_HAT_Y),
        )
        if (changed) send(slot, r)
        return true
    }

    override fun onInputDeviceRemoved(deviceId: Int) {
        val slot = slots.indexOf(deviceId)
        if (slot < 0) return
        slots[slot] = -1; readings[slot] = null
        injected.remove(deviceId)
        if (current == slot) current = -1
        send(slot, null)
        tell()
    }
    override fun onInputDeviceAdded(deviceId: Int) { if (isPad(deviceId)) reading(deviceId) }
    override fun onInputDeviceChanged(deviceId: Int) {}

    // What a module reads ([ExtensionPads]).

    /** The connected pads by slot. */
    fun list(): List<Pair<Int, PadDevice>> = (0 until MAX).mapNotNull { slot ->
        val id = slots[slot].takeIf { it != -1 } ?: return@mapNotNull null
        (injected[id] ?: InputDevice.getDevice(id)?.let(PadDevice::of))?.let { slot to it }
    }

    /** Slot [slot]'s latest reading: 17 buttons, then 4 axes; null when it is free. */
    fun values(slot: Int): FloatArray? = readings.getOrNull(slot)?.values?.copyOf()

    /** [f] is told each change, on the main thread: a pad came or went, or another is in use. */
    fun watch(owner: Any, f: () -> Unit) { watchers[owner] = f }
    fun unwatch(owner: Any) { watchers.remove(owner) }
    private fun tell() { for (f in watchers.values.toList()) f() }

    private fun isPad(id: Int): Boolean {
        val sources = InputDevice.getDevice(id)?.sources ?: return false
        return sources and InputDevice.SOURCE_GAMEPAD == InputDevice.SOURCE_GAMEPAD ||
            sources and InputDevice.SOURCE_JOYSTICK == InputDevice.SOURCE_JOYSTICK
    }

    /** The device's slot and reading, taking it the lowest free slot if it has none; null when every slot is taken.
     * The device's motion ranges say which of its triggers are analog ([PadReading.of]). */
    private fun reading(id: Int): Pair<Int, PadReading>? {
        var slot = slots.indexOf(id)
        if (slot < 0) {
            slot = slots.indexOf(-1)
            if (slot < 0) return null
            slots[slot] = id
            val device = InputDevice.getDevice(id)
            readings[slot] = PadReading.of { axis -> device?.getMotionRange(axis) != null }
            tell()
        }
        return slot to readings[slot]!!
    }

    private fun send(slot: Int, r: PadReading?) = Exact.pad(slot, r != null, r?.values?.copyOf() ?: FloatArray(21))

    private const val MAX = 16
    /** Android's gamepad keys by standard index, as Chromium's Android mapping reads them. */
    private val BUTTONS = mapOf(
        KeyEvent.KEYCODE_BUTTON_A to 0, KeyEvent.KEYCODE_BUTTON_B to 1,
        KeyEvent.KEYCODE_BUTTON_X to 2, KeyEvent.KEYCODE_BUTTON_Y to 3,
        KeyEvent.KEYCODE_BUTTON_L1 to 4, KeyEvent.KEYCODE_BUTTON_R1 to 5,
        KeyEvent.KEYCODE_BUTTON_L2 to 6, KeyEvent.KEYCODE_BUTTON_R2 to 7,
        KeyEvent.KEYCODE_BUTTON_SELECT to 8, KeyEvent.KEYCODE_BUTTON_START to 9,
        KeyEvent.KEYCODE_BUTTON_THUMBL to 10, KeyEvent.KEYCODE_BUTTON_THUMBR to 11,
        KeyEvent.KEYCODE_DPAD_UP to 12, KeyEvent.KEYCODE_DPAD_DOWN to 13,
        KeyEvent.KEYCODE_DPAD_LEFT to 14, KeyEvent.KEYCODE_DPAD_RIGHT to 15,
        KeyEvent.KEYCODE_BUTTON_MODE to 16,
    )
}

/** A pad's identity as a module reads it: its [InputDevice]'s, or a drive's pad's ([test]). */
class PadDevice(val id: Int, val name: String, val vendorId: Int, val productId: Int, val descriptor: String, val test: Boolean) {
    /** The device, which a test pad has none of. */
    val device: InputDevice? get() = if (test) null else InputDevice.getDevice(id)

    companion object {
        fun of(d: InputDevice) = PadDevice(d.id, d.name, d.vendorId, d.productId, d.descriptor, false)
    }
}

/**
 * One pad's standard-mapping reading: [values] holds 17 buttons, then 4 axes, in 1/256 steps. Each trigger has one
 * owner: one with an axis ([analogLeft], [analogRight]) reports its travel and ignores its key; one without keeps its
 * key's state through every motion event. The d-pad is its keys until the pad reports a hat.
 */
internal class PadReading(private val analogLeft: Boolean, private val analogRight: Boolean) {
    val values = FloatArray(21)
    private var hat = false

    /** A button's edge; whether the reading changed. */
    fun key(button: Int, down: Boolean): Boolean {
        if ((button == 6 && analogLeft) || (button == 7 && analogRight)) return false
        return set(button, if (down) 1f else 0f)
    }

    /** One motion event's axes; whether the reading changed. */
    fun motion(lx: Float, ly: Float, rx: Float, ry: Float, lt: Float, rt: Float, hatX: Float, hatY: Float): Boolean {
        var changed = false
        for ((i, v) in listOf(lx, ly, rx, ry).withIndex()) changed = set(17 + i, quantize(v, -1f)) || changed
        if (analogLeft) changed = set(6, quantize(lt, 0f)) || changed
        if (analogRight) changed = set(7, quantize(rt, 0f)) || changed
        if (hatX != 0f || hatY != 0f) hat = true
        if (hat) {
            changed = set(12, if (hatY < -0.5f) 1f else 0f) || changed
            changed = set(13, if (hatY > 0.5f) 1f else 0f) || changed
            changed = set(14, if (hatX < -0.5f) 1f else 0f) || changed
            changed = set(15, if (hatX > 0.5f) 1f else 0f) || changed
        }
        return changed
    }

    private fun set(i: Int, v: Float): Boolean {
        if (values[i] == v) return false
        values[i] = v
        return true
    }

    companion object {
        const val LTRIGGER = MotionEvent.AXIS_LTRIGGER
        const val RTRIGGER = MotionEvent.AXIS_RTRIGGER
        const val GAS = MotionEvent.AXIS_GAS
        const val BRAKE = MotionEvent.AXIS_BRAKE

        /** A reading for a device with these axes: the left trigger is analog with LTRIGGER or BRAKE, the right with
         * RTRIGGER or GAS, each on its own. */
        fun of(hasAxis: (Int) -> Boolean) = PadReading(hasAxis(LTRIGGER) || hasAxis(BRAKE), hasAxis(RTRIGGER) || hasAxis(GAS))

        private fun quantize(v: Float, low: Float) = (Math.round(v * 256f) / 256f).coerceIn(low, 1f) + 0f
    }
}
