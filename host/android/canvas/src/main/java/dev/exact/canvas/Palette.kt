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
 * there each role is the tone Material's scheme gives it ([derived]). Before Android 12 there is no dynamic colour, and
 * the roles are Material's baseline scheme ([Legacy.palette]).
 */
internal object Palette {
    /** How many roles are told for each scheme: every role of Material's `ColorScheme`, in exact2's order
     * (`presenter::Material`): the twelve D4 maps exact2's own roles to first, then the rest, which the Material look
     * reads by its own names (LLP 1115.001.001 D2.3). */
    const val ROLES = 36

    /** The roles light, then dark, ARGB. */
    fun read(context: Context): IntArray {
        if (Build.VERSION.SDK_INT < 31) return Legacy.palette()
        if (Build.VERSION.SDK_INT < 34) return derived(context)
        val light = intArrayOf(
            system.system_primary_light, system.system_on_primary_light, system.system_secondary_container_light,
            system.system_surface_light, system.system_on_surface_light, system.system_on_surface_variant_light,
            system.system_surface_container_low_light, system.system_surface_container_light,
            system.system_surface_container_high_light, system.system_surface_container_highest_light,
            system.system_outline_variant_light, system.system_error_light,
            system.system_primary_container_light, system.system_on_primary_container_light,
            system.system_inverse_primary_light, system.system_secondary_light, system.system_on_secondary_light,
            system.system_on_secondary_container_light, system.system_tertiary_light, system.system_on_tertiary_light,
            system.system_tertiary_container_light, system.system_on_tertiary_container_light,
            system.system_background_light, system.system_on_background_light, system.system_surface_variant_light,
            PRIMARY, system.system_inverse_surface_light, system.system_inverse_on_surface_light,
            system.system_on_error_light, system.system_error_container_light, system.system_on_error_container_light,
            system.system_outline_light, SCRIM, system.system_surface_bright_light, system.system_surface_dim_light,
            system.system_surface_container_lowest_light,
        )
        val dark = intArrayOf(
            system.system_primary_dark, system.system_on_primary_dark, system.system_secondary_container_dark,
            system.system_surface_dark, system.system_on_surface_dark, system.system_on_surface_variant_dark,
            system.system_surface_container_low_dark, system.system_surface_container_dark,
            system.system_surface_container_high_dark, system.system_surface_container_highest_dark,
            system.system_outline_variant_dark, system.system_error_dark,
            system.system_primary_container_dark, system.system_on_primary_container_dark,
            system.system_inverse_primary_dark, system.system_secondary_dark, system.system_on_secondary_dark,
            system.system_on_secondary_container_dark, system.system_tertiary_dark, system.system_on_tertiary_dark,
            system.system_tertiary_container_dark, system.system_on_tertiary_container_dark,
            system.system_background_dark, system.system_on_background_dark, system.system_surface_variant_dark,
            PRIMARY, system.system_inverse_surface_dark, system.system_inverse_on_surface_dark,
            system.system_on_error_dark, system.system_error_container_dark, system.system_on_error_container_dark,
            system.system_outline_dark, SCRIM, system.system_surface_bright_dark, system.system_surface_dim_dark,
            system.system_surface_container_lowest_dark,
        )
        // `surfaceTint` is `primary` and `scrim` black, as Material's schemes have them: neither is read as a
        // resource, since the platform did not add both with the others.
        val color = { ids: IntArray, i: Int ->
            when (ids[i]) {
                PRIMARY -> context.getColor(ids[0])
                SCRIM -> 0xFF000000.toInt()
                else -> context.getColor(ids[i])
            }
        }
        return IntArray(ROLES * 2) { i -> if (i < ROLES) color(light, i) else color(dark, i - ROLES) }
    }

    /** Markers in [read]'s tables for a role that is no resource of its own. */
    private const val PRIMARY = 0
    private const val SCRIM = 1

