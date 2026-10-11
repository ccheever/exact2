package dev.exact.canvas

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.view.View

/**
 * tag=1 (a diagnostic, with the ticker's job): every frame draws its number as 12 black-or-white squares in the
 * header, at [X_DP], [Y_DP] (each [BIT_DP] square, most significant first), and logs what the reader thought it drew
 * in that frame ([state]) as `BENCH XB <n> ...`, so a screen recording's frames can be matched to the reader's state.
 */
internal class TagView(context: Context, private val state: () -> String) : View(context) {
    private var n = 0
    private val black = Paint().apply { color = Color.BLACK }
    private val white = Paint().apply { color = Color.WHITE }
    private val d = resources.displayMetrics.density

    override fun onDraw(canvas: Canvas) {
        n = (n + 1) and 0xfff
        for (k in 0 until 12) {
            val on = (n shr (11 - k)) and 1 == 1
            canvas.drawRect(k * BIT_DP * d, 0f, (k + 1) * BIT_DP * d, BIT_DP * d, if (on) black else white)
        }
        android.util.Log.i("BENCH", "XB $n ${state()}")
        android.os.Trace.setCounter("exact tag", n.toLong())
        postInvalidateOnAnimation()
    }

    companion object {
        const val X_DP = 150f
        const val Y_DP = 8f
        const val BIT_DP = 6f
    }
}
