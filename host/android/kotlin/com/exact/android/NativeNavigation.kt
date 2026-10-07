package com.exact.android

import android.content.Context
import android.graphics.Rect
import android.view.MenuItem
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.Toolbar

/** Projection of authored route/header/tab state into Android SDK chrome.
 * The shared runner owns route identity, retained stacks and every action. Android
 * toolbar/menu/system-back presses always resolve a currently live authored target.
 * The presenter owns attachment and suppresses the returned authored descendants.
 */
internal class NativeNavigation(private val context: Context, private val press: (Int) -> Unit) {
    data class Node(
        val id: Int,
        val kind: String,
        val props: Map<String, String>,
        val children: IntArray,
        val box: Presenter.Box?,
        val frame: Rect,
        val handlers: Set<String>,
        val faceTitle: String? = null,
        val visible: Boolean = true
    )
    data class Mount(val owner: Int, val view: View)
    data class Projection(val mounts: List<Mount>, val hiddenRoutes: Set<Int>, val projected: Set<Int>)
    private data class BarItem(val id: Int, val label: String, val accessibility: String, val enabled: Boolean)
    private data class Bar(val title: String, val subtitle: String?, val items: List<BarItem>, val back: Boolean)
    private class Chrome(val view: View) { var bar: Bar? = null; var tabs: List<BarItem>? = null; var selected: List<Boolean>? = null }
    private val chrome = linkedMapOf<Int, Chrome>()
    private var nodes = emptyMap<Int, Node>()
    private var parents = emptyMap<Int, Int>()
    private var navigationRoot: Int? = null
    private var activeRoute: Int? = null
    private var activeDepth = 0
    private var closed = false

