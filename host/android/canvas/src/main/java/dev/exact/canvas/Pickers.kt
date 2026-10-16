package dev.exact.canvas

import android.app.Activity
import android.app.DatePickerDialog
import android.app.Dialog
import android.app.TimePickerDialog
import android.content.DialogInterface
import android.text.format.DateFormat
import android.view.ViewTreeObserver
import android.widget.DatePicker
import android.widget.TimePicker
import java.time.DateTimeException
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.ZoneId
import java.time.ZoneOffset
import java.util.Locale

/**
 * A date or time control's picker as it opens (LLP 1115.001.000 D1): what the control holds and allows, in HTML's
 * forms (`2026-10-16`, `14:05`, `2026-10-16T14:05`; empty where the author gave none), and what a hatch may change
 * before it shows ([ExactHatches.picker]). What a picker answers is held to HTML's rules for the kind
 * (<https://html.spec.whatwg.org/multipage/input.html#date-state-(type=date)> and the time and local date and time
 * states): within `min` and `max`, on a `step`, and written as a normalized string.
 */
class ExactPicker internal constructor(
    /** `date`, `time` or `datetime-local`. */
    val kind: String,
    val value: String,
    val min: String,
    val max: String,
    val step: String,
) {
    /** The dialog's title; null for the platform's ("Select date"). */
    var title: String? = null
    /** Open on typing the value rather than on the calendar or the clock. Material's pickers only. */
    var input = false
    /** Offer the other of the two (Material's mode toggle). Material's pickers only. */
    var modeToggle = true
    /** Whether a day may be chosen, beside [min] and [max]: a day it refuses is shown and cannot be picked. */
    var selectable: ((LocalDate) -> Boolean)? = null
    /** A 24-hour clock; null for the device's setting. */
    var hour24: Boolean? = null
    /** The hatch's own picker in place of the platform's: called with what to answer, a value in HTML's form, or
     * null to change nothing. Called once, on the main thread; it must answer once. */
    var show: ((answer: (String?) -> Unit) -> Unit)? = null

    private val held: LocalDateTime? = when (kind) {
        "date" -> parseDay(value)?.atStartOfDay()
        "time" -> parseTime(value)?.atDate(EPOCH)
        else -> parseDateTime(value)
    }
    private val low = bound(min)
    private val high = bound(max)

    /** The first and last days [min] and [max] allow, where they name one. */
    val firstDay: LocalDate? get() = low?.toLocalDate()?.takeIf { kind != "time" }
    val lastDay: LocalDate? get() = high?.toLocalDate()?.takeIf { kind != "time" }

    /** The day [value] names, else null. */
    val heldDay: LocalDate? get() = held?.toLocalDate()?.takeIf { kind != "time" }

    /** The day [value] names, else today, within [firstDay] and [lastDay]. */
    fun day(): LocalDate {
        val day = heldDay ?: LocalDate.now()
        val (first, last) = firstDay to lastDay
        return if (first != null && day < first) first else if (last != null && day > last) last else day
    }

    /** The time [value] names, its seconds included, else now to the minute. */
    fun time(): LocalTime = held?.toLocalTime()?.takeIf { kind != "date" } ?: LocalTime.now().withSecond(0).withNano(0)

    /** [hour] and [minute] as a picker that offers no seconds answers them: the value's own time where it is that
     * hour and minute, so its seconds stay, else on the minute. */
    fun time(hour: Int, minute: Int): LocalTime =
        time().takeIf { held != null && it.hour == hour && it.minute == minute } ?: LocalTime.of(hour, minute)

    /** Whether [day] may be chosen: within [min] and [max] and [selectable]. */
    fun allows(day: LocalDate): Boolean {
        val (first, last) = firstDay to lastDay
        return (first == null || day >= first) && (last == null || day <= last) && selectable?.invoke(day) != false
    }

    /** Whether [answer], a value of this kind in HTML's form, may be the control's: its day [allows], within [min] and
     * [max] (a time's range may pass midnight, `min` after `max`), and on the [step] from its base. */
    fun allows(answer: String): Boolean {
        val at = when (kind) {
            "date" -> parseDay(answer)?.atStartOfDay()
            "time" -> parseTime(answer)?.atDate(EPOCH)
            else -> parseDateTime(answer)
        } ?: return false
        if (kind != "time" && !allows(at.toLocalDate())) return false
        val inRange = if (kind == "time" && low != null && high != null && low > high) at >= low || at <= high
        else (low == null || at >= low) && (high == null || at <= high)
        if (!inRange) return false
        val step = stepMillis() ?: return true
        val base = low ?: EPOCH.atStartOfDay()
        return Math.floorMod(millis(at) - millis(base), step) == 0L
    }

    /** [day] and [time] as the control's value: a normalized string (seconds only when not zero, a fraction only
     * when not zero, in as few digits as it needs), in ASCII digits whatever the locale. */
    fun value(day: LocalDate?, time: LocalTime?): String = when (kind) {
        "date" -> day!!.toString()
        "time" -> format(time!!)
        else -> "$day" + "T" + format(time!!)
    }

    private fun bound(s: String): LocalDateTime? = when (kind) {
        "date" -> parseDay(s)?.atStartOfDay()
        "time" -> parseTime(s)?.atDate(EPOCH)
        else -> parseDateTime(s)
    }

    /** The step in milliseconds, or null for `any`. A step that is not a number above zero is the kind's default: a
     * day, or a minute. */
    private fun stepMillis(): Long? {
        if (step.equals("any", ignoreCase = true)) return null
        val scale = if (kind == "date") 86_400_000.0 else 1_000.0
        val n = step.toDoubleOrNull()?.takeIf { it.isFinite() && it > 0 } ?: if (kind == "date") 1.0 else 60.0
        return Math.round(n * scale).coerceAtLeast(1)
    }

    private fun millis(at: LocalDateTime): Long = at.toInstant(ZoneOffset.UTC).toEpochMilli()

    private fun format(t: LocalTime): String {
        val hm = String.format(Locale.ROOT, "%02d:%02d", t.hour, t.minute)
        if (t.second == 0 && t.nano == 0) return hm
        val s = String.format(Locale.ROOT, "%s:%02d", hm, t.second)
        if (t.nano == 0) return s
        return s + "." + String.format(Locale.ROOT, "%03d", t.nano / 1_000_000).trimEnd('0').ifEmpty { "0" }
    }

    companion object {
        private val EPOCH = LocalDate.of(1970, 1, 1)
        private val DAY = Regex("""(\d{4,})-(\d{2})-(\d{2})""")
        private val TIME = Regex("""(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?""")

        /** The day a valid date string names (the whole string; a year of four digits or more, not 0), else null. */
        fun parseDay(s: String): LocalDate? {
            val m = DAY.matchEntire(s) ?: return null
            val (y, mo, d) = m.destructured
            val year = y.toIntOrNull()?.takeIf { it > 0 } ?: return null
            return try { LocalDate.of(year, mo.toInt(), d.toInt()) } catch (e: DateTimeException) { null }
        }

        /** The time a valid time string names (the whole string), else null. */
        fun parseTime(s: String): LocalTime? {
            val m = TIME.matchEntire(s) ?: return null
            val (h, mi, sec, fraction) = m.destructured
            val nanos = if (fraction.isEmpty()) 0 else fraction.padEnd(3, '0').toInt() * 1_000_000
            return try { LocalTime.of(h.toInt(), mi.toInt(), sec.ifEmpty { "0" }.toInt(), nanos) } catch (e: DateTimeException) { null }
        }

        /** The day and time a valid local date and time string names, else null. */
        fun parseDateTime(s: String): LocalDateTime? {
            val t = s.indexOf('T').takeIf { it >= 0 } ?: return null
            val day = parseDay(s.substring(0, t)) ?: return null
            return parseTime(s.substring(t + 1))?.atDate(day)
        }
    }
}

