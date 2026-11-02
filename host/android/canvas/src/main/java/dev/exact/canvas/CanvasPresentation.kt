package dev.exact.canvas

/** Readiness of one window attachment. The retained image covers it until its first present. */
internal class CanvasPresentation {
    var ready = false
        private set
    var failed = false
    private var submitted: Long? = null

    fun attach() {
        clear()
        failed = false
    }

    fun clear() {
        ready = false
        submitted = null
    }

    fun firstFrame() {
        ready = true
        failed = false
        submitted = null
    }

    fun bufferFrame() { failed = false }

    /** -1: return to buffers; 0: keep the retained image; 1: reveal the surface.
     * Status 2 starts the deadline: a complete frame has been submitted. */
    fun observe(status: Int, now: Long): Int {
        if (status == 1) { firstFrame(); return 1 }
        if (status == 2 && submitted == null) submitted = now
        if (status < 0 || submitted?.let { now - it >= 2000 } == true) {
            failed = true
            return -1
        }
        return 0
    }

    fun canAttach(hasImage: Boolean): Boolean = hasImage && !failed
}