    /** Called for a cold tree/props/style change, never for a paint-only delta. */
    fun sync(roots: IntArray, nodes: List<Node>): Projection {
        check(!closed)
        require(nodes.map { it.id }.toSet().size == nodes.size) { "duplicate Android navigation node" }
        this.nodes = nodes.associateBy { it.id }
        val nextParents = mutableMapOf<Int, Int>()
        for (node in nodes) for (child in node.children) {
            require(nextParents.put(child, node.id) == null) { "Android navigation node has multiple parents" }
        }
        parents = nextParents
        navigationRoot = roots.firstOrNull { this.nodes[it]?.props?.containsKey("navigationKey") == true }
        activeRoute = null
        activeDepth = 0
        val hidden = linkedSetOf<Int>()
        val projected = linkedSetOf<Int>()
        val mounts = mutableListOf<Mount>()
        val liveChrome = linkedSetOf<Int>()
        val root = navigationRoot?.let { this.nodes[it] }
        if (root != null) {
            val direct = root.children.asSequence().mapNotNull { this.nodes[it] }.toList()
            val routes = direct.filter { it.props.containsKey("navigationKey") }.ifEmpty {
                // A root-level tabpanel retains its own authored route stack.
                direct.filter { role(it) == "tabpanel" }.flatMap { panel ->
                    panel.children.asSequence().mapNotNull { this.nodes[it] }.toList().filter { it.props.containsKey("navigationKey") }
                }
            }
            if (routes.isNotEmpty()) {
                val selected = routes.indexOfFirst { it.props["navigationKey"] == root.props["navigationKey"] }
                require(selected >= 0) { "Android navigationKey names no retained route" }
                val route = routes[selected]
                activeRoute = route.id
                // Depth belongs to this panel's stack, not another tab's routes.
                val owner = parents[route.id]
                activeDepth = routes.take(selected + 1).count { parents[it.id] == owner }
                hidden.addAll(routes.filter { it.id != route.id }.map { it.id })
                hidden.addAll(direct.filter { role(it) == "tabpanel" && it.id != owner }.map { it.id })
                header(route)?.let { shape ->
                    val back = backTarget() != null
                    val bar = Bar(shape.title, shape.subtitle, shape.items.filter { it.id != backTarget()?.id }, back)
                    mounts.add(Mount(shape.header.id, toolbar(shape.header.id, bar)))
                    liveChrome.add(shape.header.id)
                    projected.addAll(descendants(shape.header.id))
                }
            }
        }
        // Window/inline toolbars and ordinary tablists use SDK chrome too. A
        // header already projected into the navigation bar owns its descendants.
        for (node in nodes) {
            if (node.id in projected || belowAny(node.id, hidden) || !visibleInTree(node)) continue
            when (role(node)) {
                "toolbar" -> {
                    if (node.props["toolbarPlacement"] == "keyboard") continue
                    val items = actions(node.id).map(::item)
                    if (items.isEmpty()) continue
                    val bar = Bar(node.props["accessibilityLabel"].orEmpty(), null, items, false)
                    mounts.add(Mount(node.id, toolbar(node.id, bar)))
                    liveChrome.add(node.id)
                    projected.addAll(descendants(node.id))
                }
                "tablist" -> {
                    require(node.props["accessibilityOrientation"] != "vertical") { "Android vertical native tabs are not implemented" }
                    val tabs = node.children.asSequence().mapNotNull { this.nodes[it] }.toList().filter { role(it) == "tab" && "press" in it.handlers }
                    if (tabs.isEmpty()) continue
                    mounts.add(Mount(node.id, tabbar(node.id, tabs)))
                    liveChrome.add(node.id)
                    projected.addAll(descendants(node.id))
                }
            }
        }
        for (id in chrome.keys.filter { it !in liveChrome }) retire(id)
        return Projection(mounts, hidden, projected)
    }
    private data class Header(val header: Node, val title: String, val subtitle: String?, val items: List<BarItem>)
    private fun header(route: Node): Header? {
        val first = route.children.firstOrNull()?.let { nodes[it] } ?: return null
        if (first.props["semanticTag"] != "header" || !visibleInTree(first)) return null
        val all = descendants(first.id).mapNotNull { nodes[it] }.filter(::visibleInTree)
        val headings = all.filter {
            it.props.containsKey("accessibilityHeadingLevel") || (it.props["semanticTag"] in setOf("h1", "h2", "h3", "h4", "h5", "h6"))
        }
        if (headings.size != 1) return null
        val heading = headings.single()
        val items = actions(first.id).filter { action -> !contains(action.id, heading.id) }.map(::item)
        val subtitle = all.firstOrNull { it.kind == "text" && it.id != heading.id &&
            items.none { item -> contains(item.id, it.id) } }?.props?.get("text")?.takeIf { it.isNotEmpty() }
        return Header(first, label(heading), subtitle, items)
    }
    private fun toolbar(owner: Int, bar: Bar): Toolbar {
        var entry = chrome[owner]
        if (entry?.view !is Toolbar) {
            if (entry != null) retire(owner)
            val toolbar = Toolbar(context).apply {
                setOnMenuItemClickListener { selected -> activate(selected.itemId) }
                setNavigationOnClickListener { back() }
                val background = context.obtainStyledAttributes(intArrayOf(android.R.attr.colorBackground))
                try { setBackgroundColor(background.getColor(0, android.graphics.Color.WHITE)) } finally { background.recycle() }
            }
            entry = Chrome(toolbar)
            chrome[owner] = entry
        }
        val current = checkNotNull(entry)
        val toolbar = current.view as Toolbar
        if (current.bar == bar) return toolbar
        current.bar = bar
        toolbar.title = bar.title
        toolbar.subtitle = bar.subtitle
        toolbar.menu.clear()
        for ((order, item) in bar.items.withIndex()) toolbar.menu.add(0, item.id, order, item.label).apply {
            isEnabled = item.enabled
            contentDescription = item.accessibility
            setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        }
        if (bar.back) {
            val attributes = context.obtainStyledAttributes(intArrayOf(android.R.attr.homeAsUpIndicator))
            try { toolbar.navigationIcon = attributes.getDrawable(0) ?: context.getDrawable(android.R.drawable.ic_media_previous) }
            finally { attributes.recycle() }
            toolbar.navigationContentDescription = backTarget()?.let(::label)?.ifEmpty { "Back" } ?: "Back"
        } else {
            toolbar.navigationIcon = null
            toolbar.navigationContentDescription = null
        }
        return toolbar
    }
    private fun tabbar(owner: Int, tabs: List<Node>): LinearLayout {
        var entry = chrome[owner]
        if (entry?.view !is LinearLayout) {
            if (entry != null) retire(owner)
            entry = Chrome(LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL })
            chrome[owner] = entry
        }
        val current = checkNotNull(entry)
        val layout = current.view as LinearLayout
        val items = tabs.map(::item)
        val selected = tabs.map { it.props["accessibilitySelected"] == "true" }
        val rebuilt = current.tabs != items
        if (rebuilt) {
            layout.removeAllViews()
            for (item in items) layout.addView(Button(context).apply {
                text = item.label
                contentDescription = item.accessibility
                isEnabled = item.enabled
                setOnClickListener { activate(item.id) }
            }, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f))
            current.tabs = items
        }
        if (rebuilt || current.selected != selected) {
            for (index in selected.indices) layout.getChildAt(index).isSelected = selected[index]
            current.selected = selected
        }
        return layout
    }
    private fun item(node: Node): BarItem {
        val title = label(node).ifEmpty { "Action" }
        return BarItem(node.id, title, node.props["accessibilityLabel"]?.ifEmpty { title } ?: title, enabled(node))
    }
    private fun label(node: Node): String {
        node.faceTitle?.takeIf { it.isNotEmpty() }?.let { return it }
        node.props["text"]?.takeIf { it.isNotEmpty() }?.let { return it }
        val pieces = descendants(node.id).mapNotNull { nodes[it] }.mapNotNull { child -> child.props["text"]?.takeIf { it.isNotEmpty() } }
        if (pieces.isNotEmpty()) return pieces.joinToString(" ")
        return node.props["accessibilityLabel"] ?: node.props["id"] ?: node.props["testId"].orEmpty()
    }
    private fun actions(root: Int): List<Node> {
        val out = mutableListOf<Node>()
        fun visit(id: Int) {
            val node = nodes[id] ?: return
            if (!authoredVisible(node)) return
            if ("press" in node.handlers) { out.add(node); return }
            for (child in node.children) visit(child)
        }
        for (child in (nodes[root]?.children ?: intArrayOf())) visit(child)
        return out
    }
    private fun descendants(root: Int): List<Int> {
        val out = mutableListOf<Int>()
        val seen = hashSetOf(root)
        fun visit(id: Int) {
            require(seen.add(id)) { "cyclic Android navigation tree" }
            out.add(id)
            for (child in (nodes[id]?.children ?: intArrayOf())) visit(child)
        }
        for (child in (nodes[root]?.children ?: intArrayOf())) visit(child)
        return out
    }
    private fun contains(root: Int, target: Int): Boolean = root == target || descendants(root).contains(target)
    private fun belowAny(id: Int, ancestors: Set<Int>): Boolean {
        var at: Int? = id
        val visited = hashSetOf<Int>()
        while (at != null && visited.add(at)) {
            if (at in ancestors) return true
            at = parents[at]
        }
        return false
    }
    private fun role(node: Node): String? = node.props["accessibilityRole"] ?: node.props["role"]
    private fun authoredVisible(node: Node): Boolean = node.visible && node.props["hidden"] != "true"
    private fun visibleInTree(node: Node): Boolean {
        var at: Node? = node
        val seen = hashSetOf<Int>()
        while (at != null && seen.add(at.id)) {
            if (!authoredVisible(at)) return false
            at = parents[at.id]?.let { nodes[it] }
        }
        return true
    }
    private fun enabled(node: Node): Boolean {
        var at: Node? = node
        val seen = hashSetOf<Int>()
        while (at != null && seen.add(at.id)) {
            if (at.props["disabled"] == "true" || at.props["inert"] == "true" || !authoredVisible(at)) return false
            at = parents[at.id]?.let { nodes[it] }
        }
        return true
    }
    private fun backTarget(): Node? {
        if (closed || activeDepth <= 1) return null
        val root = navigationRoot?.let { nodes[it] } ?: return null
        val active = activeRoute ?: return null
        val target = root.props["navigationBack"] ?: return null
        return nodes.values.filter { it.props["id"] == target && "press" in it.handlers && enabled(it) && contains(active, it.id) }
            .minByOrNull { it.id.toUInt() }
    }
    private fun activate(id: Int): Boolean {
        val node = nodes[id] ?: return false
        if (closed || "press" !in node.handlers || !enabled(node)) return false
        val active = activeRoute
        // Retained inactive route controls cannot be activated by an old menu.
        var at: Int? = id
        while (at != null) {
            if (at != navigationRoot && nodes[at]?.props?.containsKey("navigationKey") == true && at != active) return false
            at = parents[at]
        }
        press(id)
        return true
    }
    /** A system/back icon press follows the current authored route and handler. */
    fun back(): Boolean = backTarget()?.let { activate(it.id) } ?: false
    private fun retire(id: Int) {
        val entry = chrome.remove(id) ?: return
        when (val view = entry.view) {
            is Toolbar -> { view.menu.clear(); view.setOnMenuItemClickListener(null); view.setNavigationOnClickListener(null) }
            is LinearLayout -> view.removeAllViews()
        }
        (entry.view.parent as? ViewGroup)?.removeView(entry.view)
    }
    fun close() {
        if (closed) return
        closed = true
        for (id in chrome.keys.toList()) retire(id)
        nodes = emptyMap()
        parents = emptyMap()
        navigationRoot = null
        activeRoute = null
    }
}