/** What shows a picker: the framework's dialogs ([FrameworkPickers]), or a library's an app links (Compose Material
 * 3's, `host/android/app`'s `src/compose`). [drawn] is called at its first draw, [answer] once. What it returns
 * closes the picker, which then answers null: the activity is going. */
interface Pickers {
    fun show(activity: Activity, picker: ExactPicker, drawn: () -> Unit, answer: (String?) -> Unit): () -> Unit
}

/** The framework's `DatePickerDialog` and `TimePickerDialog`: in the boot image, so they cost the app nothing to
 * link; what an app shows when it does not link Compose. `datetime-local` asks for the day, then the time. OK is
 * enabled only for a choice the control allows ([ExactPicker.allows]). */
object FrameworkPickers : Pickers {
    override fun show(activity: Activity, picker: ExactPicker, drawn: () -> Unit, answer: (String?) -> Unit): () -> Unit {
        // The dialog up now: `datetime-local`'s time follows its day.
        lateinit var up: Dialog
        up = if (picker.kind == "time") time(activity, picker, null, drawn, answer)
        else date(activity, picker, drawn) { d ->
            when {
                d == null -> answer(null)
                picker.kind == "date" -> answer(picker.value(d, null))
                else -> up = time(activity, picker, d, {}, answer)
            }
        }
        return { up.dismiss() }
    }

