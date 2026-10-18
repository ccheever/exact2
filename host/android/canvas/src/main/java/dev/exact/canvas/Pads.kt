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
 * first, whatever holds the focus, as a console's game hears its pad.
 */
class Pads(context: Context) : InputManager.InputDeviceListener {
    private val input = context.getSystemService(Context.INPUT_SERVICE) as InputManager
    /** The device id in each slot, -1 when free. */
    private val slots = IntArray(MAX) { -1 }
    /** Each slot's last reading sent: 17 buttons, then 4 axes. */
    private val sent = arrayOfNulls<FloatArray>(MAX)

    init { input.registerInputDeviceListener(this, null) }

    fun release() {
        input.unregisterInputDeviceListener(this)
        blur()
    }

    /** The app went to the background: each pad is released, and reports itself again on its next change. */
    fun blur() {
        for (slot in 0 until MAX) if (slots[slot] >= 0) { slots[slot] = -1; sent[slot] = null; send(slot, null) }
    }

    /** A gamepad button's edge; whether it was one. */
    fun key(e: KeyEvent): Boolean {
        val button = BUTTONS[e.keyCode] ?: return false
        if (!isPad(e.deviceId) || (e.action != KeyEvent.ACTION_DOWN && e.action != KeyEvent.ACTION_UP)) return false
        if (e.repeatCount > 0) return true
        val r = reading(e.deviceId) ?: return true
        r[button] = if (e.action == KeyEvent.ACTION_DOWN) 1f else 0f
        commit(e.deviceId, r)
        return true
    }

    /** A gamepad's or joystick's axes; whether they were its. */
    fun motion(e: MotionEvent): Boolean {
        if (e.source and InputDevice.SOURCE_JOYSTICK != InputDevice.SOURCE_JOYSTICK || e.action != MotionEvent.ACTION_MOVE) return false
        val r = reading(e.deviceId) ?: return true
        r[17] = quantize(e.getAxisValue(MotionEvent.AXIS_X), -1f)
        r[18] = quantize(e.getAxisValue(MotionEvent.AXIS_Y), -1f)
        r[19] = quantize(e.getAxisValue(MotionEvent.AXIS_Z), -1f)
        r[20] = quantize(e.getAxisValue(MotionEvent.AXIS_RZ), -1f)
        // Triggers: some pads report LTRIGGER/RTRIGGER, others BRAKE/GAS.
        r[6] = quantize(maxOf(e.getAxisValue(MotionEvent.AXIS_LTRIGGER), e.getAxisValue(MotionEvent.AXIS_BRAKE)), 0f)
        r[7] = quantize(maxOf(e.getAxisValue(MotionEvent.AXIS_RTRIGGER), e.getAxisValue(MotionEvent.AXIS_GAS)), 0f)
        // A hat is the d-pad.
        val hx = e.getAxisValue(MotionEvent.AXIS_HAT_X); val hy = e.getAxisValue(MotionEvent.AXIS_HAT_Y)
        if (hx != 0f || hy != 0f || HAT_SEEN.contains(e.deviceId)) {
            HAT_SEEN.add(e.deviceId)
            r[12] = if (hy < -0.5f) 1f else 0f; r[13] = if (hy > 0.5f) 1f else 0f
            r[14] = if (hx < -0.5f) 1f else 0f; r[15] = if (hx > 0.5f) 1f else 0f
        }
        commit(e.deviceId, r)
        return true
    }

    override fun onInputDeviceRemoved(deviceId: Int) {
        val slot = slots.indexOf(deviceId)
        if (slot < 0) return
        slots[slot] = -1; sent[slot] = null
        send(slot, null)
    }
    override fun onInputDeviceAdded(deviceId: Int) {}
    override fun onInputDeviceChanged(deviceId: Int) {}

    private fun isPad(id: Int): Boolean {
        val sources = InputDevice.getDevice(id)?.sources ?: return false
        return sources and InputDevice.SOURCE_GAMEPAD == InputDevice.SOURCE_GAMEPAD ||
            sources and InputDevice.SOURCE_JOYSTICK == InputDevice.SOURCE_JOYSTICK
    }

    /** A copy of the device's last reading, taking it a slot if it has none; null when every slot is taken. */
    private fun reading(id: Int): FloatArray? {
        var slot = slots.indexOf(id)
        if (slot < 0) {
            slot = slots.indexOf(-1)
            if (slot < 0) return null
            slots[slot] = id
        }
        return sent[slot]?.copyOf() ?: FloatArray(21)
    }

    private fun commit(id: Int, r: FloatArray) {
        val slot = slots.indexOf(id)
        if (sent[slot]?.contentEquals(r) == true) return
        sent[slot] = r
        send(slot, r)
    }

    private fun send(slot: Int, r: FloatArray?) = Exact.pad(slot, r != null, r ?: FloatArray(21))

    private companion object {
        const val MAX = 16
        /** Devices that reported a hat: their d-pad is the hat, not keys. */
        val HAT_SEEN = HashSet<Int>()
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
        /** Values travel in 1/256 steps: exact in a float, and a stick's noise below that is no change. */
        fun quantize(v: Float, low: Float) = (Math.round(v * 256f) / 256f).coerceIn(low, 1f) + 0f
    }
}
