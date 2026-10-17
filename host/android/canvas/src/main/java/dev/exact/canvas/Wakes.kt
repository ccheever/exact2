package dev.exact.canvas

import android.os.MessageQueue
import android.os.MessageQueue.OnFileDescriptorEventListener.EVENT_ERROR
import android.os.MessageQueue.OnFileDescriptorEventListener.EVENT_INPUT
import android.os.ParcelFileDescriptor

/** The host's wake descriptors ([Native.fds]: executor replies, decoded images) watched on a looper's [queue], each
 * waking [woken]. [sync] watches them again when the host's set changes: a dev loop's plan replaces the host's
 * executor, and with it the descriptor (LLP 1122 D4), which is always a new number (the new one is made while the
 * old one is still open). A descriptor that reports an error (its far end gone) is no longer watched. */
internal class Wakes(private val queue: MessageQueue, private val woken: () -> Unit) {
    private val watched = HashMap<Int, ParcelFileDescriptor>()
    private val handler = android.os.Handler(android.os.Looper.myLooper()!!)

    /** Watch [Native.fds] as they are now: the gone ones dropped, the new ones added. */
    fun sync() {
        val now = Native.fds().filter { it >= 0 }
        for (fd in watched.keys.filter { it !in now }) drop(fd)
        for (fd in now) {
            if (fd in watched) continue
            val pfd = ParcelFileDescriptor.fromFd(fd)
            watched[fd] = pfd
            queue.addOnFileDescriptorEventListener(pfd.fileDescriptor, EVENT_INPUT or EVENT_ERROR) { _, events ->
                // Gone: unwatched by returning 0, and closed once the looper has let it go.
                if (events and EVENT_ERROR != 0) { watched.remove(fd); handler.post { pfd.close() }; 0 } else { woken(); EVENT_INPUT }
            }
        }
    }

    /** Watch none (the view leaves its window). */
    fun close() { for (fd in watched.keys.toList()) drop(fd) }

    private fun drop(fd: Int) {
        val pfd = watched.remove(fd) ?: return
        queue.removeOnFileDescriptorEventListener(pfd.fileDescriptor)
        pfd.close()
    }
}
