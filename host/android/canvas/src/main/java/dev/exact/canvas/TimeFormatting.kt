package dev.exact.canvas

import android.icu.text.DisplayContext
import android.icu.text.MeasureFormat
import android.icu.text.RelativeDateTimeFormatter
import android.icu.util.Measure
import android.icu.util.MeasureUnit
import android.icu.util.ULocale
import android.text.format.DateUtils
import java.text.DateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.max

/** Platform presentations evaluated against the runner's reference date. */
object TimeFormatting {
    private val relative = mutableMapOf<String, RelativeDateTimeFormatter>()
    private val measures = mutableMapOf<String, MeasureFormat>()
    private fun relative(language: String, wide: Boolean = false): RelativeDateTimeFormatter =
        relative.getOrPut("$language/$wide") {
            RelativeDateTimeFormatter.getInstance(ULocale.forLanguageTag(language), null,
                if (wide) RelativeDateTimeFormatter.Style.LONG else RelativeDateTimeFormatter.Style.SHORT,
                DisplayContext.CAPITALIZATION_NONE)
        }
    private fun measure(language: String, wide: Boolean): MeasureFormat =
        measures.getOrPut("$language/$wide") { MeasureFormat.getInstance(ULocale.forLanguageTag(language),
            if (wide) MeasureFormat.FormatWidth.WIDE else MeasureFormat.FormatWidth.SHORT) }

    @JvmStatic fun format(datetime: Double, now: Double, format: String, language: String, zone: String, n: Double, unit: Int): String? {
        if (!datetime.isFinite() || datetime < -62135596800000.0 || datetime > 253402300799999.0) return null
        return when (format) {
            "relative-reservation" -> {
                val phrase = measure(language, false).formatMeasures(Measure(59, MeasureUnit.MINUTE), Measure(59, MeasureUnit.SECOND))
                val minute = measure(language, true).formatMeasures(Measure(59, MeasureUnit.MINUTE))
                val wrapper = relative(language, true).format(59.0,
                    if (now > datetime) RelativeDateTimeFormatter.Direction.LAST else RelativeDateTimeFormatter.Direction.NEXT,
                    RelativeDateTimeFormatter.RelativeUnit.MINUTES)
                if (wrapper.contains(minute)) wrapper.replace(minute, phrase) else null
            }
            "relative" -> if (n == 0.0) {
                relative(language).format(RelativeDateTimeFormatter.Direction.PLAIN, RelativeDateTimeFormatter.AbsoluteUnit.NOW)
            } else {
                val units = arrayOf(RelativeDateTimeFormatter.RelativeUnit.MINUTES, RelativeDateTimeFormatter.RelativeUnit.HOURS,
                    RelativeDateTimeFormatter.RelativeUnit.DAYS, RelativeDateTimeFormatter.RelativeUnit.MONTHS, RelativeDateTimeFormatter.RelativeUnit.YEARS)
                if (unit !in units.indices) return null
                relative(language).format(abs(n), if (n < 0) RelativeDateTimeFormatter.Direction.LAST else RelativeDateTimeFormatter.Direction.NEXT, units[unit])
            }
            "timer", "countdown" -> {
                val seconds = if (format == "countdown") ceil(max(0.0, datetime - now) / 1000) else floor(abs(datetime - now) / 1000)
                DateUtils.formatElapsedTime(seconds.toLong())
            }
            else -> {
                val parts = format.split("-", limit = 2)
                if (parts.size != 2) return null
                val style = when (parts[1]) { "short" -> DateFormat.SHORT; "medium" -> DateFormat.MEDIUM; "long" -> DateFormat.LONG; "full" -> DateFormat.FULL; else -> return null }
                val locale = Locale.forLanguageTag(language)
                val formatter = when (parts[0]) { "date" -> DateFormat.getDateInstance(style, locale); "time" -> DateFormat.getTimeInstance(style, locale); else -> return null }
                formatter.timeZone = TimeZone.getTimeZone(zone)
                formatter.format(Date(datetime.toLong()))
            }
        }
    }
}
