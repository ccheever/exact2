package dev.exact.canvas

import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.RenderNode
import android.graphics.Typeface
import android.os.Looper
import android.text.InputFilter
import android.text.InputType
import android.text.TextWatcher
import android.util.Log
import android.util.TypedValue
import android.view.Choreographer
import android.view.ContextThemeWrapper
import android.view.Gravity
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * The framework's own [EditText], as a hosted field uses it (LLP 1115.001 D8): nothing of it is replaced, and it
 * says when its selection moves, which no listener of the class tells.
 */
internal class FieldText(context: Context) : EditText(unadjusted(context)) {
    var selected: ((Int, Int) -> Unit)? = null
    /** `x_log_fields=1`: what the keyboard asks of the field's input connection, to the log. Its composition is
     * between it and the field, and shows nowhere else. */
    var heard: ((String) -> Unit)? = null
    /** The keyboard changed the field through its input connection. Its composition may have begun or ended with no
     * change of text or selection (the keyboard marking a word the caret entered, or finishing what is marked as it
     * stands), which no listener of the class tells. */
    var asked: (() -> Unit)? = null
    /** Where the field stands in the accessibility tree the reader answers with ([Access], LLP 1115.001 D13), and
     * what its author named it; null while that tree does not have it. The field reports itself, as any `EditText`
     * does: this only says where, and by what name. */
    var stand: Access.Stand? = null

    override fun onInitializeAccessibilityNodeInfo(info: android.view.accessibility.AccessibilityNodeInfo) {
        super.onInitializeAccessibilityNodeInfo(info)
        stand?.place(info)
    }

    /** Bold Text's step is in the weight the field is given ([unadjusted]): a change of the device's font weight
     * adjustment reaches the kernel, never the field's own typeface. */
    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(if (android.os.Build.VERSION.SDK_INT >= 31) Configuration(newConfig).apply { fontWeightAdjustment = 0 } else newConfig)
    }

    override fun onSelectionChanged(selStart: Int, selEnd: Int) {
        super.onSelectionChanged(selStart, selEnd)
        selected?.invoke(selStart, selEnd)
    }

    override fun onCreateInputConnection(info: EditorInfo): InputConnection? {
        val own = super.onCreateInputConnection(info) ?: return null
        val say = { what: String -> heard?.invoke(what) }
        val done = { did: Boolean -> asked?.invoke(); did }
        return object : InputConnectionWrapper(own, false) {
            override fun setComposingText(text: CharSequence?, at: Int): Boolean { say("setComposingText \"$text\""); return done(super.setComposingText(text, at)) }
            override fun setComposingRegion(start: Int, end: Int): Boolean { say("setComposingRegion $start..$end"); return done(super.setComposingRegion(start, end)) }
            override fun commitText(text: CharSequence?, at: Int): Boolean { say("commitText \"$text\""); return done(super.commitText(text, at)) }
            override fun finishComposingText(): Boolean { say("finishComposingText"); return done(super.finishComposingText()) }
            override fun deleteSurroundingText(before: Int, after: Int): Boolean { say("deleteSurroundingText $before, $after"); return done(super.deleteSurroundingText(before, after)) }
            override fun deleteSurroundingTextInCodePoints(before: Int, after: Int): Boolean { say("deleteSurroundingTextInCodePoints $before, $after"); return done(super.deleteSurroundingTextInCodePoints(before, after)) }
        }
    }
}

/**
 * The app's text fields (LLP 1115.001 D8). Each `input` of a text type and each `textarea` is the framework's own
 * [EditText], with no background of its own, over the container exact2 paints for it. The IME and its composition,
 * autofill, the selection handles, the magnifier, the floating toolbar and the spell checker are the platform's,
 * because the field is. `host/linux/src/presenter/hosted.rs` is exact2's side and has the words that cross.
 *
 * **Where a field is.** exact2 lays it out and says, with each stream, where its content box was painted, under
 * which clip, and among which scroller's rows ([told], [Native.fields]). The view is put there for real (its
 * handles, its magnifier and the keyboard's cursor anchor are placed by the view's own position), inside a [Port]
 * the size of its clip, and it moves with its scroller as the reader moves the scroller's rows ([place]).
 *
 * **How it is drawn.** Not by the view system: this layout draws nothing, as [Natives] draws none of its views.
 * Each field's own drawing is recorded into a [RenderNode] that the stream draws in the field's place (op 28, kind
 * 4), so the field stacks in tree order with what exact2 paints and is clipped as its box is. A field that
 * invalidates (a caret that blinks, a character typed) has only its node recorded again.
 *
 * **Whose a touch is.** The reader's view takes every touch ([ExactView.onTouchEvent]). One that lands in a field's
 * box is held until exact2 says what is under the point ([Native.under]): if it is the field, the touch is the
 * field's from its down on ([touch], [under]) and exact2 hears none of it, only the focus the field then takes. A
 * drag that begins on a field still scrolls the list around it ([Scrolling] cancels the touch in the field), unless
 * the field asked to keep it ([keeps]: a selection or a caret being dragged), as in a ScrollView.
 *
 * **Whose the text is.** The field's, while the person edits it. Every edit is counted and told ([flush],
 * [Native.fieldEdited]); exact2 says back the count it has reached beside the value the app shows. A value told is
 * taken only when it answers the newest edit and differs from what the field holds: then the text is replaced, in
 * place, and the keyboard starts over. A value that is the field's own changes nothing, so a composition stands.
 *
 * **The app's value never ends a composition** (LLP 1115.000 §5, ruling 6; LLP 1115.001 D8 as corrected). The field
 * says when its text begins to compose and when that ends ([flush], [Native.fieldComposing]), before the edit that
 * began or ended it. While it composes exact2 holds a value the app writes and tells the field its own text, so
 * nothing here is replaced; when the composition ends, committed or cancelled, exact2 tells the value it held, the
 * last one, after the app has heard the text. The action key commits what is composing first, as a person means by
 * it. Not seen on a phone (`QUEUE.md`): the rule is held at exact2's side (`presenter/hosted_tests.rs`).
 */
