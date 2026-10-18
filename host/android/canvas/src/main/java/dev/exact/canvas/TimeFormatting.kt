package dev.exact.canvas

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
    @JvmStatic fun format(datetime: Double, now: Double, format: String, language: String, zone: String): String? {
        if (!datetime.isFinite() || datetime < -62135596800000.0 || datetime > 253402300799999.0) return null
        return when (format) {
            "relative-reservation" -> {
                val sign = if (datetime < now) -1 else 1
                DateUtils.getRelativeTimeSpanString(now.toLong() + sign * 59 * 60000L, now.toLong(), DateUtils.MINUTE_IN_MILLIS, DateUtils.FORMAT_ABBREV_RELATIVE).toString() + ", " + DateUtils.getRelativeTimeSpanString(now.toLong() + sign * 59000L, now.toLong(), DateUtils.SECOND_IN_MILLIS, DateUtils.FORMAT_ABBREV_RELATIVE).toString()
            }
            "relative" -> DateUtils.getRelativeTimeSpanString(datetime.toLong(), now.toLong(), DateUtils.MINUTE_IN_MILLIS, DateUtils.FORMAT_ABBREV_RELATIVE).toString()
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