    private fun date(activity: Activity, picker: ExactPicker, drawn: () -> Unit, done: (LocalDate?) -> Unit): Dialog {
        val day = picker.day()
        var chosen: LocalDate? = null
        val ok = { d: LocalDate -> if (picker.kind == "date") picker.allows(picker.value(d, null)) else picker.allows(d) }
        val dialog = object : DatePickerDialog(activity, { _, y, m, dd -> chosen = LocalDate.of(y, m + 1, dd) }, day.year, day.monthValue - 1, day.dayOfMonth) {
            override fun onDateChanged(view: DatePicker, year: Int, month: Int, dayOfMonth: Int) {
                super.onDateChanged(view, year, month, dayOfMonth)
                getButton(DialogInterface.BUTTON_POSITIVE)?.isEnabled = ok(LocalDate.of(year, month + 1, dayOfMonth))
            }
        }
        // The framework's picker counts its bounds in the device's zone.
        val local = { d: LocalDate -> d.atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli() }
        picker.firstDay?.let { dialog.datePicker.minDate = local(it) }
        picker.lastDay?.let { dialog.datePicker.maxDate = local(it) }
        picker.title?.let { dialog.setTitle(it) }
        open(dialog, drawn) { done(chosen) }
        dialog.getButton(DialogInterface.BUTTON_POSITIVE)?.isEnabled = ok(day)
        return dialog
    }

    private fun time(activity: Activity, picker: ExactPicker, day: LocalDate?, drawn: () -> Unit, done: (String?) -> Unit): Dialog {
        val t = picker.time()
        var chosen: LocalTime? = null
        val ok = { h: Int, m: Int -> picker.allows(picker.value(day, picker.time(h, m))) }
        val clock = picker.hour24 ?: DateFormat.is24HourFormat(activity)
        val dialog = object : TimePickerDialog(activity, { _, h, m -> chosen = picker.time(h, m) }, t.hour, t.minute, clock) {
            override fun onTimeChanged(view: TimePicker, hourOfDay: Int, minute: Int) {
                super.onTimeChanged(view, hourOfDay, minute)
                getButton(DialogInterface.BUTTON_POSITIVE)?.isEnabled = ok(hourOfDay, minute)
            }
        }
        picker.title?.let { dialog.setTitle(it) }
        open(dialog, drawn) { done(chosen?.let { picker.value(day, it) }) }
        dialog.getButton(DialogInterface.BUTTON_POSITIVE)?.isEnabled = ok(t.hour, t.minute)
        return dialog
    }

    /** Show [d]; [closed] once it goes, by a choice or a dismissal. */
    private fun open(d: Dialog, drawn: () -> Unit, closed: () -> Unit) {
        d.setOnDismissListener { closed() }
        d.create()
        var told = false
        d.window!!.decorView.viewTreeObserver.addOnDrawListener(ViewTreeObserver.OnDrawListener { if (!told) { told = true; drawn() } })
        d.show()
    }
}
