package dev.exact.canvas

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Resources
import android.graphics.Rect
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.MotionEvent
import android.view.View
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction
import android.view.accessibility.AccessibilityNodeProvider
import android.widget.FrameLayout

/**
 * What the reader answers accessibility services with (LLP 1115.001 D13). What the reader shows is painted: the
 * page in one view, each bar and each surface of the top layer in a [Pane]. None of it says anything to TalkBack by
 * itself, so the view that holds it answers through an [AccessibilityNodeProvider] whose virtual nodes are exact2's
 * own statement of the tree ([Native.access], `host/linux/src/presenter/access.rs`): for each node its identity,
 * what it stands under, what it is, its state, its box, its range, its place in a set, and its name, value and
 * description, in the order a person reads.
 *
 * One [Access] answers for one view. The page's is the reader's root ([Root]), which holds the page's view, the
 * bars over it and the text fields under it: its nodes are the top app bar's, then the page's, then the navigation
 * bar's, in the window's own pixels. A surface of the top layer is a window of its own ([Surfaces]), and its pane
 * has an [Access] for that surface alone, in that window's pixels; while one is open exact2's tree is the surface
 * and nothing else, so the page's view answers with no node and nothing under an alert can be reached.
 *
 * **A node's identity** is exact2's ([Node.id]): a view's own id, or under a list's row a hash of the list, the
 * row's item and the node's place in the row, so it is the same node to TalkBack after a commit, a scroll, and a
 * row that leaves and returns. It is the virtual view id as it is.
 *
 * **What reports itself is not reported twice.** A hosted text field is a real `EditText` ([Fields]): where
 * exact2's tree has it, this adds the field's own view as the child (no virtual node), and the field says it stands
 * there and is labelled by its accessible name ([Stand], [FieldText]). The page's view, the bars' panes and the
 * layers that hold them say nothing: they are not important for accessibility.
 *
 * **Nothing is asked while no service is on.** [Owner] calls [Native.access] only while the system says an
 * accessibility service is enabled ([ExactActivity]), and exact2 builds the tree only when asked.
 *
 * **Actions** go to exact2 by one call ([Exact.accessAct]): a double tap is the presenter's own press at the node,
 * as the driver's `tap` is; a held press opens its context menu; a scroller scrolls and a range steps or is set.
 * Accessibility focus is kept here and is not exact2's keyboard focus: neither moves the other.
 *
 * **Events.** A statement says what changed since the last (`presenter::access::change`), and only that is sent:
 * a content change for the node whose subtree, text or state changed, a scroll for a scroller that moved, a
 * selection for a tab that became selected, and a content change from a live region so that TalkBack speaks it.
 * A statement told whole is one content change of the view, and a live region it names still speaks. The page's route is its pane: its title is the
 * view's pane title, which TalkBack announces when it changes.
 */
internal class Access(private val host: View) {
    /** One node of exact2's tree, its box in the host view's pixels. */
    class Node(
        val id: Int, val parent: Int, val view: Int, val role: Int, val flags: Int,
        val x: Int, val y: Int, val w: Int, val h: Int,
        val min: Float, val max: Float, val now: Float, val at: Int, val of: Int,
        val name: String, val value: String, val hint: String,
    ) {
        val children = ArrayList<Int>(0)
        fun has(flag: Int) = flags and flag != 0
    }

    /** A statement of the tree ([Native.access]), read. */
    class Told(w: IntArray) {
        /** The surface whose window the nodes are in; 0 for the page's. */
        val surface = w[1]
        /** The tree is told whole; the only changes then listed are live regions that speak. */
        val whole = w[2] == 1
        /** The route that shows and its title, as the top app bar has it. */
        val pane = w[3]
        val title: String
        /** Four words a change: a node (0: the view), what changed, and as float bits how far a scroller moved. */
        val changes: IntArray
        val nodes = ArrayList<Node>()

