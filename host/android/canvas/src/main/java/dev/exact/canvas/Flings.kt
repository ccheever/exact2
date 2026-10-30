package dev.exact.canvas

import android.content.Context
import android.view.GestureDetector
import android.view.MotionEvent

/** LLP 1057.001 §8 on Android: `swipe` is the platform's fling, [GestureDetector.OnGestureListener.onFling], with
 * [android.view.ViewConfiguration]'s slop and velocities, over the view's whole touch, every pointer of it. The
 * detector sees each event before exact2 does, so a fling, which fires at the lift, reaches exact2 ahead of the lift
 * and ends its contact. [kept] says who has the touch then: a touch the reader's scrolling or a hosted field took is
 * no fling, and one a field holds until exact2 answers flings when exact2 is given it ([whenGiven]). [swiped] hands
 * exact2 the direction and the most pointers the touch had down at once, and exact2 finds the node
 * (`presenter/fling.rs`). */
internal class Flings(
    context: Context,
    private val kept: () -> Kept,
    private val whenGiven: (() -> Unit) -> Unit,
    private val swiped: (String, Int) -> Unit,
) {
    private val track = FlingTouch()
    private val detector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
        override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
            val who = kept()
            val (direction, pointers) = track.fling(velocityX, velocityY, who) ?: return false
            if (who == Kept.ASKED) whenGiven { swiped(direction, pointers) } else swiped(direction, pointers)
            return true
        }
    }).apply {
        // Drop double-tap tracking so that a flick soon after a tap is a fling
        setOnDoubleTapListener(null)
    }

    /** Each event of the touch, before exact2 has it. */
    fun touch(e: MotionEvent) {
        track.event(e.actionMasked == MotionEvent.ACTION_DOWN, e.pointerCount)
        detector.onTouchEvent(e)
    }
}

/** Who has a touch at its lift, apart from exact2: the reader's scrolling, which has the rest of a touch once it
 * takes one; a hosted field that took it ([Fields]); or a field that holds it until exact2 says whether the field is
 * under it, and gives it to exact2 when not. */
internal enum class Kept { NONE, SCROLLING, FIELD, ASKED }

/** The most pointers a touch had down at once, and the fling exact2 hears. It is apart from the detector so that the
 * tests run without Android's classes. */
internal class FlingTouch {
    private var most = 0

    fun event(down: Boolean, pointers: Int) {
        if (down) most = 0
        most = maxOf(most, pointers)
    }

    /** The fling at ([velocityX], [velocityY]) px/s: its direction and the most pointers down, or null when [kept]
     * keeps it from exact2. */
    fun fling(velocityX: Float, velocityY: Float, kept: Kept): Pair<String, Int>? =
        if (kept == Kept.SCROLLING || kept == Kept.FIELD) null else direction(velocityX, velocityY) to most

    companion object {
        fun direction(velocityX: Float, velocityY: Float): String =
            if (Math.abs(velocityX) > Math.abs(velocityY)) (if (velocityX > 0) "right" else "left")
            else if (velocityY > 0) "down" else "up"
    }
}