internal class Fields(context: Context) : FrameLayout(context) {
    /** What a field shows through: its clip, in the reader's view. A drawing-less layout the size of the clip, so
     * that the platform, which hides a selection handle outside the field's ancestors, hides one the clip cuts. */
    class Port(context: Context) : FrameLayout(context) {
        init { clipChildren = false; setWillNotDraw(true) }
        override fun dispatchDraw(canvas: Canvas) {}
    }

    /** One hosted field: exact2's node [id], its view, and the node its drawing is recorded into. */
    class Field(val id: Int, val port: Port, val text: FieldText, val node: RenderNode) {
        var generation = 0
        /** Bit 0: it was painted and has a place; bit 1: it takes no touch and no focus. */
        var flags = 0
        /** The scroller whose rows it was painted among (0: none): its view moves as they do. */
        var group = 0
        /** Its content box where the last stream painted it, and its clip there (a negative width: none). */
        var x = 0f; var y = 0f; var w = 0; var h = 0
        var clipX = 0f; var clipY = 0f; var clipW = -1f; var clipH = -1f
        /** Where its view is now, in the reader's view: [x], [y] moved as its scroller's rows are. */
        var atX = 0f; var atY = 0f
        /** Its look as exact2 last told it: the words, compared whole. */
        var look: IntArray? = null
        /** The count of the person's edits here, and the last told to exact2. */
        var edits = 0
        var edited = false
        var moved = false
        /** Whether its text composes, as exact2 was last told. */
        var composing = false
        /** The revision of the last selection a command set, as applied. */
        var select = 0
        /** What the action key says it does ([Native.fieldAction]'s numbers), and whether Enter breaks a line. */
        var action = 0
        var multiline = false
        /** The colours of its caret and of its handles, and those its drawables were last given. */
        var caret = 0
        var accent = 0
        var tinted = 0L
        var dirty = true
    }

    /** The view that reads the host now: the one whose stream draws the fields and whose scrollers move them. */
    var view: ExactView? = null
    /** Asked to draw a frame after a field's node was recorded again (no new stream). */
    var frame: () -> Unit = {}

    private val fields = HashMap<Int, Field>()
    private val byView = HashMap<View, Field>()
    private val imm = context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    /** The reader is changing a field itself (a value exact2 told, a focus it asked for): nothing to tell back. */
    private var quiet = false
    /** The field that holds the focus, and the one exact2 asked the focus for before it was told of it. */
    private var focused: Field? = null
    private var wanted = 0
    private var scheduled = false
    private var flushing = false
    /** `x_log_fields=1` (exact2's EXACT_LOG_FIELDS): each edit told to exact2, with the composing region, to the log. */
    private val log = android.system.Os.getenv("EXACT_LOG_FIELDS") == "1"

    init {
        clipChildren = false
        // Laid out, attached and focusable, but drawn only through their nodes.
        setWillNotDraw(true)
    }

    override fun dispatchDraw(canvas: Canvas) {}

    override fun onDescendantInvalidated(child: View, target: View) {
        val f = byView[target] ?: return
        f.dirty = true
        if (scheduled) return
        scheduled = true
        Choreographer.getInstance().postFrameCallback {
            scheduled = false
            if (refresh()) frame()
        }
    }

    /** Record every changed field's drawing again; whether any was. */
    private fun refresh(): Boolean {
        var any = false
        for (f in fields.values) if (f.dirty) { record(f); any = true }
        return any
    }

    /** [f]'s own drawing into its node. The view system applies a view's scroll before it draws it; drawn by hand,
     * the field's (a long line scrolled along, a textarea scrolled down) is applied here. */
    private fun record(f: Field) {
        f.dirty = false
        if (f.w <= 0 || f.h <= 0) return
        f.text.computeScroll()
        f.node.setPosition(0, 0, f.w, f.h)
        val c = f.node.beginRecording(f.w, f.h)
        c.translate(-f.text.scrollX.toFloat(), -f.text.scrollY.toFloat())
        f.text.draw(c)
        f.node.endRecording()
    }

    /** The node op 28 draws for field [id] (main thread). */
    fun node(id: Int): RenderNode? = fields[id]?.node

    /** Field [id]'s own view, where the accessibility tree has the field ([Access], LLP 1115.001 D13). */
    fun text(id: Int): FieldText? = fields[id]?.text

    /** A stream draws field [id] (op 28, seen while the stream is ingested, before anything of it is drawn). Android
     * frees the drawing of a node that left the render tree, as a field's does when a paint leaves its row out (it
     * was out of its scroller's port): drawn again, the field is recorded again. */
    fun drawn(id: Int) {
        val f = fields[id] ?: return
        if (f.dirty || !f.node.hasDisplayList()) record(f)
    }

    /** Every field's node, for a row recorded on another thread: one map shared until a field comes or goes. */
    private var nodes: Map<Int, RenderNode>? = null
    fun snapshot(): Map<Int, RenderNode> = nodes ?: fields.mapValues { it.value.node }.also { nodes = it }