        init {
            var at = 4
            fun string(): String {
                val n = w[at]
                val bytes = ByteArray(n) { k -> (w[at + 1 + k / 4] ushr (8 * (k % 4))).toByte() }
                at += 1 + (n + 3) / 4
                return String(bytes, Charsets.UTF_8)
            }
            fun float(i: Int) = java.lang.Float.intBitsToFloat(w[i])
            title = string()
            changes = w.copyOfRange(at + 1, at + 1 + 4 * w[at])
            at += 1 + changes.size
            val count = w[at++]
            repeat(count) {
                val i = at
                at += 14
                nodes.add(Node(
                    w[i], w[i + 1], w[i + 2], w[i + 3], w[i + 4], w[i + 5], w[i + 6], w[i + 7], w[i + 8],
                    float(i + 9), float(i + 10), float(i + 11), w[i + 12], w[i + 13], string(), string(), string(),
                ))
            }
        }

        /** An alert's headline: what its window is titled. */
        val headline: String? get() = nodes.firstOrNull { it.role == HEADING && it.parent == 0 }?.name
    }

    /** Where a hosted field stands in the tree: under node [parent] of [host]'s (0: the view itself), named [name]. */
    class Stand(val host: View, val parent: Int, val name: String) {
        /** The field's own node, made to say so: its parent is the node it stands under, and its accessible name is
         * its hint, which TalkBack reads as an edit box's label. */
        fun place(info: AccessibilityNodeInfo) {
            if (parent == 0) info.setParent(host) else info.setParent(host, parent)
            if (name.isNotEmpty()) info.hintText = name
        }
    }

    private val manager = host.context.getSystemService(AccessibilityManager::class.java)
    private var nodes: Map<Int, Node> = emptyMap()
    /** The nodes that stand under the view itself, in order. */
    private var top: List<Int> = emptyList()
    /** The node that holds accessibility focus, and the one a finger explores. */
    private var focused = NONE
    private var hovered = NONE
    private var pane = 0
    /** The reader's text fields, where this view's nodes may be one ([Root]). */
    var fields: Fields? = null
    /** Real views that report themselves and stand after the nodes (the page's hosted elements). */
    var others: () -> List<View> = { emptyList() }
    private var placed = ArrayList<FieldText>()
    private val at = IntArray(2)
    private val box = Rect()
    /** `x_log_access=1` (exact2's EXACT_LOG_ACCESS): each statement, each node a service focuses or acts on and each
     * event sent, to the log: what TalkBack was on and did, where its speech cannot be read. */
    private val log = android.system.Os.getenv("EXACT_LOG_ACCESS") == "1"

    private fun say(what: String) { if (log) Log.i("exact", "access: $what") }

    /** A node as the log names it: its class, its words and its state. */
    private fun named(id: Int): String {
        val n = nodes[id] ?: return "node $id (gone)"
        val state = buildList {
            if (n.role == HEADING) add("heading"); if (n.role == LINK) add("link"); if (n.role == TAB) add("tab ${n.at} of ${n.of}")
            if (n.has(CHECKABLE)) add(if (n.has(CHECKED)) "checked" else "not checked")
            if (n.has(SELECTED)) add("selected"); if (n.has(DISABLED)) add("disabled")
            if (n.has(EXPANDABLE)) add(if (n.has(EXPANDED)) "expanded" else "collapsed")
            if (n.has(RANGE)) add("${n.now} in ${n.min}..${n.max}")
            if (n.has(CLICK)) add("click"); if (n.has(LONG_CLICK)) add("long click")
            if (n.has(FORWARD)) add("forward"); if (n.has(BACKWARD)) add("backward")
        }
        return "${kind(n).substringAfterLast('.')} \"${n.name.ifEmpty { if (n.role == UP) up(host.context) else "" }}\"" +
            (if (n.value.isEmpty()) "" else " value \"${n.value}\"") + (if (state.isEmpty()) "" else " [${state.joinToString(", ")}]")
    }

