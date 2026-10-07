package com.exact.android

import android.app.AlertDialog
import android.app.DatePickerDialog
import android.app.TimePickerDialog
import android.content.Context
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.widget.Button
import android.widget.ArrayAdapter
import android.widget.CheckBox
import android.widget.CompoundButton
import android.widget.FrameLayout
import android.widget.RadioButton
import android.widget.SeekBar
import android.widget.Switch
import org.json.JSONObject
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import kotlin.math.abs
import kotlin.math.floor
import kotlin.math.roundToInt

/** Native chrome, with values and viewless contents owned by the shared kernel. */
internal class NativeControls(
    private val context: Context,
    private val dispatch: (Int, Int, String?) -> Unit,
    private val intrinsic: (Int, Float, Float) -> Unit
) {
    private val scale = context.resources.displayMetrics.density
    private val entries = android.util.SparseArray<Entry>()
    private val queries = LinkedHashSet<Int>()
    private var closed = false
    private var radioGroupsDirty = false
    private inner class Entry(val id: Int) {
        val root = FrameLayout(context)
        var widget: View? = null
        var kind = ""
        var props = JSONObject()
        var style = JSONObject()
        var face: JSONObject? = null
        var options: JSONObject? = null
        var picked: Int? = null
        var updating = false
        var group: JSONObject? = null
        var radioSignature: List<String>? = null
        var boundValue: String? = null
        var rangeStart = 0.0
        var rangeEnd = 100.0
        var rangeStep = 1.0
        var rangeChanged = false
        var rangeTracking = false
        var lastRange: String? = null
        var lastNatural: Pair<Float, Float>? = null
        var naturalMinimumWidth = 0
        var dialog: android.app.Dialog? = null
        fun alive() = !closed && entries[id] === this
        fun value(value: String, input: Boolean = true, change: Boolean = true) {
            if (!alive() || updating || widget?.isEnabled != true) return
            val expected = kind
            if (input) dispatch(id, 23, value)
            if (alive() && kind == expected && change) dispatch(id, 1, value)
        }
        fun checked(on: Boolean) {
            if (!alive() || updating || widget?.isEnabled != true) return
            val expected = kind
            dispatch(id, 25, on.toString())
            if (alive() && kind == expected) dispatch(id, 24, on.toString())
            // A committed checked prop is authoritative, including a refused
            // toggle whose action leaves its bound value unchanged.
            if (alive() && kind == expected) snapChecked()
        }
        fun setChecked(on: Boolean) {
            val control = widget as? CompoundButton ?: return
            if (control.isChecked == on) return
            val before = updating
            updating = true
            try { control.isChecked = on } finally { updating = before }
        }
        fun snapChecked() {
            if (props.has("checked") && !props.isNull("checked")) setChecked(props.optString("checked") == "true")
        }
        fun make(next: String) {
            dialog?.dismiss(); dialog = null
            root.removeAllViews()
            if (kind == "radio" || next == "radio") radioGroupsDirty = true
            kind = next; boundValue = null; face = null; options = null; group = null; picked = null; radioSignature = null
            rangeChanged = false; rangeTracking = false; lastRange = null
            val control: View = when (next) {
                "checkbox" -> CheckBox(context)
                "switch" -> Switch(context)
                "radio" -> object : RadioButton(context) {
                    // Exact owns the HTML group. Keep SDK chrome, but make its
                    // click wait for our group operation instead of toggling itself.
                    override fun toggle() = Unit
                }
                "range" -> SeekBar(context)
                "button", "select", "date", "time", "datetime-local" -> Button(context)
                else -> error("Android native control '$next' is not implemented")
            }
            widget = control
            naturalMinimumWidth = control.minimumWidth
            root.addView(control, FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.CENTER))
            control.setOnFocusChangeListener { _, focused ->
                if (alive() && !updating) dispatch(id, if (focused) 4 else 5, null)
            }
            if (control is RadioButton) {
                control.setOnClickListener { checkRadio(id) }
                control.setOnKeyListener { _, key, event ->
                    if (event.action != KeyEvent.ACTION_DOWN || event.isAltPressed || event.isCtrlPressed || event.isMetaPressed) false
                    else when (key) {
                        KeyEvent.KEYCODE_DPAD_DOWN, KeyEvent.KEYCODE_DPAD_RIGHT -> radioArrow(id, true)
                        KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_LEFT -> radioArrow(id, false)
                        KeyEvent.KEYCODE_SPACE -> { checkRadio(id); true }
                        else -> false
                    }
                }
            } else if (control is CompoundButton) control.setOnCheckedChangeListener { _, on -> checked(on) }
            if (control is SeekBar) control.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onStartTrackingTouch(view: SeekBar) { rangeChanged = false; rangeTracking = true; lastRange = null }
                override fun onProgressChanged(view: SeekBar, progress: Int, user: Boolean) {
                    if (user && !updating) {
                        val shown = rangeValue(progress)
                        if (shown != lastRange) {
                            lastRange = shown
                            rangeChanged = true
                            value(shown, change = false)
                        }
                    }
                }
                override fun onStopTrackingTouch(view: SeekBar) {
                    rangeTracking = false
                    if (rangeChanged) value(rangeValue(view.progress), input = false)
                    rangeChanged = false; lastRange = null
                    if (alive() && widget === view) configure(props, style)
                }
            })
            if (control is Button && control !is CompoundButton) control.setOnClickListener {
                if (!alive() || !control.isEnabled) return@setOnClickListener
                when (kind) {
                    "button" -> dispatch(id, 0, null)
                    "select" -> showSelect()
                    else -> showDate()
                }
            }
            if (next in setOf("button", "select", "radio")) queries.add(id)
        }
        fun rangeValue(progress: Int): String {
            val raw = rangeStart + (rangeEnd - rangeStart) * progress / 10000.0
            return formatRange(sanitizeRange(raw, rangeStart, rangeEnd, rangeStep))
        }
        fun configure(props: JSONObject, style: JSONObject) {
            updating = true
            try {
                this.props = props; this.style = style
                val declared = props.optString("type")
                val next = if (declared in setOf("button", "select", "radio", "range", "date", "time", "datetime-local")) declared
                    else if (props.optString("accessibilityRole") == "switch") "switch" else "checkbox"
                if (next != kind) make(next)
                val control = checkNotNull(widget)
                control.isEnabled = props.optString("disabled") != "true" && props.optString("inert") != "true"
                control.contentDescription = props.optString("accessibilityLabel").ifEmpty { null }
                control.tag = props.optString("testId")
                control.visibility = if (style.optString("appearance", "auto") == "none") View.INVISIBLE else View.VISIBLE
                if (control is CompoundButton) snapChecked()
                if (kind == "radio") {
                    val signature = listOf("name", "disabled", "inert", "checked").map { props.optString(it) }
                    if (radioSignature != signature) { radioSignature = signature; radioGroupsDirty = true }
                }
                if (control is SeekBar) {
                    fun finite(name: String, fallback: Double) = props.optString(name).trim().toDoubleOrNull()?.takeIf { it.isFinite() } ?: fallback
                    val start = finite("min", 0.0)
                    val end = finite("max", 100.0).coerceAtLeast(start)
                    val step = if (props.optString("step").trim().equals("any", ignoreCase = true)) 0.0 else finite("step", 1.0).takeIf { it > 0 } ?: 1.0
                    val raw = props.optString("value")
                    if (!rangeTracking && (boundValue != raw || start != rangeStart || end != rangeEnd || step != rangeStep)) {
                        rangeStart = start; rangeEnd = end; rangeStep = step
                        val number = raw.trim().toDoubleOrNull()?.takeIf { it.isFinite() } ?: (start / 2 + end / 2)
                        val snapped = sanitizeRange(number, start, end, step)
                        control.max = 10000
                        control.progress = if (end > start) ((snapped - start) * 10000 / (end - start)).roundToInt() else 0
                        boundValue = raw
                    }
                }
                if (control is Button && control !is CompoundButton) {
                    if (kind == "button") control.text = face?.optString("title", "") ?: ""
                    else if (kind != "select" && boundValue != props.optString("value")) {
                        boundValue = props.optString("value")
                        control.text = boundValue?.ifEmpty { kind } ?: kind
                    }
                }
                measure()
            } finally { updating = false }
        }
        fun measure() {
            val control = widget ?: return
            if (kind == "select") control.minimumWidth = naturalMinimumWidth
            control.measure(View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED),
                View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED))
            var naturalWidth = control.measuredWidth
            var naturalHeight = control.measuredHeight
            if (kind == "select") {
                val options = options?.optJSONArray("options")
                if (options != null && options.length() > 0) {
                    // HTML selects size to the widest option, even when another
                    // option is currently shown. A probe never mutates the live face.
                    val probe = Button(context).apply {
                        layoutParams = FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT)
                    }
                    for (i in 0 until options.length()) {
                        probe.text = options.getJSONObject(i).getString("label")
                        probe.measure(View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED),
                            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED))
                        naturalWidth = maxOf(naturalWidth, probe.measuredWidth)
                        naturalHeight = maxOf(naturalHeight, probe.measuredHeight)
                    }
                }
                control.minimumWidth = naturalWidth
            }
            val width = if (kind == "range") 129f else naturalWidth / scale
            val size = width to naturalHeight / scale
            if (size != lastNatural && size.first > 0 && size.second > 0) {
                lastNatural = size
                intrinsic(id, size.first, size.second)
            }
            val fill = kind == "button" || kind == "range"
            val params = control.layoutParams as FrameLayout.LayoutParams
            params.width = if (fill) FrameLayout.LayoutParams.MATCH_PARENT else FrameLayout.LayoutParams.WRAP_CONTENT
            params.height = if (kind == "button") FrameLayout.LayoutParams.MATCH_PARENT else FrameLayout.LayoutParams.WRAP_CONTENT
            params.gravity = Gravity.CENTER
            control.layoutParams = params
        }
        fun showSelect() {
            if (!alive() || kind != "select" || widget?.isEnabled != true) return
            val menu = options ?: return
            val choices = menu.getJSONArray("options")
            val labels = Array(choices.length()) { choices.getJSONObject(it).getString("label") }
            val selected = picked ?: if (menu.isNull("chosen")) null else menu.getInt("chosen")
            val adapter = object : ArrayAdapter<String>(context, android.R.layout.select_dialog_singlechoice, labels) {
                override fun getView(position: Int, convertView: View?, parent: android.view.ViewGroup): View {
                    val row = super.getView(position, convertView, parent)
                    row.isEnabled = isEnabled(position)
                    return row
                }
                override fun areAllItemsEnabled(): Boolean = false
                override fun isEnabled(position: Int): Boolean = position in labels.indices && !choices.getJSONObject(position).getBoolean("disabled")
            }
            dialog?.dismiss()
            dialog = AlertDialog.Builder(context).setSingleChoiceItems(adapter, selected?.takeIf { it in labels.indices } ?: -1) { d, index ->
                if (!alive() || kind != "select" || widget?.isEnabled != true || options !== menu || index !in labels.indices) {
                    d.dismiss(); return@setSingleChoiceItems
                }
                val option = choices.getJSONObject(index)
                if (option.getBoolean("disabled")) return@setSingleChoiceItems
                picked = index
                (widget as Button).text = option.getString("label")
                d.dismiss()
                value(option.getString("value"))
            }.setNegativeButton(android.R.string.cancel, null).show()
        }
        fun showDate() {
            if (!alive() || kind !in setOf("date", "time", "datetime-local") || widget?.isEnabled != true) return
            val expected = kind
            fun available() = alive() && kind == expected && widget?.isEnabled == true
            val current = (widget as Button).text.toString()
            val date = runCatching { LocalDate.parse(current.take(10)) }.getOrElse { LocalDate.now() }
            val time = runCatching { LocalTime.parse(if (kind == "datetime-local") current.substringAfter('T') else current) }.getOrElse { LocalTime.NOON }
            fun chooseTime(day: LocalDate? = null) {
                if (!available()) return
                dialog = TimePickerDialog(context, { _, hour, minute ->
                    if (!available()) return@TimePickerDialog
                    val selected = LocalTime.of(hour, minute)
                    val raw = if (day == null) selected.toString() else LocalDateTime.of(day, selected).toString()
                    (widget as Button).text = raw; value(raw)
                }, time.hour, time.minute, android.text.format.DateFormat.is24HourFormat(context)).also { it.show() }
            }
            if (kind == "time") { chooseTime(); return }
            dialog = DatePickerDialog(context, { _, year, month, day ->
                if (!available()) return@DatePickerDialog
                val selected = LocalDate.of(year, month + 1, day)
                if (kind == "datetime-local") chooseTime(selected)
                else { (widget as Button).text = selected.toString(); value(selected.toString()) }
            }, date.year, date.monthValue - 1, date.dayOfMonth).also { picker ->
                val zone = java.time.ZoneId.systemDefault()
                props.optString("min").take(10).let { runCatching { LocalDate.parse(it) }.getOrNull() }?.let { picker.datePicker.minDate = it.atStartOfDay(zone).toInstant().toEpochMilli() }
                props.optString("max").take(10).let { runCatching { LocalDate.parse(it) }.getOrNull() }?.let { picker.datePicker.maxDate = it.atStartOfDay(zone).toInstant().toEpochMilli() }
                picker.show()
            }
        }
    }
    private fun checkRadio(id: Int): Boolean {
        val entry = entries[id] ?: return false
        val widget = entry.widget as? RadioButton ?: return false
        if (!entry.alive() || entry.kind != "radio" || entry.updating || !widget.isEnabled || widget.isChecked) return false
        val group = entry.group?.optJSONArray("group")
        if (group != null) for (i in 0 until group.length()) {
            val other = group.getInt(i)
            if (other != id) entries[other]?.takeIf { it.kind == "radio" }?.setChecked(false)
        }
        entry.setChecked(true)
        entry.value(entry.props.optString("value", "on"))
        // Bound radios show the committed group again; unbound ones retain
        // the person's choice. Programmatic group writes emit no SDK events.
        for (i in 0 until entries.size()) entries.valueAt(i).takeIf { it.kind == "radio" }?.snapChecked()
        return true
    }
    private fun radioArrow(id: Int, forward: Boolean): Boolean {
        val entry = entries[id] ?: return false
        if (entry.widget?.isEnabled != true) return false
        val group = entry.group ?: return true
        val key = if (forward) "next" else "previous"
        if (!group.isNull(key)) {
            val target = group.getInt(key)
            entries[target]?.takeIf { it.kind == "radio" && it.widget?.isEnabled == true }?.let {
                it.widget?.requestFocus()
                checkRadio(target)
            }
        }
        return true
    }
    fun create(id: Int, props: JSONObject): View {
        check(!closed && entries[id] == null)
        return Entry(id).also { entries.put(id, it); it.configure(props, JSONObject()) }.root
    }
    fun update(id: Int, props: JSONObject, style: JSONObject) { entries[id]?.configure(props, style) }
    fun controlsChanged() {
        for (i in 0 until entries.size()) if (entries.valueAt(i).kind in setOf("button", "select", "radio")) queries.add(entries.keyAt(i))
    }
    /** Called after the publication buffer has been fully consumed. */
    fun resolve(query: (Int, Int) -> JSONObject) {
        if (radioGroupsDirty) {
            radioGroupsDirty = false
            for (i in 0 until entries.size()) if (entries.valueAt(i).kind == "radio") queries.add(entries.keyAt(i))
        }
        val pending = queries.toList(); queries.clear()
        for (id in pending) {
            val entry = entries[id] ?: continue
            val result = query(id, when (entry.kind) { "button" -> 0; "select" -> 1; "radio" -> 2; else -> continue })
            check(result.isNull("error")) { result.optString("error") }
            if (entry.kind == "button") {
                entry.face = result
                (entry.widget as Button).text = if (result.isNull("title")) "" else result.getString("title")
                if (!result.isNull("label")) entry.widget?.contentDescription = result.getString("label")
            } else if (entry.kind == "radio") {
                entry.group = result
            } else if (entry.options?.toString() != result.toString()) {
                entry.dialog?.dismiss(); entry.dialog = null
                entry.options = result; entry.picked = null
                val chosen = if (result.isNull("chosen")) null else result.getJSONArray("options").getJSONObject(result.getInt("chosen"))
                (entry.widget as Button).text = chosen?.getString("label") ?: ""
            }
            entry.measure()
        }
    }
    fun title(id: Int): String? = entries[id]?.face?.let { if (it.isNull("title")) null else it.getString("title") }
    fun action(id: Int): View? = entries[id]?.widget
    fun remove(id: Int) {
        val entry = entries[id]
        if (entry?.kind == "radio") radioGroupsDirty = true
        entries.remove(id); queries.remove(id)
        entry?.dialog?.dismiss()
    }
    internal companion object {
        fun sanitizeRange(value: Double, start: Double, end: Double, step: Double): Double {
            val clamped = value.coerceIn(start, end)
            if (step <= 0) return clamped
            var snapped = start + floor((clamped - start) / step + 0.5) * step
            if (snapped > end) snapped -= step
            val stable = if (abs(snapped) < Double.MAX_VALUE / 1e9) floor(snapped * 1e9 + 0.5) / 1e9 else snapped
            return stable.coerceIn(start, end)
        }
        fun formatRange(value: Double): String = if (value == floor(value) && abs(value) < 1e15) value.toLong().toString() else value.toString()
    }
    fun close() {
        closed = true
        for (i in 0 until entries.size()) entries.valueAt(i).dialog?.dismiss()
        entries.clear(); queries.clear()
    }
}
