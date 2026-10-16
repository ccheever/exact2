package dev.exact.canvas

import android.app.Activity
import android.app.DatePickerDialog
import android.app.Dialog
import android.app.TimePickerDialog
import android.text.format.DateFormat
import android.view.ViewTreeObserver
import java.time.LocalDate
import java.time.LocalTime

/**
 * A date or time control's picker as it opens (LLP 1115.001.000 D1): what the control holds and allows, in HTML's
 * forms (`2026-10-16`, `14:05`, `2026-10-16T14:05`; empty where the author gave none), and what a hatch may change
 * before it shows ([ExactHatches.picker]).
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
    /** Open on typing the value rather than on the calendar or the clock. */
    var input = false
    /** Offer the other of the two (Material's mode toggle). */
    var modeToggle = true
    /** Whether a day may be chosen, beside [min] and [max]: a day it refuses is shown and cannot be picked. */
    var selectable: ((LocalDate) -> Boolean)? = null
    /** A 24-hour clock; null for the device's setting. */
    var hour24: Boolean? = null
    /** The hatch's own picker in place of the platform's: called with what to answer, a value in HTML's form, or
     * null to change nothing. Called once, on the main thread; it must answer once. */
    var show: ((answer: (String?) -> Unit) -> Unit)? = null

    /** The day [value] names, else today. */
    fun day(): LocalDate = parseDay(value) ?: LocalDate.now()
    /** The time [value] names, else now. */
    fun time(): LocalTime = parseTime(value) ?: LocalTime.now().withSecond(0).withNano(0)
    /** Whether [day] may be chosen: within [min] and [max] and [selectable]. */
    fun allows(day: LocalDate): Boolean =
        parseDay(min)?.let { day >= it } != false && parseDay(max)?.let { day <= it } != false && selectable?.invoke(day) != false

    /** [day] and [time] as the control's value. */
    fun value(day: LocalDate?, time: LocalTime?): String = when (kind) {
        "date" -> day.toString()
        "time" -> format(time!!)
        else -> "$day" + "T" + format(time!!)
    }

    /** Seconds only where the step asks for them, as a browser's picker writes. */
    private fun format(t: LocalTime): String = if ((step.toDoubleOrNull() ?: 60.0) % 60.0 != 0.0) "%02d:%02d:%02d".format(t.hour, t.minute, t.second) else "%02d:%02d".format(t.hour, t.minute)

    companion object {
        /** The day an HTML date or `datetime-local` value names, else null. */
        fun parseDay(s: String): LocalDate? = try { LocalDate.parse(s.take(10)) } catch (e: RuntimeException) { null }
        /** The time an HTML time or `datetime-local` value names, else null. */
        fun parseTime(s: String): LocalTime? = try { LocalTime.parse(s.substringAfter('T')) } catch (e: RuntimeException) { null }
    }
}

/** What shows a picker: the framework's dialogs ([FrameworkPickers]), or a library's an app links (Compose Material
 * 3's, `host/android/app`'s `src/compose`). [drawn] is called at its first draw, [answer] once. */
interface Pickers {
    fun show(activity: Activity, picker: ExactPicker, drawn: () -> Unit, answer: (String?) -> Unit)
}

/** The framework's `DatePickerDialog` and `TimePickerDialog`: in the boot image, so they cost the app nothing to
 * link; what an app shows when it does not link Compose. `datetime-local` asks for the day, then the time. */
object FrameworkPickers : Pickers {
    override fun show(activity: Activity, picker: ExactPicker, drawn: () -> Unit, answer: (String?) -> Unit) {
        if (picker.kind == "time") time(activity, picker, drawn) { t -> answer(t?.let { picker.value(null, it) }) }
        else date(activity, picker, drawn) { d ->
            when {
                d == null -> answer(null)
                picker.kind == "date" -> answer(picker.value(d, null))
                else -> time(activity, picker, {}) { t -> answer(t?.let { picker.value(d, it) }) }
            }
        }
    }

    private fun date(activity: Activity, picker: ExactPicker, drawn: () -> Unit, done: (LocalDate?) -> Unit) {
        val day = picker.day()
        var chosen: LocalDate? = null
        val d = DatePickerDialog(activity, { _, y, m, dd -> chosen = LocalDate.of(y, m + 1, dd) }, day.year, day.monthValue - 1, day.dayOfMonth)
        // The framework's picker counts its bounds in the device's zone.
        val local = { day: LocalDate -> day.atStartOfDay(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli() }
        ExactPicker.parseDay(picker.min)?.let { d.datePicker.minDate = local(it) }
        ExactPicker.parseDay(picker.max)?.let { d.datePicker.maxDate = local(it) }
        picker.title?.let { d.setTitle(it) }
        open(d, drawn) { done(chosen) }
    }

    private fun time(activity: Activity, picker: ExactPicker, drawn: () -> Unit, done: (LocalTime?) -> Unit) {
        val t = picker.time()
        var chosen: LocalTime? = null
        val d = TimePickerDialog(activity, { _, h, m -> chosen = LocalTime.of(h, m) }, t.hour, t.minute, picker.hour24 ?: DateFormat.is24HourFormat(activity))
        picker.title?.let { d.setTitle(it) }
        open(d, drawn) { done(chosen) }
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