    /**
     * Android 12 and 13: each role from the tonal palettes by Material 3's published tone for it (the "tonal spot"
     * scheme the system's own roles follow from Android 14), light and then dark. Of the first accent: primary 40 and
     * 80, onPrimary 100 and 20, primaryContainer 90 and 30, onPrimaryContainer 10 and 90, inversePrimary 80 and 40.
     * Of the second: secondary 40 and 80, onSecondary 100 and 20, secondaryContainer 90 and 30, onSecondaryContainer
     * 10 and 90; the third accent likewise for tertiary. Of the neutral palette: surface and background 98 and 6,
     * onSurface and onBackground 10 and 90, the surface containers lowest 100 and 4, low 96 and 10, plain 94 and 12,
     * high 92 and 17, highest 90 and 22, surfaceBright 98 and 24, surfaceDim 87 and 6, inverseSurface 20 and 90,
     * inverseOnSurface 95 and 20. Of the neutral variant: surfaceVariant 90 and 30, onSurfaceVariant 30 and 80,
     * outline 50 and 60, outlineVariant 80 and 30. A tone the palette has no swatch for (98, 96, 94, 92, 87, 24, 22,
     * 17, 12, 6, 4) is the palette's colour at that lightness, which the platform computes
     * ([ColorStateList.withLStar], as Material's own library does for these). The error roles are no part of a
     * dynamic palette: they are Material's baseline ones. `surfaceTint` is `primary`, and `scrim` black.
     */
    @android.annotation.TargetApi(31)
    private fun derived(context: Context): IntArray {
        val c = { id: Int -> context.getColor(id) }
        val neutral = { tone: Float -> ColorStateList.valueOf(c(system.system_neutral1_600)).withLStar(tone).defaultColor }
        val baseline = Legacy.palette()
        val error = { i: Int, dark: Boolean -> baseline[(if (dark) ROLES else 0) + i] }
        return intArrayOf(
            c(system.system_accent1_600), c(system.system_accent1_0), c(system.system_accent2_100),
            neutral(98f), c(system.system_neutral1_900), c(system.system_neutral2_700),
            neutral(96f), neutral(94f), neutral(92f), c(system.system_neutral1_100),
            c(system.system_neutral2_200), error(11, false),
            c(system.system_accent1_100), c(system.system_accent1_900), c(system.system_accent1_200),
            c(system.system_accent2_600), c(system.system_accent2_0), c(system.system_accent2_900),
            c(system.system_accent3_600), c(system.system_accent3_0), c(system.system_accent3_100),
            c(system.system_accent3_900), neutral(98f), c(system.system_neutral1_900), c(system.system_neutral2_100),
            c(system.system_accent1_600), c(system.system_neutral1_800), c(system.system_neutral1_50),
            error(28, false), error(29, false), error(30, false), c(system.system_neutral2_500), 0xFF000000.toInt(),
            neutral(98f), neutral(87f), c(system.system_neutral1_0),
            c(system.system_accent1_200), c(system.system_accent1_800), c(system.system_accent2_700),
            neutral(6f), c(system.system_neutral1_100), c(system.system_neutral2_200),
            c(system.system_neutral1_900), neutral(12f), neutral(17f), neutral(22f),
            c(system.system_neutral2_700), error(11, true),
            c(system.system_accent1_700), c(system.system_accent1_100), c(system.system_accent1_600),
            c(system.system_accent2_200), c(system.system_accent2_800), c(system.system_accent2_100),
            c(system.system_accent3_200), c(system.system_accent3_800), c(system.system_accent3_700),
            c(system.system_accent3_100), neutral(6f), c(system.system_neutral1_100), c(system.system_neutral2_700),
            c(system.system_accent1_200), c(system.system_neutral1_100), c(system.system_neutral1_800),
            error(28, true), error(29, true), error(30, true), c(system.system_neutral2_400), 0xFF000000.toInt(),
            neutral(24f), neutral(6f), neutral(4f),
        )
    }
}