    /** exact2's statement of this view's tree. */
    fun told(t: Told) {
        val before = nodes
        val now = HashMap<Int, Node>(t.nodes.size * 2)
        val roots = ArrayList<Int>()
        for (n in t.nodes) now[n.id] = n
        for (n in t.nodes) (if (n.parent == 0) null else now[n.parent])?.children?.add(n.id) ?: roots.add(n.id)
        nodes = now; top = roots
        stand()
        if (focused != NONE && focused !in now) focused = NONE
        if (hovered != NONE && hovered !in now) hovered = NONE
        say("told surface ${t.surface}, ${t.nodes.size} nodes, ${if (t.whole) "whole" else "${t.changes.size / 4} changes"}, pane \"${t.title}\"")
        if (t.surface == 0 && (t.pane != pane || host.accessibilityPaneTitle?.toString() != t.title)) {
            // The route is the page's pane: TalkBack announces its title when it changes.
            pane = t.pane
            host.accessibilityPaneTitle = t.title.ifEmpty { null }
        }
        if (!manager.isEnabled) return
        // Told whole: one change, of everything. The changes it still lists are live regions that speak.
        if (t.whole) changed(HOST, AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE)
        var i = 0
        while (i < t.changes.size) {
            val id = t.changes[i]; val what = t.changes[i + 1]
            val source = if (id == 0) HOST else id
            val was = before[id]; val n = now[id]
            if (what and SUBTREE != 0) changed(source, AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE)
            else if (what and (TEXT_CHANGED or STATE) != 0) {
                var types = 0
                if (what and TEXT_CHANGED != 0) types = types or AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT or AccessibilityEvent.CONTENT_CHANGE_TYPE_CONTENT_DESCRIPTION
                if (what and STATE != 0) types = types or AccessibilityEvent.CONTENT_CHANGE_TYPE_STATE_DESCRIPTION
                changed(source, types)
            }
            if (what and STATE != 0 && n != null && was != null && n.has(SELECTED) && !was.has(SELECTED)) send(id, AccessibilityEvent.TYPE_VIEW_SELECTED)
            if (what and SCROLLED != 0 && n != null) {
                val dx = java.lang.Float.intBitsToFloat(t.changes[i + 2]); val dy = java.lang.Float.intBitsToFloat(t.changes[i + 3])
                send(id, AccessibilityEvent.TYPE_VIEW_SCROLLED) { e ->
                    e.scrollDeltaX = Math.round(dx); e.scrollDeltaY = Math.round(dy)
                    e.isScrollable = true
                }
            }
            // A live region speaks: a content change from the region itself is what TalkBack reads it by.
            if (what and LIVE != 0) changed(source, if (was != null && n != null && was.name != n.name) AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT else AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE)
            i += 4
        }
    }

    /** Nothing of this view can be reached: a surface is open over it, or no service asks any more. */
    fun clear() {
        if (nodes.isEmpty()) return
        nodes = emptyMap(); top = emptyList(); focused = NONE; hovered = NONE
        stand()
        if (manager.isEnabled) changed(HOST, AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE)
    }

    /** Each hosted field the tree has is told where it stands; one it no longer has reports itself as it did. */
    private fun stand() {
        val now = ArrayList<FieldText>()
        for (n in nodes.values) if (n.role == FIELD) fields?.text(n.view)?.let { it.stand = Stand(host, n.parent, n.name); now.add(it) }
        for (f in placed) if (f !in now) f.stand = null
        placed = now
    }

