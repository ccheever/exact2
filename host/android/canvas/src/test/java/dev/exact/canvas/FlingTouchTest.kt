package dev.exact.canvas

import org.junit.Assert.*
import org.junit.Test

/** LLP 1057.001 §8 on Android, the reader's half: a fling goes to exact2 with its direction and the most pointers its
 * touch had down, unless the reader's scrolling or a hosted field took the touch. */
class FlingTouchTest {
    private val touch = FlingTouch()

    @Test fun aFlingGoesItsVelocitysLargerAxisAndSign() {
        assertEquals("right", FlingTouch.direction(900f, -300f))
        assertEquals("left", FlingTouch.direction(-900f, 300f))
        assertEquals("down", FlingTouch.direction(300f, 900f))
        assertEquals("up", FlingTouch.direction(-300f, -900f))
        assertEquals("a tie is vertical", "down", FlingTouch.direction(500f, 500f))
    }

    @Test fun aTouchTheScrollingOrAFieldTookFlingsNothing() {
        touch.event(down = true, pointers = 1)
        assertNull("the reader's scrolling took it", touch.fling(-900f, 0f, Kept.SCROLLING))
        assertNull("a field took it", touch.fling(-900f, 0f, Kept.FIELD))
        assertEquals("left" to 1, touch.fling(-900f, 0f, Kept.NONE))
        assertEquals("a field holds it until exact2 answers: the fling waits for exact2 to have it", "left" to 1,
            touch.fling(-900f, 0f, Kept.ASKED))
    }

    @Test fun theCountIsTheMostPointersDownAtOnceAHandOffIncluded() {
        touch.event(down = true, pointers = 1)
        touch.event(down = false, pointers = 2)
        // The first finger lifts and the second flings on: one pointer at the lift
        touch.event(down = false, pointers = 1)
        assertEquals("up" to 2, touch.fling(0f, -1200f, Kept.NONE))
        touch.event(down = true, pointers = 1)
        assertEquals("the next touch counts afresh", "down" to 1, touch.fling(0f, 1200f, Kept.NONE))
    }
}
