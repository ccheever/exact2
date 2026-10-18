package dev.exact.canvas

import org.junit.Assert.*
import org.junit.Test

class PadReadingTest {
    /** A pad without trigger axes: its L2 key holds through motion that reports no triggers. */
    @Test fun aDigitalTriggerHoldsThroughMotion() {
        val r = PadReading.of { false }
        assertTrue(r.key(6, true))
        assertTrue(r.motion(0.5f, 0f, 0f, 0f, 0f, 0f, 0f, 0f))
        assertEquals(1f, r.values[6])
        assertEquals(0.5f, r.values[17])
        assertTrue(r.key(6, false))
        assertEquals(0f, r.values[6])
    }

    /** A pad with both trigger axes: each axis is its trigger; the keys change nothing. */
    @Test fun anAnalogTriggerFollowsItsAxis() {
        val r = PadReading.of { it == PadReading.LTRIGGER || it == PadReading.RTRIGGER }
        assertFalse(r.key(7, true))
        assertTrue(r.motion(0f, 0f, 0f, 0f, 0f, 0.3f, 0f, 0f))
        assertEquals(77f / 256f, r.values[7])
        assertTrue(r.motion(0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f))
        assertEquals(0f, r.values[7])
    }

    /** An analog left (BRAKE) and a digital right: each trigger keeps its own owner. */
    @Test fun eachTriggerHasItsOwnOwner() {
        for ((axes, analog) in listOf(setOf(PadReading.BRAKE) to (6 to 7), setOf(PadReading.GAS) to (7 to 6))) {
            val r = PadReading.of { it in axes }
            val (axisOwned, keyOwned) = analog
            assertTrue("key-owned trigger $keyOwned", r.key(keyOwned, true))
            assertFalse("axis-owned trigger $axisOwned ignores its key", r.key(axisOwned, true))
            val (lt, rt) = if (axisOwned == 6) 0.5f to 0f else 0f to 0.5f
            r.motion(0f, 0f, 0f, 0f, lt, rt, 0f, 0f)
            assertEquals(0.5f, r.values[axisOwned])
            assertEquals("the key-owned trigger holds through motion", 1f, r.values[keyOwned])
        }
    }

    /** The d-pad is keys until a hat reports; then the hat owns it. */
    @Test fun aHatOwnsTheDpadOnceItReports() {
        val r = PadReading.of { false }
        r.key(12, true)
        assertFalse(r.motion(0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f))
        assertEquals(1f, r.values[12])
        assertTrue(r.motion(0f, 0f, 0f, 0f, 0f, 0f, 1f, 0f))
        assertEquals(listOf(0f, 0f, 0f, 1f), (12..15).map { r.values[it] })
    }
}