    val provider = object : AccessibilityNodeProvider() {
        override fun createAccessibilityNodeInfo(id: Int): AccessibilityNodeInfo? =
            if (id == HOST) whole() else nodes[id]?.takeIf { it.role != FIELD }?.let { node(it) }

        override fun performAction(id: Int, action: Int, args: Bundle?): Boolean {
            if (id == HOST) return host.performAccessibilityAction(action, args)
            val n = nodes[id] ?: return false
            if (log && action != AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) say(when (action) {
                AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS -> "focus"
                AccessibilityNodeInfo.ACTION_CLICK -> "click"
                AccessibilityNodeInfo.ACTION_LONG_CLICK -> "long click"
                else -> "action $action"
            } + " " + named(id))
            return when (action) {
                AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS -> focus(id)
                AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS -> unfocus(id)
                AccessibilityNodeInfo.ACTION_CLICK -> n.has(CLICK) && Exact.accessAct(id, PRESS, 0f)
                AccessibilityNodeInfo.ACTION_LONG_CLICK -> n.has(LONG_CLICK) && Exact.accessAct(id, HOLD, 0f)
                AccessibilityNodeInfo.ACTION_EXPAND -> n.has(EXPANDABLE) && !n.has(EXPANDED) && n.has(CLICK) && Exact.accessAct(id, PRESS, 0f)
                AccessibilityNodeInfo.ACTION_COLLAPSE -> n.has(EXPANDED) && n.has(CLICK) && Exact.accessAct(id, PRESS, 0f)
                AccessibilityNodeInfo.ACTION_SCROLL_FORWARD, android.R.id.accessibilityActionScrollDown, android.R.id.accessibilityActionScrollRight ->
                    n.has(FORWARD) && Exact.accessAct(id, ON, 0f)
                AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD, android.R.id.accessibilityActionScrollUp, android.R.id.accessibilityActionScrollLeft ->
                    n.has(BACKWARD) && Exact.accessAct(id, BACK, 0f)
                android.R.id.accessibilityActionSetProgress -> {
                    val to = args?.takeIf { it.containsKey(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE) } ?: return false
                    n.role == SLIDER && Exact.accessAct(id, SET, to.getFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE))
                }
                else -> false
            }
        }

