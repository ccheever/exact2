package dev.exact.canvas

import android.app.PendingIntent
import android.content.ActivityNotFoundException
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.Uri
import android.os.Build
import android.os.SystemClock
import android.util.Log
import android.view.Choreographer
import android.view.HapticFeedbackConstants

/**
 * What exact2 asks of the platform, acted on (LLP 1115.001 D5): the one channel out. exact2 queues a request and
 * wakes the reader; whoever holds the host takes the queue ([Native.requests], on exact2's thread) and hands it to
 * the main thread, where [perform] reads it. A request with an answer (the share sheet) is answered later by its id
 * ([Exact.answer]). `host/android/README.md` has the kinds and their words; `host/linux/src/presenter/requests.rs`
 * writes them.
 *
 * Everything here is the platform's own: an `Intent` for a link, `performHapticFeedback` for a haptic, the system's
 * clipboard and its Sharesheet. Nothing is imitated, and the reader has no dependency for any of it.
 */
internal class Requests(private val activity: ExactActivity) {
    /** The view a haptic is played through and whose keyboard is shown: the activity's reader, as it is made. */
    var view: ExactView? = null
    /** Called with whether system Back has somewhere to go in the app (LLP 1115.001 D6). */
    var back: (Boolean) -> Unit = {}
    /** The top layer's windows (LLP 1115.001 D10): what exact2 says of its open surface goes to them. */
    var surfaces: Surfaces? = null

    /** `x_log_requests=1` (exact2's EXACT_LOG_REQUESTS): each request as it is acted on, to the log. A haptic or a
     * clipboard write shows nowhere else. */
    private val log = android.system.Os.getenv("EXACT_LOG_REQUESTS") == "1"

