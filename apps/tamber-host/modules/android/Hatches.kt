package exact.app

import android.content.Context
import android.graphics.Canvas
import android.util.Log
import android.view.View
import dev.exact.canvas.ExactElement
import dev.exact.canvas.ExactHatches
import dev.exact.canvas.ExactTamber

/**
 * The Tamber binding's sample on Android (LLP 1121 D12, stage 5): the `swatch` hatch hosts a view in its node's box
 * that fills itself with the design's `swatch.fill` through `ExactTamber(view).color(…)`, inset by `swatch.inset`
 * (`length(…)`, stage 6 G), and draws again when its observation says the place's inputs moved (a `setMode`, the
 * pin above it, the scheme, the person's text size). Each colour and inset it draws is logged under `ExactTamber`
 * with its node, which the gate reads beside a screen capture.
 */
class Hatches : ExactHatches() {
    override fun element(element: ExactElement) {
        if (element.hatch == "swatch" && element.isNew) element.host { context -> Swatch(context, element.node) }
    }

    private class Swatch(context: Context, private val node: Int) : View(context) {
        private val tamber = ExactTamber(this)
        private var drawn = 0
        private var said: Int? = null

        init {
            tamber.observe { invalidate() }
        }

        private val paint = android.graphics.Paint()
        private var saidInset: Float? = null

        override fun onDraw(canvas: Canvas) {
            val fill = tamber.color("swatch.fill")
            // Points to px: the inset is in dp, as the canvas host lays out.
            val inset = tamber.length("swatch.inset") * resources.displayMetrics.density
            paint.color = fill
            canvas.drawRect(inset, inset, width - inset, height - inset, paint)
            drawn++
            if (fill != said || inset != saidInset) {
                said = fill
                saidInset = inset
                Log.i(TAG, "swatch #$node ${String.format("#%08x", fill)} inset ${inset.toInt()}px ×$drawn")
            }
        }
    }

    private companion object {
        const val TAG = "ExactTamber"
    }
}
