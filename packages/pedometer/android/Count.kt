package exact.pedometer

/**
 * One reader group's count (D12): a Recording total plus the step counter's increase since that read, or, on the
 * sensor path, the counter's increase from a zero that moves at every return from the background and every reboot.
 * It touches no platform API, so its sums can be checked without a device.
 */
class Count(val start: Double) {
    class Answer(val steps: Double, val since: Double, val at: Double)

    companion object {
        /** How long a Recording total stands before a poll reads it again. */
        const val READ_EVERY_MS = 60_000.0
    }

    private var latest: Double? = null
    // The sensor path: its zero, when the zero was taken, and whether the next event takes a new one.
    private var zero: Double? = null
    private var since = start
    private var rezero = false
    // The Recording path: the last total, what it counted from, when it was read, and the counter then.
    private var total: Double? = null
    private var totalFrom = start
    private var totalAt = Double.NEGATIVE_INFINITY
    private var atRead: Double? = null

    /** A counter event. A fall is a reboot: both paths count on from the new value. */
    fun counter(value: Double, now: Double) {
        val last = latest
        if (last != null && value < last) {
            // The Recording path keeps what the counter added before the fall, so the count never goes back.
            val t = total
            val a = atRead
            if (t != null && a != null && last >= a) total = t + (last - a)
            zero = value
            since = now
            atRead = value
        }
        if (zero == null || rezero) {
            if (rezero) since = now
            zero = value
            rezero = false
        }
        if (atRead == null) atRead = value
        latest = value
    }

    /** The app left the foreground: no counter event arrives until the return, so neither path trusts the last one. */
    fun hidden() {
        latest = null
        atRead = null
        rezero = true
    }

    /** Whether the next answer reads the Recording API again: none yet, a stale one, or one across a gap. */
    fun due(now: Double): Boolean = total == null || now - totalAt >= READ_EVERY_MS || atRead == null

    /** The sensor path's answer. */
    fun sensor(now: Double): Answer {
        val z = zero
        val l = latest
        return Answer(if (z != null && l != null) l - z else 0.0, since, now)
    }

    /** A Recording read of [steps] from [from]: the counter counts on from its next event. */
    fun recorded(steps: Double, from: Double, now: Double): Answer {
        total = steps
        totalFrom = from
        totalAt = now
        atRead = latest
        return Answer(steps, from, now)
    }

    /** Between Recording reads: the last total and the counter's increase since. */
    fun between(now: Double): Answer {
        val a = atRead
        val l = latest
        return Answer((total ?: 0.0) + if (a != null && l != null && l >= a) l - a else 0.0, totalFrom, now)
    }
}