    /** The fields leave the window: their reader is being made again (the activity starts after a stop), or their
     * activity goes. A field that loses the focus that way has not lost it in the app: exact2 is told nothing, and
     * asks the reader that comes for the focus and the keyboard again ([focus]). */
    fun leaving() { away = true }
    private var away = false

    /** The reader was made again: its stream draws these nodes anew, and Android has discarded what they held. */
    fun adopted(by: ExactView) {
        view = by
        away = false
        for (f in fields.values) f.dirty = true
    }

    // What exact2 says.

    /**
     * exact2's statement of its fields ([Native.fields]), taken before the stream it came with is drawn: a count, then
     * for each field its id, generation, flags and scroller; its box and its clip as float bits; the edit it answers
     * and the value as of it; its look (eighteen words) and three strings. A field it leaves out is gone.
     */
    fun told(w: IntArray) {
        if (w.isEmpty()) return
        val began = if (log) System.nanoTime() else 0L
        val had = fields.size
        val seen = HashSet<Int>(w[0])
        var i = 1
        while (i + HEAD <= w.size) {
            val id = w[i]; val generation = w[i + 1]
            val (value, look) = string(w, i + HEAD)
            val (placeholder, afterHint) = string(w, look + LOOK)
            val (autocomplete, afterFill) = string(w, afterHint)
            val (family, end) = string(w, afterFill)
            seen.add(id)
            var f = fields[id]
            // A view made for a field exact2 already knows (the activity was made again) starts at exact2's count
            // of its edits, and so takes the value told with it.
            if (f == null) f = make(id, generation).also { it.edits = w[i + 12] }
            // Its node is another row's item now (LLP 1078): the view starts over, with nothing of the last one's.
            else if (f.generation != generation) renew(f, generation)
            val placed = w[i + 2] and PLACED != 0
            val shown = placed && f.flags and PLACED == 0
            f.flags = w[i + 2]; f.group = w[i + 3]
            // A field with no place (its row was not painted) has no box to tell: it keeps the size it had.
            val nw = if (placed) max(0, fl(w[i + 6]).roundToInt()) else f.w
            val nh = if (placed) max(0, fl(w[i + 7]).roundToInt()) else f.h
            val sized = nw != f.w || nh != f.h
            if (placed) { f.x = fl(w[i + 4]); f.y = fl(w[i + 5]) }
            f.w = nw; f.h = nh
            f.clipX = fl(w[i + 8]); f.clipY = fl(w[i + 9]); f.clipW = fl(w[i + 10]); f.clipH = fl(w[i + 11])
            val words = w.copyOfRange(look, end)
            if (!words.contentEquals(f.look)) {
                f.look = words
                // Setting an input type sets the text again: no edit of the person's.
                quiet = true
                try { style(f, w, look, placeholder, autocomplete, family) } finally { quiet = false }
            }
            if (sized) size(f) else if (shown) f.dirty = true
            position(f)
            // The value answers the field's newest edit, or it is older than what the person has typed since.
            if (w[i + 12] == f.edits && value != f.text.text.toString()) replace(f, value)
            val rev = w[look + 15]
            if (rev != f.select) { f.select = rev; select(f, w[look + 16], w[look + 17]) }
            if (f.dirty) record(f)
            i = end
        }
        if (fields.size > seen.size) for (f in fields.values.filter { !seen.contains(it.id) }) release(f)
        if (wanted != 0) fields[wanted]?.let { focus(it.id) }
        if (log && fields.size != had) Log.i(TAG, "fields: ${w[0]} told, $had held before, ${(System.nanoTime() - began) / 1000} us")
    }