        override fun findFocus(focus: Int): AccessibilityNodeInfo? =
            if (focus == AccessibilityNodeInfo.FOCUS_ACCESSIBILITY && focused != NONE) createAccessibilityNodeInfo(focused) else null
    }

    /** The view's own node: itself, with exact2's top nodes under it and then the real views that follow them. */
    private fun whole(): AccessibilityNodeInfo {
        val info = AccessibilityNodeInfo(host)
        host.onInitializeAccessibilityNodeInfo(info)
        under(info, top)
        for (v in others()) info.addChild(v)
        return info
    }

    private fun under(info: AccessibilityNodeInfo, ids: List<Int>) {
        for (id in ids) {
            val n = nodes[id] ?: continue
            // A hosted field is its own view, which says it stands here ([Stand]).
            if (n.role == FIELD) fields?.text(n.view)?.let { info.addChild(it) } else info.addChild(host, id)
        }
    }

    @SuppressLint("NewApi")
    private fun node(n: Node): AccessibilityNodeInfo {
        val info = AccessibilityNodeInfo(host, n.id)
        info.packageName = host.context.packageName
        info.className = kind(n)
        if (n.parent == 0 || n.parent !in nodes) info.setParent(host) else info.setParent(host, n.parent)
        host.getLocationOnScreen(at)
        box.set(at[0] + n.x, at[1] + n.y, at[0] + n.x + n.w, at[1] + n.y + n.h)
        info.setBoundsInScreen(box)
        if (Build.VERSION.SDK_INT >= 34) {
            host.getLocationInWindow(at)
            box.set(at[0] + n.x, at[1] + n.y, at[0] + n.x + n.w, at[1] + n.y + n.h)
            info.setBoundsInWindow(box)
        }
        info.isVisibleToUser = true
        info.isImportantForAccessibility = true
        info.isEnabled = !n.has(DISABLED)
        info.isAccessibilityFocused = focused == n.id
        info.addAction(if (focused == n.id) AccessibilityAction.ACTION_CLEAR_ACCESSIBILITY_FOCUS else AccessibilityAction.ACTION_ACCESSIBILITY_FOCUS)
        // Its words: what it says is its text, and what only names it is its description.
        when (n.role) {
            IMAGE, SLIDER, PROGRESS, SCROLLER, TABLIST, GROUP, DROPDOWN -> if (n.name.isNotEmpty()) info.contentDescription = n.name
            UP -> info.contentDescription = n.name.ifEmpty { up(host.context) }
            EDIT -> { info.text = n.value; info.hintText = n.name; info.isEditable = true }
            else -> info.text = n.name
        }
        if (n.role != EDIT && n.value.isNotEmpty()) info.stateDescription = n.value
        if (n.hint.isNotEmpty()) info.tooltipText = n.hint
        when (n.role) {
            HEADING -> info.isHeading = true
            LINK -> info.extras.putCharSequence(ROLE, "link")
            TAB -> {
                // As Material's own tabs and navigation bar items say it: a tab, its place, and whether it is selected.
                info.extras.putCharSequence(ROLE, "Tab")
                if (n.of > 0) info.collectionItemInfo = AccessibilityNodeInfo.CollectionItemInfo(0, 1, n.at - 1, 1, false, n.has(SELECTED))
            }
            TABLIST -> if (n.of > 0) info.collectionInfo = AccessibilityNodeInfo.CollectionInfo(1, n.of, false, AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_SINGLE)
            SLIDER, PROGRESS -> if (n.has(RANGE)) info.rangeInfo = AccessibilityNodeInfo.RangeInfo(AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT, n.min, n.max, n.now)
            SCROLLER -> {
                info.isScrollable = n.has(FORWARD) || n.has(BACKWARD)
                // A list's rows say their place and how many they are: the scroller is that list.
                val rows = n.children.maxOfOrNull { nodes[it]?.of ?: 0 } ?: 0
                if (rows > 0) info.collectionInfo = AccessibilityNodeInfo.CollectionInfo(if (n.has(SIDEWAYS)) 1 else rows, if (n.has(SIDEWAYS)) rows else 1, false)
            }
        }
        if (n.role != TAB && n.of > 0 && n.at > 0) {
            val sideways = nodes[n.parent]?.has(SIDEWAYS) == true
            info.collectionItemInfo = AccessibilityNodeInfo.CollectionItemInfo(if (sideways) 0 else n.at - 1, 1, if (sideways) n.at - 1 else 0, 1, false)
        }
        if (n.has(CHECKABLE)) {
            info.isCheckable = true
            if (Build.VERSION.SDK_INT >= 36) info.checked = if (n.has(CHECKED)) AccessibilityNodeInfo.CHECKED_STATE_TRUE else AccessibilityNodeInfo.CHECKED_STATE_FALSE
            else @Suppress("DEPRECATION") { info.isChecked = n.has(CHECKED) }
        }
        info.isSelected = n.has(SELECTED)
        if (n.has(POLITE)) info.liveRegion = View.ACCESSIBILITY_LIVE_REGION_POLITE
        if (n.has(ASSERTIVE)) info.liveRegion = View.ACCESSIBILITY_LIVE_REGION_ASSERTIVE
        // What a double tap does. A tab that is selected takes none, as Material's does not.
        if (n.has(CLICK) && !(n.role == TAB && n.has(SELECTED))) { info.isClickable = true; info.addAction(AccessibilityAction.ACTION_CLICK) }
        if (n.has(LONG_CLICK)) { info.isLongClickable = true; info.addAction(AccessibilityAction.ACTION_LONG_CLICK) }
        if (n.has(EXPANDABLE) && n.has(CLICK)) info.addAction(if (n.has(EXPANDED)) AccessibilityAction.ACTION_COLLAPSE else AccessibilityAction.ACTION_EXPAND)
        if (n.role == SCROLLER || n.role == SLIDER) {
            val sideways = n.has(SIDEWAYS)
            if (n.has(FORWARD)) {
                info.addAction(AccessibilityAction.ACTION_SCROLL_FORWARD)
                if (n.role == SCROLLER) info.addAction(if (sideways) AccessibilityAction.ACTION_SCROLL_RIGHT else AccessibilityAction.ACTION_SCROLL_DOWN)
            }
            if (n.has(BACKWARD)) {
                info.addAction(AccessibilityAction.ACTION_SCROLL_BACKWARD)
                if (n.role == SCROLLER) info.addAction(if (sideways) AccessibilityAction.ACTION_SCROLL_LEFT else AccessibilityAction.ACTION_SCROLL_UP)
            }
            if (n.role == SLIDER) info.addAction(AccessibilityAction.ACTION_SET_PROGRESS)
        }
        under(info, n.children)
        return info
    }

    /**
     * What Android has for this view, for a driver's `tree --ax` (LLP 1080.002; [AgentDrive]): each node's own
     * [AccessibilityNodeInfo], the object Android hands a service, read back in the order a service walks it; for a
     * hosted field, the field's own. Appended to [out] as the driver's elements: what the node stands for, by its
     * class, its heading flag and its role description; its name, value and states as the node says them; its box
     * in points; and the view it is, which is exact2's to join.
     */
    fun ax(out: org.json.JSONArray, limit: Int): Int {
        val density = host.resources.displayMetrics.density
        var more = 0
        fun walk(ids: List<Int>, parent: Int?) {
            for (id in ids) {
                val n = nodes[id] ?: continue
                val info = (if (n.role == FIELD) fields?.text(n.view)?.createAccessibilityNodeInfo() else provider.createAccessibilityNodeInfo(id)) ?: continue
                if (out.length() >= limit) { more++; continue }
                val i = out.length()
                out.put(element(i, parent, n, info, density))
                walk(n.children, i)
            }
        }
        walk(top, null)
        return more
    }

    @Suppress("DEPRECATION")
    private fun element(i: Int, parent: Int?, n: Node, info: AccessibilityNodeInfo, density: Float): org.json.JSONObject {
        val kind = info.className?.toString().orEmpty()
        val said = info.extras.getCharSequence(ROLE)?.toString()
        val field = kind.endsWith("EditText")
        val actions = info.actionList.map { it.id }
        val role = when {
            info.isHeading -> "heading"
            said == "link" -> "link"
            said == "Tab" -> "tab"
            else -> when (kind.substringAfterLast('.')) {
                "Button", "ImageButton", "ToggleButton" -> "button"
                "CheckBox" -> "checkbox"; "Switch" -> "switch"; "RadioButton" -> "radio"
                "SeekBar" -> "slider"; "ProgressBar" -> "progressbar"; "EditText" -> "textbox"
                "ImageView" -> "image"; "Spinner" -> "combobox"; "TextView" -> "StaticText"
                else -> if (info.collectionInfo?.selectionMode == AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_SINGLE) "tablist" else "group"
            }
        }
        // An edit box is named by its hint, and its text is its value; anything else by its description, else its text.
        val name = (if (field) info.hintText else info.contentDescription ?: info.text)?.toString().orEmpty()
        val value = (if (field) info.text?.takeUnless { info.isShowingHintText } else info.stateDescription)?.toString()
        val states = org.json.JSONObject()
        if (info.isCheckable) states.put("checked", info.isChecked)
        if (info.isSelected || said == "Tab") states.put("selected", info.isSelected)
        if (!info.isEnabled) states.put("disabled", true)
        if (AccessibilityNodeInfo.ACTION_EXPAND in actions) states.put("expanded", false)
        if (AccessibilityNodeInfo.ACTION_COLLAPSE in actions) states.put("expanded", true)
        if (info.isPassword) states.put("protected", true)
        info.getBoundsInScreen(box)
        host.rootView.getLocationOnScreen(at)
        val native = org.json.JSONObject().put("class", kind)
        said?.let { native.put("role", it) }
        if (info.isHeading) native.put("heading", true)
        info.rangeInfo?.let { native.put("range", org.json.JSONArray(listOf(it.min, it.max, it.current))) }
        if (info.liveRegion != View.ACCESSIBILITY_LIVE_REGION_NONE) native.put("live", if (info.liveRegion == View.ACCESSIBILITY_LIVE_REGION_ASSERTIVE) "assertive" else "polite")
        native.put("actions", org.json.JSONArray(info.actionList.mapNotNull { a ->
            when (a.id) {
                AccessibilityNodeInfo.ACTION_CLICK -> "click"; AccessibilityNodeInfo.ACTION_LONG_CLICK -> "longClick"
                AccessibilityNodeInfo.ACTION_SCROLL_FORWARD -> "scrollForward"; AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD -> "scrollBackward"
                android.R.id.accessibilityActionSetProgress -> "setProgress"
                AccessibilityNodeInfo.ACTION_EXPAND -> "expand"; AccessibilityNodeInfo.ACTION_COLLAPSE -> "collapse"
                else -> null
            }
        }))
        val e = org.json.JSONObject().put("i", i).put("parent", parent ?: org.json.JSONObject.NULL)
            .put("id", if (n.view != 0) n.view else org.json.JSONObject.NULL).put("via", if (n.view != 0) "self" else "none")
            .put("role", role).put("name", name)
        if (!value.isNullOrEmpty() && !info.isPassword) e.put("value", value)
        info.tooltipText?.let { e.put("description", it.toString()) }
        return e.put("states", states).put("interactive", info.isClickable || info.isCheckable || field || AccessibilityNodeInfo.ACTION_SCROLL_FORWARD in actions || AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD in actions)
            .put("frame", org.json.JSONObject().put("x", points(box.left - at[0], density)).put("y", points(box.top - at[1], density))
                .put("w", points(box.width(), density)).put("h", points(box.height(), density)).put("source", "accessibility"))
            .put("native", native)
    }

    /** Pixels as points, to a hundredth, as the other hosts' frames are. */
    private fun points(px: Int, density: Float): Double = Math.round(px / density * 100.0) / 100.0

    private fun focus(id: Int): Boolean {
        if (focused == id) return false
        val was = focused
        focused = id
        if (was != NONE) send(was, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)
        send(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
        return true
    }

    private fun unfocus(id: Int): Boolean {
        if (focused != id) return false
        focused = NONE
        send(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)
        return true
    }

    /** A finger explores this view ([View.dispatchHoverEvent]): the node under it is the one TalkBack reads. Whether
     * it is over one of exact2's nodes; over a hosted field, or nothing, the views under the finger answer. */
    fun hover(e: MotionEvent): Boolean {
        if (nodes.isEmpty() || !manager.isEnabled || !manager.isTouchExplorationEnabled) return false
        return when (e.actionMasked) {
            MotionEvent.ACTION_HOVER_ENTER, MotionEvent.ACTION_HOVER_MOVE -> { val id = under(e.x.toInt(), e.y.toInt()); explore(id); id != NONE }
            MotionEvent.ACTION_HOVER_EXIT -> { val was = hovered; explore(NONE); was != NONE }
            else -> false
        }
    }

    private fun explore(id: Int) {
        if (id == hovered) return
        val was = hovered
        hovered = id
        if (was != NONE) send(was, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT)
        if (id != NONE) send(id, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER)
    }

    /** The innermost node at a point of the view that says something or takes a press; none over a hosted field. */
    private fun under(x: Int, y: Int): Int {
        fun inside(n: Node) = x >= n.x && x < n.x + n.w && y >= n.y && y < n.y + n.h
        fun find(ids: List<Int>): Int {
            for (k in ids.indices.reversed()) {
                val n = nodes[ids[k]] ?: continue
                if (!inside(n)) continue
                if (n.role == FIELD) return NONE
                val inner = find(n.children)
                if (inner != NONE) return inner
                if (n.name.isNotEmpty() || n.has(CLICK) || n.role == UP || n.has(RANGE)) return n.id
            }
            return NONE
        }
        return find(top)
    }

    private fun changed(id: Int, types: Int) = send(id, AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED) { it.contentChangeTypes = types }

    /** An event from node [id], or from the view itself. */
    private fun send(id: Int, type: Int, fill: ((AccessibilityEvent) -> Unit)? = null) {
        if (!manager.isEnabled) return
        val parent = host.parent ?: return
        val e = AccessibilityEvent(type)
        if (id == HOST) host.onInitializeAccessibilityEvent(e) else {
            val n = nodes[id] ?: return
            e.setSource(host, id)
            e.packageName = host.context.packageName
            e.className = kind(n)
            e.isEnabled = !n.has(DISABLED)
            e.isChecked = n.has(CHECKED)
            if (n.name.isNotEmpty()) e.text.add(n.name)
        }
        fill?.invoke(e)
        if (log && type != AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED && type != AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)
            say("event ${AccessibilityEvent.eventTypeToString(type)}${if (e.contentChangeTypes != 0) " change ${e.contentChangeTypes}" else ""} from ${if (id == HOST) "the view" else named(id)}")
        parent.requestSendAccessibilityEvent(host, e)
    }

    companion object {
        const val HOST = AccessibilityNodeProvider.HOST_VIEW_ID
        private const val NONE = Int.MIN_VALUE
        /** How a role is named where no class says it: the extra TalkBack reads, as AndroidX writes it. */
        private const val ROLE = "AccessibilityNodeInfo.roleDescription"

        /** What a node is (`presenter::access::role`). */
        const val GROUP = 0
        const val TEXT = 1
        const val HEADING = 2
        const val BUTTON = 3
        const val LINK = 4
        const val CHECKBOX = 5
        const val SWITCH = 6
        const val RADIO = 7
        const val SLIDER = 8
        const val IMAGE = 9
        const val TAB = 10
        const val SCROLLER = 11
        const val PROGRESS = 12
        const val EDIT = 13
        const val FIELD = 14
        const val TABLIST = 15
        const val DROPDOWN = 16
        const val TOGGLE = 17
        const val UP = 18

        /** A node's flags (`presenter::access::flag`). */
        const val CLICK = 1
        const val LONG_CLICK = 1 shl 1
        const val CHECKABLE = 1 shl 2
        const val CHECKED = 1 shl 3
        const val SELECTED = 1 shl 4
        const val DISABLED = 1 shl 5
        const val EXPANDABLE = 1 shl 6
        const val EXPANDED = 1 shl 7
        const val FORWARD = 1 shl 8
        const val BACKWARD = 1 shl 9
        const val SIDEWAYS = 1 shl 10
        const val POLITE = 1 shl 11
        const val ASSERTIVE = 1 shl 12
        const val RANGE = 1 shl 13

        /** What changed of a node (`presenter::access::change`). */
        private const val SUBTREE = 1
        private const val TEXT_CHANGED = 1 shl 1
        private const val STATE = 1 shl 2
        private const val SCROLLED = 1 shl 3
        private const val LIVE = 1 shl 4

        /** What a service asks of a node (`presenter::access::act`). */
        private const val PRESS = 1
        private const val HOLD = 2
        private const val ON = 3
        private const val BACK = 4
        private const val SET = 5

        /** The class TalkBack knows a role by. A heading, a link and a tab are said by the node, not by a class. */
        fun kind(n: Node): String = when (n.role) {
            TEXT, HEADING, LINK -> "android.widget.TextView"
            BUTTON -> "android.widget.Button"
            CHECKBOX -> "android.widget.CheckBox"
            SWITCH -> "android.widget.Switch"
            RADIO -> "android.widget.RadioButton"
            SLIDER -> "android.widget.SeekBar"
            IMAGE -> "android.widget.ImageView"
            SCROLLER -> if (n.has(SIDEWAYS)) "android.widget.HorizontalScrollView" else "android.widget.ScrollView"
            PROGRESS -> "android.widget.ProgressBar"
            EDIT -> "android.widget.EditText"
            DROPDOWN -> "android.widget.Spinner"
            TOGGLE -> "android.widget.ToggleButton"
            UP -> "android.widget.ImageButton"
            TAB -> "android.view.View"
            else -> "android.view.ViewGroup"
        }

        /** What the platform calls the Up arrow, in the device's language: the framework's own words for it. */
        private fun up(context: Context): String {
            val system = Resources.getSystem()
            val id = system.getIdentifier("action_bar_up_description", "string", "android")
            return if (id != 0) context.resources.getString(id) else "Navigate up"
        }
    }
}

/**
 * The reader's root: the view that holds the page's view, the fields under it and the bars over it, and answers
 * accessibility services for all of them ([Access]) while one is on. With none on it is a plain frame.
 */
internal class Root(context: Context) : FrameLayout(context) {
    val access = Access(this)
    /** Whether an accessibility service is on ([ExactActivity]). */
    var accessible = false

    override fun getAccessibilityNodeProvider(): AccessibilityNodeProvider? = if (accessible) access.provider else null

    override fun dispatchHoverEvent(event: MotionEvent): Boolean = (accessible && access.hover(event)) || super.dispatchHoverEvent(event)
}
