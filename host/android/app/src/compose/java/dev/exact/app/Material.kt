package dev.exact.app

import android.app.Activity
import android.content.res.Configuration
import android.text.format.DateFormat
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DisplayMode
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimeInput
import androidx.compose.material3.TimePicker
import androidx.compose.material3.TimePickerDialog
import androidx.compose.material3.TimePickerDialogDefaults
import androidx.compose.material3.TimePickerDisplayMode
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.setViewTreeLifecycleOwner
import androidx.savedstate.SavedStateRegistryController
import androidx.savedstate.SavedStateRegistryOwner
import androidx.savedstate.setViewTreeSavedStateRegistryOwner
import dev.exact.canvas.ExactPicker
import dev.exact.canvas.Pickers
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneOffset

/** The plan has a date or time control: its picker is Compose Material 3's (LLP 1115.001.000 D1). */
internal val materialPickers: Pickers? = MaterialPickers

/**
 * Material 3's `DatePickerDialog` and `TimePickerDialog`, as a Compose app shows them, on the device's dynamic
 * colours. Each opens in a `ComposeView` added to the activity for as long as it is up; the dialog is a window of
 * its own above the page. `datetime-local` asks for the day, then the time.
 */
private object MaterialPickers : Pickers {
    /** The reader's activity is a plain Activity: Compose looks for a lifecycle and a saved state on the view tree. */
    private class Owner : SavedStateRegistryOwner {
        private val registry = LifecycleRegistry(this)
        private val saved = SavedStateRegistryController.create(this)
        override val lifecycle: Lifecycle get() = registry
        override val savedStateRegistry get() = saved.savedStateRegistry
        init {
            saved.performRestore(null)
            registry.currentState = Lifecycle.State.RESUMED
        }
    }

    private val owner by lazy { Owner() }

    override fun show(activity: Activity, picker: ExactPicker, drawn: () -> Unit, answer: (String?) -> Unit) {
        activity.window.decorView.setViewTreeLifecycleOwner(owner)
        activity.window.decorView.setViewTreeSavedStateRegistryOwner(owner)
        val view = ComposeView(activity)
        var told = false
        val first = { if (!told) { told = true; drawn() } }
        view.setContent {
            Theme(activity) {
                Picker(picker, first) { chosen ->
                    (view.parent as? ViewGroup)?.removeView(view)
                    answer(chosen)
                }
            }
        }
        activity.addContentView(view, FrameLayout.LayoutParams(1, 1))
    }
}

@Composable
private fun Theme(activity: Activity, content: @Composable () -> Unit) {
    val dark = activity.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
    MaterialTheme(colorScheme = if (dark) dynamicDarkColorScheme(activity) else dynamicLightColorScheme(activity), content = content)
}

@Composable
private fun Picker(picker: ExactPicker, drawn: () -> Unit, done: (String?) -> Unit) {
    var day by remember { mutableStateOf<LocalDate?>(null) }
    if (picker.kind == "time" || day != null) {
        TimeDialog(picker, drawn) { t -> done(t?.let { picker.value(day, it) }) }
    } else {
        DateDialog(picker, drawn) { d ->
            when {
                d == null -> done(null)
                picker.kind == "date" -> done(picker.value(d, null))
                else -> day = d
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DateDialog(picker: ExactPicker, drawn: () -> Unit, done: (LocalDate?) -> Unit) {
    val min = ExactPicker.parseDay(picker.min)
    val max = ExactPicker.parseDay(picker.max)
    val state = rememberDatePickerState(
        initialSelectedDateMillis = ExactPicker.parseDay(picker.value)?.let(::utc),
        initialDisplayedMonthMillis = utc(picker.day()),
        yearRange = (min?.year ?: DatePickerDefaults.YearRange.first)..(max?.year ?: DatePickerDefaults.YearRange.last),
        initialDisplayMode = if (picker.input) DisplayMode.Input else DisplayMode.Picker,
        selectableDates = object : SelectableDates {
            override fun isSelectableDate(utcTimeMillis: Long) = picker.allows(Instant.ofEpochMilli(utcTimeMillis).atZone(ZoneOffset.UTC).toLocalDate())
        },
    )
    val chosen = state.selectedDateMillis?.let { Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate() }
    DatePickerDialog(
        onDismissRequest = { done(null) },
        confirmButton = { TextButton(onClick = { done(chosen) }, enabled = chosen != null) { Text(stringResource(android.R.string.ok)) } },
        dismissButton = { TextButton(onClick = { done(null) }) { Text(stringResource(android.R.string.cancel)) } },
    ) {
        val title = picker.title
        DatePicker(
            state = state,
            modifier = Modifier.drawWithContent { drawContent(); drawn() },
            title = if (title == null) {
                { DatePickerDefaults.DatePickerTitle(state.displayMode, Modifier.padding(TitlePadding)) }
            } else {
                { Text(title, Modifier.padding(TitlePadding)) }
            },
            showModeToggle = picker.modeToggle,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TimeDialog(picker: ExactPicker, drawn: () -> Unit, done: (LocalTime?) -> Unit) {
    val t = picker.time()
    val state = rememberTimePickerState(t.hour, t.minute, picker.hour24 ?: DateFormat.is24HourFormat(LocalContext.current))
    var mode by remember { mutableStateOf(if (picker.input) TimePickerDisplayMode.Input else TimePickerDisplayMode.Picker) }
    val title = picker.title
    TimePickerDialog(
        onDismissRequest = { done(null) },
        confirmButton = { TextButton(onClick = { done(LocalTime.of(state.hour, state.minute)) }) { Text(stringResource(android.R.string.ok)) } },
        title = { if (title == null) TimePickerDialogDefaults.Title(mode) else Text(title) },
        modeToggleButton = if (picker.modeToggle) {
            {
                TimePickerDialogDefaults.DisplayModeToggle(
                    onDisplayModeChange = { mode = if (mode == TimePickerDisplayMode.Picker) TimePickerDisplayMode.Input else TimePickerDisplayMode.Picker },
                    displayMode = mode,
                )
            }
        } else null,
        dismissButton = { TextButton(onClick = { done(null) }) { Text(stringResource(android.R.string.cancel)) } },
    ) {
        val drawing = Modifier.drawWithContent { drawContent(); drawn() }
        if (mode == TimePickerDisplayMode.Picker) TimePicker(state, drawing) else TimeInput(state, drawing)
    }
}

/** Where Material's own date picker places its title (`DatePickerDefaults`' padding, 1.4.0). */
private val TitlePadding = PaddingValues(start = 24.dp, end = 12.dp, top = 16.dp)

private fun utc(day: LocalDate): Long = day.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
