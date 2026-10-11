package dev.exact.canvas

import android.R.color as system
import android.content.Context
import android.content.res.ColorStateList
import android.os.Build

/**
 * The device's colours as the app is told them (LLP 1115.001 D4): Material 3's colour roles, resolved here for this
 * device from the platform's own resources, under the light scheme and under the dark one, and told whole in one call
 * ([Native.colors]) before the host boots and again whenever one of them is not what was last told (a new wallpaper
 * changes the dynamic palette). exact2 gives each of its own colour roles one of these
 * (`host/linux/src/presenter/palette.rs`, which has D4's table). Both schemes are told whatever the night mode is:
 * exact2 chooses between them by the scheme it is told with the device's settings ([Device]).
 *
 * From Android 14 the platform has each role as a colour of its own (`android.R.color.system_primary_light` and the
 * rest), and nothing is derived. Android 12 and 13 have only the dynamic tonal palettes (`system_accent1_*`,
 * `system_accent2_*`, `system_neutral1_*`, `system_neutral2_*`, a swatch each for tones 100, 99, 95, 90 … 10, 0), so
 * there each role is the tone Material's scheme gives it ([derived]).
 */
internal object Palette {
    /** How many roles are told for each scheme. Their order is exact2's (`presenter::Material`): primary, onPrimary,
     * secondaryContainer, surface, onSurface, onSurfaceVariant, surfaceContainerLow, surfaceContainer,
     * surfaceContainerHigh, surfaceContainerHighest, outlineVariant, error. */
    const val ROLES = 12

    /** The twelve roles light, then the twelve dark, ARGB. */
    fun read(context: Context): IntArray {
        if (Build.VERSION.SDK_INT < 34) return derived(context)
        val light = intArrayOf(
            system.system_primary_light, system.system_on_primary_light, system.system_secondary_container_light,
            system.system_surface_light, system.system_on_surface_light, system.system_on_surface_variant_light,
            system.system_surface_container_low_light, system.system_surface_container_light,
            system.system_surface_container_high_light, system.system_surface_container_highest_light,
            system.system_outline_variant_light, system.system_error_light,
        )
        val dark = intArrayOf(
            system.system_primary_dark, system.system_on_primary_dark, system.system_secondary_container_dark,
            system.system_surface_dark, system.system_on_surface_dark, system.system_on_surface_variant_dark,
            system.system_surface_container_low_dark, system.system_surface_container_dark,
            system.system_surface_container_high_dark, system.system_surface_container_highest_dark,
            system.system_outline_variant_dark, system.system_error_dark,
        )
        return IntArray(ROLES * 2) { i -> context.getColor(if (i < ROLES) light[i] else dark[i - ROLES]) }
    }

    /**
     * Android 12 and 13: each role from the tonal palettes by Material 3's published tone for it (the "tonal spot"
     * scheme the system's own roles follow from Android 14): primary 40 and 80 (light and dark) and onPrimary 100 and
     * 20 of the first accent; secondaryContainer 90 and 30 of the second accent; of the neutral palette surface 98
     * and 6, onSurface 10 and 90, and the surface containers low 96 and 10, plain 94 and 12, high 92 and 17, highest
     * 90 and 22; of the neutral variant onSurfaceVariant 30 and 80 and outlineVariant 80 and 30. A tone the palette
     * has no swatch for (98, 96, 94, 92, 6, 12, 17, 22) is the palette's colour at that lightness, which the
     * platform computes ([ColorStateList.withLStar], as Material's own library does for these). `error` is no part
     * of a dynamic palette: it is Material's baseline error, tones 40 and 80.
     */
    private fun derived(context: Context): IntArray {
        val c = { id: Int -> context.getColor(id) }
        val neutral = { tone: Float -> ColorStateList.valueOf(c(system.system_neutral1_600)).withLStar(tone).defaultColor }
        return intArrayOf(
            c(system.system_accent1_600), c(system.system_accent1_0), c(system.system_accent2_100),
            neutral(98f), c(system.system_neutral1_900), c(system.system_neutral2_700),
            neutral(96f), neutral(94f), neutral(92f), c(system.system_neutral1_100),
            c(system.system_neutral2_200), 0xFFB3261E.toInt(),
            c(system.system_accent1_200), c(system.system_accent1_800), c(system.system_accent2_700),
            neutral(6f), c(system.system_neutral1_100), c(system.system_neutral2_200),
            c(system.system_neutral1_900), neutral(12f), neutral(17f), neutral(22f),
            c(system.system_neutral2_700), 0xFFF2B8B5.toInt(),
        )
    }
}