    private fun make(id: Int, generation: Int): Field {
        val text = bare(FieldText(context))
        if (log) text.heard = { Log.i(TAG, "field $id keyboard: $it") }
        val port = Port(context)
        port.addView(text, LayoutParams(1, 1))
        addView(port, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT))
        val f = Field(id, port, text, RenderNode("exact field"))
        f.generation = generation
        fields[id] = f
        byView[text] = f
        nodes = null
        listen(f)
        return f
    }

    /** What the field tells: its edits and its selection (as they settle, [flush]), the focus it takes or loses, its
     * action key and a hardware keyboard's Enter and Tab. */
    private fun listen(f: Field) {
        val text = f.text
        text.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: android.text.Editable?) {
                if (quiet) return
                // Counted now: a value exact2 tells for an older count is not for this text.
                f.edits++
                f.edited = true
                later()
            }
        })
        text.selected = { _, _ -> if (!quiet) { f.moved = true; later() } }
        // Any IME call is real input (a backspace in an empty field included, which changes nothing it reports):
        // a held state lets go (LLP 1122 D8).
        text.asked = { Exact.fieldInput(f.id, f.generation); later() }
        text.setOnFocusChangeListener { _, has ->
            if (has) {
                focused = f
                tint(f)
                if (!quiet && !away) Exact.fieldFocused(f.id, f.generation, true)
            } else if (focused === f) {
                focused = null
                // Told once the turn is over, and only when no other field took the focus: a tap from one field to
                // the next is one move of the focus to exact2, not a loss and then a gain (which would put the
                // keyboard away between them).
                if (!quiet && !away) post { if (focused == null && !away && fields[f.id] === f) Exact.fieldFocused(f.id, f.generation, false) }
            }
        }
        text.setOnEditorActionListener { _, id, event ->
            // A textarea's Enter breaks its line, which is the field's own to do.
            if (f.multiline) return@setOnEditorActionListener false
            // A hardware keyboard's Enter arrives down and then up: the down acts.
            if (event != null && event.action != KeyEvent.ACTION_DOWN) return@setOnEditorActionListener true
            val action = when (id) {
                EditorInfo.IME_ACTION_DONE -> DONE; EditorInfo.IME_ACTION_GO -> GO; EditorInfo.IME_ACTION_NEXT -> NEXT
                EditorInfo.IME_ACTION_PREVIOUS -> PREVIOUS; EditorInfo.IME_ACTION_SEARCH -> SEARCH
                EditorInfo.IME_ACTION_SEND -> SEND
                // Enter with no action named (a hardware keyboard's): what the field's own key does.
                else -> f.action
            }
            // The key commits what is composing, so a value the app writes for it (a field cleared as it submits)
            // is not held behind a word the keyboard still has marked.
            if (BaseInputConnection.getComposingSpanStart(text.editableText) >= 0) {
                BaseInputConnection.removeComposingSpans(text.editableText)
                imm.restartInput(text)
            }
            flush()
            Exact.fieldAction(f.id, f.generation, action)
            true
        }
        text.setOnKeyListener { _, code, event ->
            // Any key down is real input, which lets a held state go (LLP 1122 D8).
            if (event.action == KeyEvent.ACTION_DOWN) Exact.fieldInput(f.id, f.generation)
            if (code != KeyEvent.KEYCODE_TAB || event.isCtrlPressed || event.isAltPressed || event.isMetaPressed) return@setOnKeyListener false
            // Tab is the page's: exact2 moves the focus through its own order, which knows the painted controls.
            if (event.action == KeyEvent.ACTION_DOWN) {
                flush()
                Exact.fieldAction(f.id, f.generation, if (event.isShiftPressed) BACKTAB else TAB)
            }
            true
        }
    }

    /** What the fields changed is told when this turn of the loop is over: a keyboard changes the text and then the
     * selection, in one batch, and exact2 hears the two as one edit. */
    private fun later() {
        if (flushing) return
        flushing = true
        post { flush() }
    }

    /** Tell exact2 each field's edit, or its selection where only that moved, as they stand now; and first, that
     * its text began to compose or stopped, so that exact2 holds the app's value while it composes and tells it
     * once that ends (an edit that ended it is heard by the app before). */
    private fun flush() {
        flushing = false
        for (f in fields.values) {
            val e = f.text.text
            val composing = BaseInputConnection.getComposingSpanStart(e) >= 0
            if (composing != f.composing) {
                f.composing = composing
                Exact.fieldComposing(f.id, f.generation, composing, e.toString())
            }
            if (!f.edited && !f.moved) continue
            val start = max(0, f.text.selectionStart); val end = max(start, f.text.selectionEnd)
            if (f.edited) {
                if (log) Log.i(TAG, "field ${f.id} edit ${f.edits}: ${e.length} units, selection $start..$end, composing " +
                    "${BaseInputConnection.getComposingSpanStart(e)}..${BaseInputConnection.getComposingSpanEnd(e)}")
                Exact.fieldEdited(f.id, f.generation, f.edits, e.toString(), start, end)
            } else Exact.fieldSelected(f.id, f.generation, start, end)
            f.edited = false; f.moved = false
        }
    }

    /** A value of the app's that is not the field's: the text is replaced where it stands, so the field keeps its
     * undo, and the caret stays where it was among the text after it, or else as far from the start. The keyboard
     * starts over: a composition does not outlive the text it was composing. exact2 tells no such value while the
     * field composes (it holds the app's until the composition ends), so this ends one only where a value crossed
     * the field's word that it had begun. The field's filters are the person's
     * limits, not the app's: its `maxlength`, and the characters its keyboard's kind admits (a number field's key
     * listener drops a "." the app sets, and the caret was then put past the end of what was left: a crash, seen on
     * a Pixel with Shelf's own test of its pages field). */
    private fun replace(f: Field, value: String) {
        val e = f.text.editableText ?: return
        val old = e.toString()
        val start = f.text.selectionStart; val end = f.text.selectionEnd
        val at = { o: Int ->
            if (o < 0) value.length
            else if (o <= old.length && value.endsWith(old.substring(o))) value.length - (old.length - o)
            else min(o, value.length)
        }
        quiet = true
        try {
            BaseInputConnection.removeComposingSpans(e)
            val filters = e.filters
            e.filters = arrayOf()
            e.replace(0, e.length, value)
            e.filters = filters
            val n = e.length
            val a = min(at(start), n); val b = min(at(end), n)
            f.text.setSelection(min(a, b), max(a, b))
        } finally { quiet = false }
        if (f.text.isFocused) imm.restartInput(f.text)
        // Where the caret is now, for the app's next `input`.
        f.moved = true
        later()
    }

    /** A selection a command of the app's set (`setSelectionRange`, `select()`). */
    private fun select(f: Field, start: Int, end: Int) {
        val n = f.text.length()
        quiet = true
        try { f.text.setSelection(start.coerceIn(0, n), end.coerceIn(0, n)) } finally { quiet = false }
    }

    private fun renew(f: Field, generation: Int) {
        f.generation = generation
        f.edits = 0; f.edited = false; f.moved = false; f.select = 0; f.composing = false
        quiet = true
        // A new text and a new history: nothing of the last item's is undone into this one.
        try { f.text.setText("") } finally { quiet = false }
    }

    private fun release(f: Field) {
        fields.remove(f.id)
        byView.remove(f.text)
        nodes = null
        // A field that goes while it holds the focus (its page closed) hands the focus to the reader's own view
        // first. Removed with it, the platform gives the focus to the first field left in the window, which told
        // exact2 as a person's tap would: the page beneath came back with its search field focused and the keyboard
        // up (seen on a Pixel with Shelf: Save in the add form).
        if (f.text.isFocused) { yielded(f); hide() }
        if (focused === f) focused = null
        removeView(f.port)
        f.node.discardDisplayList()
    }

    /** The field's look and its keyboard, from the look's words ([Native.fields]) and its three strings. */
    private fun style(f: Field, w: IntArray, at: Int, placeholder: String, autocomplete: String, family: String) {
        val what = w[at]; val keyboard = w[at + 1]; val enter = w[at + 3]
        val text = f.text
        f.multiline = what and MULTILINE != 0
        val type = inputType(what, keyboard, w[at + 2])
        if (text.inputType != type) text.inputType = type
        // Its type after its input type, which sets a password in a monospace face.
        face(text, fl(w[at + 11]), w[at + 12], what and ITALIC != 0, fl(w[at + 13]), fl(w[at + 14]), family)
        f.action = when {
            f.multiline -> 0
            enter in DONE..SEND -> enter
            enter == 1 -> 0
            // Unsaid: a search field searches and a field that submits goes; else the platform's own default for a
            // field, Next where another field follows it and Done where none does.
            keyboard == SEARCHES -> SEARCH
            what and SUBMITS != 0 -> GO
            what and FOLLOWED != 0 -> NEXT
            else -> DONE
        }
        val options = when (f.action) {
            DONE -> EditorInfo.IME_ACTION_DONE; GO -> EditorInfo.IME_ACTION_GO; NEXT -> EditorInfo.IME_ACTION_NEXT
            PREVIOUS -> EditorInfo.IME_ACTION_PREVIOUS; SEARCH -> EditorInfo.IME_ACTION_SEARCH
            SEND -> EditorInfo.IME_ACTION_SEND
            else -> EditorInfo.IME_ACTION_NONE
        }
        if (text.imeOptions != options) { text.imeOptions = options; if (text.isFocused) imm.restartInput(text) }
        val limit = w[at + 4]
        text.filters = if (limit >= 0) arrayOf<InputFilter>(InputFilter.LengthFilter(limit)) else arrayOf()
        text.gravity = (if (f.multiline) Gravity.TOP else Gravity.CENTER_VERTICAL) or
            when (w[at + 5]) { 1 -> Gravity.CENTER_HORIZONTAL; 2 -> Gravity.END; else -> Gravity.START }
        text.setTextColor(w[at + 6])
        text.setHintTextColor(w[at + 7])
        text.highlightColor = w[at + 10]
        f.caret = w[at + 8]; f.accent = w[at + 9]
        if (text.isFocused) tint(f)
        if (text.hint?.toString() != placeholder) text.hint = placeholder
        val off = what and DISABLED != 0
        if (text.isEnabled == off) text.isEnabled = !off
        // Read-only: it shows and selects, and takes no key and no keyboard.
        val fixed = what and READONLY != 0
        if (fixed) text.keyListener = null
        text.showSoftInputOnFocus = !fixed && keyboard != NO_KEYBOARD
        val hints = hints(autocomplete)
        text.importantForAutofill = if (autocomplete == "off") View.IMPORTANT_FOR_AUTOFILL_NO else View.IMPORTANT_FOR_AUTOFILL_AUTO
        text.setAutofillHints(*hints)
        f.dirty = true
    }

    /** The caret and the handles are the theme's own drawables, in the colours exact2 resolved. Given when the field
     * takes the focus, which is when it first shows either: four drawables a field are not loaded for a form's
     * every field as it opens. */
    private fun tint(f: Field) {
        val now = (f.caret.toLong() shl 32) or (f.accent.toLong() and 0xffffffffL)
        if (f.tinted == now + 1) return
        f.tinted = now + 1
        val text = f.text
        text.textCursorDrawable?.mutate()?.let { it.setTint(f.caret); text.textCursorDrawable = it }
        text.textSelectHandle?.mutate()?.let { it.setTint(f.accent); text.setTextSelectHandle(it) }
        text.textSelectHandleLeft?.mutate()?.let { it.setTint(f.accent); text.setTextSelectHandleLeft(it) }
        text.textSelectHandleRight?.mutate()?.let { it.setTint(f.accent); text.setTextSelectHandleRight(it) }
    }

    private fun size(f: Field) {
        val w = max(1, f.w); val h = max(1, f.h)
        f.text.layoutParams = LayoutParams(w, h)
        f.text.measure(MeasureSpec.makeMeasureSpec(w, MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(h, MeasureSpec.EXACTLY))
        f.text.layout(0, 0, w, h)
        f.dirty = true
    }

    /** Each field where the stream draws it now: where it was painted, moved as its scroller's rows are (the reader
     * moves them itself between exact2's paints, [Scrolling]). Called after every stream and every such move. */
    fun place() {
        for (f in fields.values) if (f.group != 0) position(f)
    }

    private fun position(f: Field) {
        val inert = f.flags and INERT != 0
        if (f.text.isFocusable == inert) {
            // A field that is covered while it holds the focus (a route was pushed over its own) gives the focus up
            // itself, before it stops being focusable. Left to the platform, a view that can no longer hold the
            // focus drops it with the keyboard still up and still connected to it: the page on top then sat under
            // a keyboard that typed into the field beneath, and Back left the page before it closed the keyboard
            // (seen on a Pixel with Shelf: search, then a tap on a result).
            if (inert && f.text.isFocused) { yielded(f); hide() }
            f.text.isFocusable = !inert; f.text.isFocusableInTouchMode = !inert
        }
        f.port.importantForAccessibility =
            if (inert || f.flags and PLACED == 0) IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS else IMPORTANT_FOR_ACCESSIBILITY_AUTO
        if (f.flags and PLACED == 0) {
            // Not painted (its row is out of the page's view): it keeps its view, its focus and its keyboard, out of
            // sight, as a field scrolled out of a ScrollView does.
            f.atX = -(f.w + AWAY); f.atY = f.y
            f.port.translationX = 0f; f.port.translationY = 0f
            f.text.translationX = f.atX; f.text.translationY = f.atY
            return
        }
        val g = if (f.group != 0) view?.group(f.group) else null
        f.atX = (f.x + (g?.translationX ?: 0f)).roundToInt().toFloat()
        f.atY = (f.y + (g?.translationY ?: 0f)).roundToInt().toFloat()
        val clipped = f.clipW >= 0f
        val px = if (clipped) f.clipX.roundToInt() else 0
        val py = if (clipped) f.clipY.roundToInt() else 0
        val pw = if (clipped) max(1, f.clipW.roundToInt()) else LayoutParams.MATCH_PARENT
        val ph = if (clipped) max(1, f.clipH.roundToInt()) else LayoutParams.MATCH_PARENT
        val lp = f.port.layoutParams
        if (lp.width != pw || lp.height != ph) f.port.layoutParams = LayoutParams(pw, ph)
        f.port.translationX = px.toFloat(); f.port.translationY = py.toFloat()
        f.text.translationX = f.atX - px; f.text.translationY = f.atY - py
    }

    // The focus and the keyboard.

    /** exact2 asks the keyboard for field [id] ([Requests], kind 6): the field takes the focus and its keyboard
     * shows. A field the reader has not been told of yet takes it when it is. */
    fun focus(id: Int) {
        val f = fields[id] ?: run { wanted = id; return }
        wanted = 0
        if (!f.text.isFocused) {
            quiet = true
            try { f.text.requestFocus() } finally { quiet = false }
        }
        if (android.os.Build.VERSION.SDK_INT >= 30) f.text.windowInsetsController?.show(WindowInsets.Type.ime()) else Legacy.showKeyboard(f.text)
    }

    /** exact2 says no hosted field holds the focus: the one that did lets go of it and the keyboard goes. Where
     * exact2 wants no keyboard at all ([down]) it goes whether or not a field is known to hold the focus: one that
     * lost it by another road may have left its keyboard up. */
    fun blur(down: Boolean = true) {
        wanted = 0
        val f = focused
        if (f != null) yielded(f)
        if (f != null || down) hide()
    }

    /** [f] lets go of the focus because exact2 took it away, so exact2 is told nothing. The reader's own view takes
     * the focus where it can, so keys go on to exact2. */
    private fun yielded(f: Field) {
        quiet = true
        try { if (view?.requestFocus() != true) f.text.clearFocus() } finally { quiet = false }
        if (focused === f) focused = null
    }

    private fun hide() { imm.hideSoftInputFromWindow(windowToken, 0) }

    /** A surface of the top layer is over the page ([on]), or no longer ([Surfaces], LLP 1115.001 D10). Under one
     * the page is inert, so no field can take the focus, whatever would hand it one: a hardware key ends touch mode,
     * and the platform then focuses the window's first focusable view, which is a field (seen on a Pixel: Tab in an
     * alert put a caret in the field under its scrim, and the keyboard rose when the alert closed). exact2 refuses
     * such a focus too (`Presenter::reader_field_focused`); this keeps the view from taking it at all. */
    fun covered(on: Boolean) {
        descendantFocusability = if (on) FOCUS_BLOCK_DESCENDANTS else FOCUS_AFTER_DESCENDANTS
        if (on) blur()
    }

    // Touches.

    private var state = NONE
    private var target: Field? = null
    private val held = ArrayList<MotionEvent>()
    /** The touch being held or routed, counted: an answer for one that ended is not taken for the next. */
    var serial = 0
        private set
    /** The field asked its parents not to take its touch (it drags a selection or its caret): the list around it
     * does not scroll under it, as a ScrollView's does not. */
    var keeps = false
        private set

    override fun requestDisallowInterceptTouchEvent(disallow: Boolean) {
        keeps = disallow
        super.requestDisallowInterceptTouchEvent(disallow)
    }

    /** The field whose box (within its clip) holds a point of the reader's view. */
    private fun at(x: Float, y: Float): Field? = fields.values.firstOrNull { f ->
        f.flags and (PLACED or INERT) == PLACED && x >= f.atX && y >= f.atY && x < f.atX + f.w && y < f.atY + f.h &&
            (f.clipW < 0f || (x >= f.clipX && y >= f.clipY && x < f.clipX + f.clipW && y < f.clipY + f.clipH))
    }

    /**
     * A touch event of the reader's view: whether it is a field's to have, so that exact2 does not hear it. A down in
     * a field's box is held, with what follows it, until exact2 says what is under it ([under]).
     */
    fun touch(e: MotionEvent): Boolean {
        if (e.actionMasked == MotionEvent.ACTION_DOWN) {
            drop()
            val f = at(e.x, e.y) ?: return false
            serial++
            keeps = false
            target = f
            state = HOLDING
            held.add(MotionEvent.obtain(e))
            return true
        }
        when (state) {
            HOLDING -> held.add(MotionEvent.obtain(e))
            OWNED -> {
                send(e)
                if (e.actionMasked == MotionEvent.ACTION_UP || e.actionMasked == MotionEvent.ACTION_CANCEL) { state = NONE; target = null }
            }
            else -> return false
        }
        return true
    }

    /** exact2's answer for touch [serial]: the hosted field under it, or 0. The field's, the touch goes to the field
     * from its down; anything else's (the field is under a bar, a sheet), it goes to exact2 as any touch does. */
    fun under(serial: Int, field: Int) {
        if (serial != this.serial || state != HOLDING) return
        val f = target
        val events = ArrayList(held)
        held.clear()
        if (f != null && f.id == field && fields[field] === f) {
            state = OWNED
            Exact.fieldInput(f.id, f.generation) // a finger down in it: real input, which lets a held state go
            for (e in events) {
                send(e)
                if (e.actionMasked == MotionEvent.ACTION_UP || e.actionMasked == MotionEvent.ACTION_CANCEL) { state = NONE; target = null }
            }
        } else {
            state = NONE; target = null
            val v = view
            if (v != null) for (e in events) {
                val action = when (e.actionMasked) {
                    MotionEvent.ACTION_DOWN -> 0; MotionEvent.ACTION_UP -> 1; MotionEvent.ACTION_MOVE -> 2; else -> 3
                }
                v.forward(action, e.x, e.y, e.getToolType(0), -1L)
            }
        }
        for (e in events) e.recycle()
    }

    /** A scroll took the touch ([Scrolling]): the field's touch is cancelled, or what was held is let go. Whether
     * the touch was a field's (exact2 then has nothing to cancel). */
    fun cancel(): Boolean {
        when (state) {
            NONE -> return false
            OWNED -> {
                val now = android.os.SystemClock.uptimeMillis()
                val e = MotionEvent.obtain(now, now, MotionEvent.ACTION_CANCEL, 0f, 0f, 0)
                target?.text?.dispatchTouchEvent(e)
                e.recycle()
            }
        }
        drop()
        return true
    }

    private fun drop() {
        for (e in held) e.recycle()
        held.clear()
        state = NONE; target = null
    }

    /** [e] to the touch's field, in the field's own coordinates. */
    private fun send(e: MotionEvent) {
        val f = target ?: return
        val own = MotionEvent.obtain(e)
        own.offsetLocation(-f.atX, -f.atY)
        f.text.dispatchTouchEvent(own)
        own.recycle()
    }

    private fun fl(bits: Int) = java.lang.Float.intBitsToFloat(bits)

    /** The string at [at]: its bytes' count, then UTF-8 padded to whole words. And the index after it. */
    private fun string(w: IntArray, at: Int): Pair<String, Int> {
        val n = w[at]
        val bytes = ByteArray(n) { k -> (w[at + 1 + k / 4] ushr (8 * (k % 4))).toByte() }
        return String(bytes, Charsets.UTF_8) to at + 1 + (n + 3) / 4
    }

    companion object {
        private const val TAG = "exact"
        /** The words of a field before its value, and of its look before its strings. */
        private const val HEAD = 13
        private const val LOOK = 18
        private const val PLACED = 1
        private const val INERT = 2
        /** The look's first word ([Native.fields]). */
        private const val MULTILINE = 1
        private const val PASSWORD = 2
        private const val DISABLED = 4
        private const val READONLY = 8
        private const val NO_CORRECT = 16
        private const val NO_SUGGEST = 32
        private const val ITALIC = 64
        private const val SUBMITS = 128
        private const val FOLLOWED = 256
        /** Its keyboard: a search's, and none at all. */
        private const val SEARCHES = 6
        private const val NO_KEYBOARD = 7
        /** What an action key does ([Native.fieldAction]). */
        private const val DONE = 2
        private const val GO = 3
        private const val NEXT = 4
        private const val PREVIOUS = 5
        private const val SEARCH = 6
        private const val SEND = 7
        private const val TAB = 16
        private const val BACKTAB = 17
        private const val NONE = 0
        private const val HOLDING = 1
        private const val OWNED = 2
        /** How far out of sight a field with no place is kept, px. */
        private const val AWAY = 100000f

        /** The context a measuring field is made in: the activity that reads the host, for its theme. */
        @Volatile internal var context: Context? = null
        private val measurer = ThreadLocal<EditText>()
        private val faces = ConcurrentHashMap<String, Typeface>()

        /** An [EditText] with nothing of its style's that would move its text: no background and so no padding, no
         * minimum size, and no font padding, so that a line is as tall as its font says (Compose's text has none
         * either). What is left to measure is the text. */
        private fun <T : EditText> bare(v: T): T {
            v.background = null
            v.setPadding(0, 0, 0, 0)
            v.includeFontPadding = false
            v.minHeight = 0; v.minWidth = 0; v.minimumHeight = 0; v.minimumWidth = 0
            return v
        }

        /** The type a field is set in ([Native.fields]): size in px, weight, slant, tracking in ems, the line's
         * height in px (0: the font's own) and the face's file with its index in a collection (`path#index`; empty:
         * the device's default family). */
        private fun face(v: EditText, size: Float, weight: Int, italic: Boolean, spacing: Float, line: Float, family: String) {
            v.setTextSize(TypedValue.COMPLEX_UNIT_PX, size)
            v.typeface = faces.getOrPut("$family|$weight|$italic") {
                val w = weight.coerceIn(1, 1000)
                val hash = family.lastIndexOf('#')
                val made = if (hash <= 0) null else try {
                    Typeface.Builder(File(family.substring(0, hash))).setTtcIndex(family.substring(hash + 1).toIntOrNull() ?: 0)
                        .setFontVariationSettings("'wght' $w").setWeight(w).setItalic(italic).build()
                } catch (e: RuntimeException) { null }
                made ?: Typeface.create(Typeface.DEFAULT, w, italic)
            }
            v.letterSpacing = spacing
            if (line > 0f) v.setLineHeight(max(1, line.roundToInt())) else v.setLineSpacing(0f, 1f)
        }

        /** The input type a field's `type`, `inputmode`, `autocapitalize`, `autocorrect` and `spellcheck` ask for. */
        private fun inputType(what: Int, keyboard: Int, capitalize: Int): Int {
            val password = what and PASSWORD != 0
            var t = when (keyboard) {
                1 -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
                2 -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
                3 -> InputType.TYPE_CLASS_PHONE
                4 -> InputType.TYPE_CLASS_NUMBER
                5 -> InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL or InputType.TYPE_NUMBER_FLAG_SIGNED
                else -> InputType.TYPE_CLASS_TEXT
            }
            if (password) t = if (t and InputType.TYPE_MASK_CLASS == InputType.TYPE_CLASS_NUMBER)
                InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_VARIATION_PASSWORD
            else InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            if (t and InputType.TYPE_MASK_CLASS != InputType.TYPE_CLASS_TEXT) return t
            if (what and MULTILINE != 0) t = t or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            if (password) return t
            t = t or when (capitalize) {
                1 -> InputType.TYPE_TEXT_FLAG_CAP_SENTENCES; 2 -> InputType.TYPE_TEXT_FLAG_CAP_WORDS
                3 -> InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS; else -> 0
            }
            // Android has one switch for the spell checker and the suggestions; autocorrect is a flag of its own, on
            // for prose as Compose's fields have it.
            if (what and NO_SUGGEST != 0) t = t or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            else if (what and NO_CORRECT == 0 && t and InputType.TYPE_MASK_VARIATION == 0) t = t or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
            return t
        }

        /** Autofill's hints for HTML's `autocomplete`: its last token, as the platform names it. */
        private fun hints(autocomplete: String): Array<String> {
            val hint = when (autocomplete.trim().substringAfterLast(' ')) {
                "email" -> View.AUTOFILL_HINT_EMAIL_ADDRESS; "username" -> View.AUTOFILL_HINT_USERNAME
                "current-password" -> View.AUTOFILL_HINT_PASSWORD; "new-password" -> "newPassword"
                "one-time-code" -> "smsOTPCode"; "name" -> View.AUTOFILL_HINT_NAME
                "given-name" -> "personGivenName"; "family-name" -> "personFamilyName"
                "tel" -> View.AUTOFILL_HINT_PHONE; "postal-code" -> View.AUTOFILL_HINT_POSTAL_CODE
                "street-address" -> View.AUTOFILL_HINT_POSTAL_ADDRESS; "country", "country-name" -> "addressCountry"
                "address-level1" -> "addressRegion"; "address-level2" -> "addressLocality"
                "cc-number" -> View.AUTOFILL_HINT_CREDIT_CARD_NUMBER; "cc-exp" -> View.AUTOFILL_HINT_CREDIT_CARD_EXPIRATION_DATE
                "cc-csc" -> View.AUTOFILL_HINT_CREDIT_CARD_SECURITY_CODE
                else -> return emptyArray()
            }
            return arrayOf(hint)
        }

        /**
         * A field's text, measured by the class that draws it (LLP 1115.001 D8): exact2 calls this while it lays the
         * page out, on the thread layout runs on, and has the answer when it returns. An [EditText] of this thread's
         * own, never attached, is set as the field is ([bare], [face]) and measured: its width, its height and its
         * first baseline in px, 21 bits each, or -1 when there is no reader to measure in. [flags]: bit 0 italic, bit
         * 1 a textarea, whose lines wrap at [width] px (negative: as long as they are).
         */
        @JvmStatic fun measure(size: Float, weight: Int, flags: Int, spacing: Float, line: Float, family: String, text: String, width: Float): Long {
            val context = context ?: return -1L
            // A host booting on a thread of exact2's own: a view wants a looper to be made on.
            if (Looper.myLooper() == null) Looper.prepare()
            val v = measurer.get() ?: bare(EditText(unadjusted(context))).also {
                it.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO
                measurer.set(it)
            }
            val multiline = flags and 2 != 0
            val type = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS or
                (if (multiline) InputType.TYPE_TEXT_FLAG_MULTI_LINE else 0)
            if (v.inputType != type) v.inputType = type
            face(v, size, weight, flags and 1 != 0, spacing, line, family)
            v.setText(text)
            val free = MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED)
            val wide = if (multiline && width >= 0f) MeasureSpec.makeMeasureSpec(ceil(width).toInt(), MeasureSpec.EXACTLY) else free
            v.measure(wide, free)
            val most = (1 shl 21) - 1
            return min(v.measuredWidth, most).toLong() or (min(v.measuredHeight, most).toLong() shl 21) or
                (v.baseline.coerceIn(0, most).toLong() shl 42)
        }
    }
}

/** [context] with no font weight adjustment, for a field's [EditText]: Bold Text's step is in the weight the kernel
 * gives it (LLP 1115.000 D12.3, UIKit's step, which light weights take too), and a `TextView` would otherwise add the
 * device's adjustment to every typeface it is set in, the one it measures in included. */
internal fun unadjusted(context: Context): Context = ContextThemeWrapper(context, context.theme).apply {
    if (android.os.Build.VERSION.SDK_INT >= 31) applyOverrideConfiguration(Configuration().apply { fontWeightAdjustment = 0 })
}