    /** The share sheets that are up, by their requests' ids: none has answered yet. */
    private val sharing = HashSet<Int>()
    private val shared = activity.packageName + ".exact.SHARED"
    /** The Sharesheet tells what was chosen (a target, or from Android 15 one of its own actions) to the sender it
     * was given: a choice is a share. Registered at the first share, not at launch, which it would only slow. */
    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val id = intent.getIntExtra(ID, 0)
            if (sharing.remove(id)) Exact.answer(id, "shared")
        }
    }
    private var listening = false

    private fun listen() {
        if (listening) return
        listening = true
        if (Build.VERSION.SDK_INT >= 33) activity.registerReceiver(receiver, IntentFilter(shared), Context.RECEIVER_NOT_EXPORTED)
        else activity.registerReceiver(receiver, IntentFilter(shared))
    }

    /** The requests in [words]: for each its kind, its id, the count of words that follow, and those words. A kind
     * this reader does not know is skipped by its count. */
    fun perform(words: IntArray) {
        var i = 0
        while (i + 3 <= words.size) {
            val kind = words[i]; val id = words[i + 1]; val body = i + 3
            i = body + words[i + 2]
            if (i > words.size) return
            if (log) Log.i(TAG, "request $id kind $kind")
            try {
                when (kind) {
                    OPEN -> open(string(words, body).first)
                    HAPTIC -> haptic(words[body])
                    COPY -> copy(string(words, body).first)
                    SHARE -> share(id, words, body)
                    BACK -> back(words[body] != 0)
                    // The second word, from D8 on: the hosted field the keyboard is for (0: one exact2 paints).
                    KEYBOARD -> view?.keyboard(words[body], if (i - body > 1) words[body + 1] else 0)
                    SURFACE -> surfaces?.told(words, body)
                    // `wakelock="screen"` (LLP 1125 D11): the platform's own view attribute.
                    KEEP_SCREEN_ON -> view?.keepScreenOn = words[body] != 0
                    PICK -> pick(id, words, body)
                    HATCH -> activity.hatches()?.told(words, body, i)
                }
            } catch (e: RuntimeException) {
                // The platform refused (no activity to start from, a full clipboard transaction): the app goes on.
                Log.w(TAG, "request $id kind $kind: $e")
                if (kind == SHARE && sharing.remove(id)) Exact.answer(id, e.message ?: e.javaClass.simpleName)
            }
        }
    }

    /** The string at [at]: its bytes' count, then UTF-8 padded to whole words. And the index after it. */
    private fun string(w: IntArray, at: Int): Pair<String, Int> {
        val n = w[at]
        val bytes = ByteArray(n) { k -> (w[at + 1 + k / 4] ushr (8 * (k % 4))).toByte() }
        return String(bytes, Charsets.UTF_8) to at + 1 + (n + 3) / 4
    }

    /** The pickers that are up, by their requests' ids: none has answered yet. */
    private val picking = HashSet<Int>()

    /**
     * A date or time control's picker (LLP 1115.001.000 D1): the control's hatch may set what it offers or take it
     * over ([ExactHatches.picker]); else the activity's [ExactActivity.pickers] shows it. Its answer is the value
     * chosen, in HTML's form, or null when it was dismissed. `x_log_requests=1` logs when it first drew, from the
     * request and from the touch that asked for it.
     */
    private fun pick(id: Int, w: IntArray, at: Int) {
        val node = w[at]
        val (kind, a) = string(w, at + 1)
        val (value, b) = string(w, a)
        val (min, c) = string(w, b)
        val (max, d) = string(w, c)
        val picker = ExactPicker(kind, value, min, max, string(w, d).first)
        activity.hatches()?.picker(node, picker)
        picking.add(id)
        val answer = { chosen: String? -> if (picking.remove(id)) Exact.answer(id, chosen) }
        val asked = SystemClock.uptimeMillis()
        val touched = Exact.touched
        val drawn = {
            if (log) Choreographer.getInstance().postFrameCallback {
                val now = SystemClock.uptimeMillis()
                Log.i(TAG, "picker $kind: its first frame ${now - asked} ms after the request, ${now - touched} after the touch")
            }
        }
        val own = picker.show
        if (own != null) own(answer) else activity.pickers.show(activity, picker, drawn, answer)
    }

    /** A link that leaves the app: an `http` or `https` URL in the browser (or the app that claims it), `mailto:` in
     * a mail app's composer, `tel:` in the dialer with the number entered and not called. exact2 lets no other
     * scheme through. No Custom Tab: that is a library's. */
    private fun open(url: String) {
        val uri = Uri.parse(url)
        val intent = when (uri.scheme?.lowercase()) {
            "mailto" -> Intent(Intent.ACTION_SENDTO, uri)
            "tel" -> Intent(Intent.ACTION_DIAL, uri)
            else -> Intent(Intent.ACTION_VIEW, uri).addCategory(Intent.CATEGORY_BROWSABLE)
        }
        try { activity.startActivity(intent) } catch (e: ActivityNotFoundException) { Log.w(TAG, "no app opens $url") }
    }

    /**
     * A haptic, by exact2's number for it (`presenter::Haptic`), as the constant of Android's that fits. Android
     * names occasions, not strengths; under them are a texture tick, a tick, a click, a heavy click and a double
     * click, which is what these choose by:
     *
     * - `selection` (1): a segment's tick (Android 14), else the clock's;
     * - `impact-light` (2) a tick, `impact-medium` (3) a click, `impact-heavy` (4) a heavy click, `impact-soft` (5)
     *   the texture tick. `impact-rigid` (6) has none of its own (no click is sharper than another) and is the click;
     * - `success` (7) is confirm and `error` (9) reject. `warning` (8) has none of its own and is reject too.
     *
     * The system plays it or not by the person's touch feedback setting.
     */
    private fun haptic(kind: Int) {
        val c = when (kind) {
            1 -> if (Build.VERSION.SDK_INT >= 34) HapticFeedbackConstants.SEGMENT_TICK else HapticFeedbackConstants.CLOCK_TICK
            2 -> HapticFeedbackConstants.CONTEXT_CLICK
            3, 6 -> HapticFeedbackConstants.VIRTUAL_KEY
            4 -> HapticFeedbackConstants.LONG_PRESS
            5 -> HapticFeedbackConstants.TEXT_HANDLE_MOVE
            7 -> HapticFeedbackConstants.CONFIRM
            8, 9 -> HapticFeedbackConstants.REJECT
            else -> return
        }
        val played = view?.performHapticFeedback(c)
        if (log) Log.i(TAG, "haptic $kind: performHapticFeedback($c) = $played")
    }

    /** The clipboard. From Android 13 the system shows what was copied itself. */
    private fun copy(text: String) {
        activity.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(activity.title, text))
        if (log) Log.i(TAG, "copied ${text.length} characters")
    }

    /**
     * The Sharesheet, with the Web Share API's three members as Chrome on Android hands them over: the text and the
     * URL are the text shared, the title its subject and the sheet's own title. Its answer is `shared` when the
     * sheet says something was chosen ([receiver]) and `dismissed` when the person is back here and it said nothing
     * ([resumed]): the platform tells no more, and never that the share was finished in the app chosen.
     */
    private fun share(id: Int, w: IntArray, at: Int) {
        val given = w[at]
        val (title, afterTitle) = string(w, at + 1)
        val (text, afterText) = string(w, afterTitle)
        val url = string(w, afterText).first
        val send = Intent(Intent.ACTION_SEND).setType("text/plain")
        send.putExtra(Intent.EXTRA_TEXT, listOfNotNull(text.takeIf { given and 2 != 0 }, url.takeIf { given and 4 != 0 }).joinToString(" "))
        if (given and 1 != 0) { send.putExtra(Intent.EXTRA_SUBJECT, title); send.putExtra(Intent.EXTRA_TITLE, title) }
        // The sheet fills in what was chosen, so the intent is mutable; it names this package, so only this app gets it.
        val chosen = PendingIntent.getBroadcast(
            activity, id, Intent(shared).setPackage(activity.packageName).putExtra(ID, id),
            PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_ONE_SHOT or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        listen()
        sharing.add(id)
        activity.startActivity(Intent.createChooser(send, null, chosen.intentSender))
    }

    /** The activity is in front again. A share sheet that was up and has said nothing was dismissed: answered so a
     * moment later, since the sheet's word of a choice is a broadcast and may land just after. */
    fun resumed() {
        if (sharing.isEmpty()) return
        val waiting = sharing.toIntArray()
        activity.window.decorView.postDelayed({ for (id in waiting) if (sharing.remove(id)) Exact.answer(id, "dismissed") }, DISMISSED_MS)
    }

    /** The activity goes: no sheet of its answers any more, and each still up is cancelled. */
    fun destroy() {
        if (listening) activity.unregisterReceiver(receiver)
        for (id in sharing) Exact.answer(id, null)
        sharing.clear()
        for (id in picking) Exact.answer(id, null)
        picking.clear()
    }

    private companion object {
        const val TAG = "exact"
        const val ID = "id"
        /** The kinds (`presenter::Request`). */
        const val OPEN = 1
        const val HAPTIC = 2
        const val COPY = 3
        const val SHARE = 4
        const val BACK = 5
        const val KEYBOARD = 6
        const val SURFACE = 7
        const val KEEP_SCREEN_ON = 8
        const val PICK = 9
        const val HATCH = 10
        /** How long after the activity is in front again a share sheet's choice is still waited for. */
        const val DISMISSED_MS = 500L
    }
}
