package exact.app

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.util.Log
import android.view.View
import dev.exact.canvas.ExactApp
import dev.exact.canvas.ExactElement
import dev.exact.canvas.ExactHatches
import dev.exact.canvas.ExactInput
import dev.exact.canvas.ExactPicker
import dev.exact.canvas.ExactTicket
import java.time.DayOfWeek

/**
 * The native-module fixture's Kotlin hatches on Android (LLP 1115.001.000 D3, D4), beside its Rust ones
 * (`modules/linux`), which hear the same moments. Each moment is logged under `ExactHatch`, with the view a node
 * gives (the `feed` field's `EditText`); the `due` hatch sets what its date and time pickers offer: its own titles,
 * no weekend day while the node says `data-weekends="off"`, and a 24-hour clock. The `spin` hatch hosts a view of
 * its own in its node's box, turns it on exact2's clock, and stops or starts it at a tap it observes.
 */
class Hatches : ExactHatches() {
    private var turning: ExactTicket? = null

    override fun app(app: ExactApp) {
        Log.i(TAG, "app ${if (app.isNew) "built" else "changed"} mood=${app.data["mood"]}")
        if (app.isNew) after(2000.0) { Log.i(TAG, "two seconds of exact2's clock") }
    }

    override fun element(element: ExactElement) {
        val moment = if (element.isNew) "built" else "changed"
        Log.i(TAG, "element ${element.hatch} #${element.node} $moment ${element.frame} ${element.words} view=${element.view?.javaClass?.simpleName}")
        if (element.hatch == "spin" && element.isNew) spin(element)
    }

    override fun elementEnded(element: ExactElement) {
        Log.i(TAG, "element ${element.hatch} #${element.node} ended")
        if (element.hatch == "spin") { turning?.stop(); turning = null }
    }

    override fun picker(element: ExactElement, picker: ExactPicker) {
        if (element.hatch != "due") return
        Log.i(TAG, "picker ${picker.kind} for #${element.node} at ${picker.value}")
        if (picker.kind == "time") {
            picker.title = "Due at"
            picker.hour24 = true
        } else {
            picker.title = "Due"
            if (element.data("weekends") == "off") picker.selectable = { it.dayOfWeek != DayOfWeek.SATURDAY && it.dayOfWeek != DayOfWeek.SUNDAY }
        }
    }

    /** The dial the reader made last, for the activity that runs now. */
    private var dial: Dial? = null

    /** A dial of the app's own in the node's box: a quarter arc that turns once a second of exact2's clock. */
    private fun spin(element: ExactElement) {
        element.host { context -> Dial(context).also { dial = it } }
        val start = { turning = frames { frame -> dial?.let { it.angle = (frame.now % 1000.0 / 1000.0 * 360.0).toFloat(); it.invalidate() } } }
        start()
        observe(element) { input ->
            if (input is ExactInput.Pointer && input.phase == "up") {
                element.diagnostics.count("taps")
                val t = turning
                if (t != null) { t.stop(); turning = null } else start()
                Log.i(TAG, "spin ${if (turning != null) "turning" else "stopped"} at (${input.localX}, ${input.localY})")
            }
        }
    }

    private class Dial(context: Context) : View(context) {
        var angle = 0f
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            color = context.getColor(android.R.color.system_accent1_600)
        }
        private val track = Paint(paint).apply { color = context.getColor(android.R.color.system_accent1_100) }

        override fun onDraw(canvas: Canvas) {
            val w = paint.strokeWidth.coerceAtLeast(width / 8f)
            paint.strokeWidth = w; track.strokeWidth = w
            val r = RectF(w, w, width - w, height - w)
            canvas.drawArc(r, 0f, 360f, false, track)
            canvas.drawArc(r, angle - 90f, 90f, false, paint)
        }
    }

    private companion object {
        const val TAG = "ExactHatch"
    }
}
