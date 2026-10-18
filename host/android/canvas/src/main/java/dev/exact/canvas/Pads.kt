package dev.exact.canvas

import android.content.Context
import android.hardware.input.InputManager
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent

/**
 * Gamepads for a world canvas (LLP 1127.003 D2): a gamepad's or joystick's buttons ([KeyEvent]) and axes
 * ([MotionEvent]) in the W3C Gamepad API's standard mapping (https://w3c.github.io/gamepad/#remapping), as
 * Chrome on Android reads them, sent to exact2 ([Native.pad]) when a button or a quantized axis changed, and as
 * disconnected when Android removes the device or the activity goes to the background. A pad's index is its slot:
 * the next device takes the lowest free one, as a fresh pad. The activity hands every key and motion event here
 * first, whatever holds the focus, as a console's game hears its pad. Android has no call that reads a pad's
 * current state, so after the background a pad reports itself again with its next event: a motion event carries
 * every axis, and a button held through the background reads up until it is pressed again, as a key does.
 */
class Pads(context: Context) : InputManager.InputDeviceListener {
    private val input = context.getSystemService(Context.INPUT_SERVICE) as InputManager
    /** The device id in each slot, -1 when free. */
    private val slots = IntArray(MAX) { -1 }
    /** Each slot's reading, as last sent. */
    private val readings = arrayOfNulls<PadReading>(MAX)

    init { input.registerInputDeviceListener(this, null) }

    fun release() {
        input.unregisterInputDeviceListener(this)
        blur()
    }

    /** The app went to the background: each pad is released, and reports itself again on its next event. */
    fun blur() {
        for (slot in 0 until MAX) if (slots[slot] >= 0) { slots[slot] = -1; readings[slot] = null; send(slot, null) }
    }

    /** A gamepad button's edge; whether it was one. */
    fun key(e: KeyEvent): Boolean {
        val button = BUTTONS[e.keyCode] ?: return false
        if (!isPad(e.deviceId) || (e.action != KeyEvent.ACTION_DOWN && e.action != KeyEvent.ACTION_UP)) return false
        if (e.repeatCount > 0) return true
        val (slot, r) = reading(e.deviceId) ?: return true
        if (r.key(button, e.action == KeyEvent.ACTION_DOWN)) send(slot, r)
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
        send(slot, null)
    }
    override fun onInputDeviceAdded(deviceId: Int) {}
    override fun onInputDeviceChanged(deviceId: Int) {}

    private fun isPad(id: Int): Boolean {
        val sources = InputDevice.getDevice(id)?.sources ?: return false
        return sources and InputDevice.SOURCE_GAMEPAD == InputDevice.SOURCE_GAMEPAD ||
            sources and InputDevice.SOURCE_JOYSTICK == InputDevice.SOURCE_JOYSTICK
    }

    /** The device's slot and reading, taking it the lowest free slot if it has none; null when every slot is taken.
     * The device's motion ranges say whether its triggers are analog ([PadReading]). */
    private fun reading(id: Int): Pair<Int, PadReading>? {
        var slot = slots.indexOf(id)
        if (slot < 0) {
            slot = slots.indexOf(-1)
            if (slot < 0) return null
            slots[slot] = id
            val device = InputDevice.getDevice(id)
            val analog = listOf(MotionEvent.AXIS_LTRIGGER, MotionEvent.AXIS_BRAKE).any { device?.getMotionRange(it) != null }
            readings[slot] = PadReading(analog)
        }
        return slot to readings[slot]!!
    }

    private fun send(slot: Int, r: PadReading?) = Exact.pad(slot, r != null, r?.values?.copyOf() ?: FloatArray(21))

    private companion object {
        const val MAX = 16
        /** Android's gamepad keys by standard index, as Chromium's Android mapping reads them. */
        val BUTTONS = mapOf(
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
}

/**
 * One pad's standard-mapping reading: [values] holds 17 buttons, then 4 axes, in 1/256 steps. The triggers have
 * one owner: a pad with trigger axes ([analogTriggers]) reports their travel and ignores its L2/R2 keys; one
 * without keeps the keys' state through every motion event. The d-pad is its keys until the pad reports a hat.
 */
internal class PadReading(private val analogTriggers: Boolean) {
    val values = FloatArray(21)
    private var hat = false

    /** A button's edge; whether the reading changed. */
    fun key(button: Int, down: Boolean): Boolean {
        if ((button == 6 || button == 7) && analogTriggers) return false
        return set(button, if (down) 1f else 0f)
    }

    /** One motion event's axes; whether the reading changed. */
    fun motion(lx: Float, ly: Float, rx: Float, ry: Float, lt: Float, rt: Float, hatX: Float, hatY: Float): Boolean {
        var changed = false
        for ((i, v) in listOf(lx, ly, rx, ry).withIndex()) changed = set(17 + i, quantize(v, -1f)) || changed
        if (analogTriggers) {
            changed = set(6, quantize(lt, 0f)) || changed
            changed = set(7, quantize(rt, 0f)) || changed
        }
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

    private companion object {
        fun quantize(v: Float, low: Float) = (Math.round(v * 256f) / 256f).coerceIn(low, 1f) + 0f
    }
}
