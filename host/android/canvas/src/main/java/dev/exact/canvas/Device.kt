package dev.exact.canvas

import android.app.UiModeManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.view.View
import android.view.accessibility.AccessibilityManager

/**
 * The device's settings as the app is told them (LLP 1115.001 D3.1): read whole, and told whole in one call
 * ([Native.configure]) before the host boots and again whenever one of them is not what was last told. exact2 does
 * with each what it has a door for (`host/linux/src/presenter/device.rs`).
 */
internal data class Device(
    /** Night mode. */
    val night: Boolean,
    /** The person's font scale (Settings, Display size and text). */
    val fontScale: Float,
    /** The first of the device's locales, a BCP 47 tag, and whether it is laid out right to left. */
    val locale: String,
    val rtl: Boolean,
    /** The 24-hour clock. */
    val hour24: Boolean,
    /** "Remove animations": the animator duration scale is 0. */
    val noAnimations: Boolean,
    /** Bold text: a font weight adjustment above 0. */
    val boldText: Boolean,
    /** High contrast: the system's contrast level above standard (Android 14), or high contrast text (Android 16). */
    val highContrast: Boolean,
    /** The screen is a finger's, and how far one moves before its touch is a drag, in dp (the system's touch slop). */
    val touch: Boolean,
    val touchSlop: Float,
) {
    /** Tell exact2: on the thread that holds the host, or on any before it boots. */
    fun tell() = Native.configure(night, fontScale, locale, rtl, hour24, noAnimations, boldText, highContrast, touch, touchSlop)

    companion object {
        fun read(context: Context): Device {
            val config = context.resources.configuration
            val weight = config.fontWeightAdjustment
            var contrast = false
            if (Build.VERSION.SDK_INT >= 34) contrast = (context.getSystemService(UiModeManager::class.java)?.contrast ?: 0f) > 0f
            if (Build.VERSION.SDK_INT >= 36) contrast = contrast || context.getSystemService(AccessibilityManager::class.java)?.isHighContrastTextEnabled == true
            return Device(
                night = config.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES,
                fontScale = config.fontScale,
                locale = config.locales[0].toLanguageTag(),
                rtl = config.layoutDirection == View.LAYOUT_DIRECTION_RTL,
                hour24 = android.text.format.DateFormat.is24HourFormat(context),
                noAnimations = !android.animation.ValueAnimator.areAnimatorsEnabled(),
                boldText = weight != Configuration.FONT_WEIGHT_ADJUSTMENT_UNDEFINED && weight > 0,
                highContrast = contrast,
                touch = config.touchscreen == Configuration.TOUCHSCREEN_FINGER,
                touchSlop = android.view.ViewConfiguration.get(context).scaledTouchSlop / context.resources.displayMetrics.density,
            )
        }
    }
}
