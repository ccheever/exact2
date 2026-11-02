package dev.exact.canvas

/** The owner's latest applicable reader report; all methods run on its thread. */
internal class TimerReports {
    var reader: Any? = null
        private set
    var known = false
        private set
    var publication = 0L
        private set
    private var applied = 0L
    var scrolling = false
        private set
    var sampling = false
        private set
    var mainWork = 0
    val applying: Boolean get() = !known || applied < publication || mainWork != 0

    fun replace(view: Any) {
        reader = view; known = false; applied = 0; scrolling = false; sampling = false
    }
    fun publish(): Long = (++publication)
    fun report(view: Any, scrolling: Boolean, sampling: Boolean, applied: Long): Boolean {
        if (reader !== view) return false
        known = true; this.scrolling = scrolling; this.sampling = sampling
        this.applied = maxOf(this.applied, minOf(applied, publication))
        return true
    }
}
