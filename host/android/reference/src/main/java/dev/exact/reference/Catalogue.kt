package dev.exact.reference

import android.os.Bundle
import android.util.Log
import android.view.WindowInsets
import android.view.WindowInsetsController
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.toArgb

/**
 * The catalogue on a phone (LLP 1115.001.001 D4): one specimen of `specimens.json`, `--es id <id> --es scheme
 * light|dark`, drawn as `DrawTest` draws it on the JVM, over the whole screen with the system bars hidden, so that a
 * screenshot holds only the specimen. Each launch logs the phone's two dynamic schemes (tag `reference`), which the
 * reader shows Exact's side in, for the door to be told them.
 */
class Catalogue : ComponentActivity() {
    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        val id = intent.getStringExtra("id") ?: "text-field-filled"
        val scheme = if (intent.getStringExtra("scheme") == "dark") darkColorScheme() else lightColorScheme()
        for ((name, s) in listOf("light" to dynamicLightColorScheme(this), "dark" to dynamicDarkColorScheme(this))) {
            Log.i("reference", "dynamic $name " + roles(s).joinToString(",") { (k, v) -> "$k=${v.toArgb().toLong() and 0xFFFFFFFFL}" })
        }
        setContent { Specimen(id, scheme) }
        window.setDecorFitsSystemWindows(false)
        window.insetsController?.apply {
            hide(WindowInsets.Type.systemBars())
            systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }
}

/** Every role of [s], by Material's name. */
fun roles(s: ColorScheme) = listOf(
    "primary" to s.primary, "onPrimary" to s.onPrimary, "primaryContainer" to s.primaryContainer,
    "onPrimaryContainer" to s.onPrimaryContainer, "inversePrimary" to s.inversePrimary,
    "secondary" to s.secondary, "onSecondary" to s.onSecondary, "secondaryContainer" to s.secondaryContainer,
    "onSecondaryContainer" to s.onSecondaryContainer, "tertiary" to s.tertiary, "onTertiary" to s.onTertiary,
    "tertiaryContainer" to s.tertiaryContainer, "onTertiaryContainer" to s.onTertiaryContainer,
    "background" to s.background, "onBackground" to s.onBackground, "surface" to s.surface,
    "onSurface" to s.onSurface, "surfaceVariant" to s.surfaceVariant, "onSurfaceVariant" to s.onSurfaceVariant,
    "surfaceTint" to s.surfaceTint, "inverseSurface" to s.inverseSurface, "inverseOnSurface" to s.inverseOnSurface,
    "error" to s.error, "onError" to s.onError, "errorContainer" to s.errorContainer,
    "onErrorContainer" to s.onErrorContainer, "outline" to s.outline, "outlineVariant" to s.outlineVariant,
    "scrim" to s.scrim, "surfaceBright" to s.surfaceBright, "surfaceDim" to s.surfaceDim,
    "surfaceContainerLowest" to s.surfaceContainerLowest, "surfaceContainerLow" to s.surfaceContainerLow,
    "surfaceContainer" to s.surfaceContainer, "surfaceContainerHigh" to s.surfaceContainerHigh,
    "surfaceContainerHighest" to s.surfaceContainerHighest,
)
