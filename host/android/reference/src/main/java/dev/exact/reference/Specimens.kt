package dev.exact.reference

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp

/** The tag on what a specimen's layout box is: the component, or a box tight round it. */
const val SPECIMEN = "specimen"

/**
 * One specimen of `specimens.json` as a hand-built app writes it: Material's default component, in its default
 * theme on [scheme]. Each has a twin in `apps/material-specimens/specimens/<id>.contract`; a `defect-` id is the
 * specimen it names with one error planted, for the oracle's detection test (LLP 1115.001.001 O1).
 */
@Composable
fun Specimen(id: String, scheme: ColorScheme) {
    MaterialTheme(colorScheme = scheme) {
        Surface(Modifier.fillMaxSize()) {
            when (id) {
                "top-app-bar" -> TopBar("Library", up = false)
                "top-app-bar-up" -> TopBar("Detail", up = true)
                "navigation-bar", "defect-label-low" -> Box(Modifier.fillMaxSize()) {
                    NavBar(Modifier.align(Alignment.BottomCenter), lowLabel = id == "defect-label-low")
                }
                else -> Box(Modifier.padding(16.dp)) { Box(Modifier.testTag(SPECIMEN)) { Inline(id) } }
            }
        }
    }
}

@Composable
private fun Inline(id: String) {
    when (id) {
        "text-field-empty" -> Field("")
        "text-field-filled" -> Field("Moby-Dick")
        "text-field-disabled" -> Field("Moby-Dick", enabled = false)
        "text-field-focused" -> Field("Moby-Dick", focused = true)
        "filled-button" -> Button(onClick = {}) { Text("Save") }
        "filled-button-disabled" -> Button(onClick = {}, enabled = false) { Text("Save") }
        "checkbox-checked" -> Checkbox(checked = true, onCheckedChange = {})
        "checkbox-unchecked" -> Checkbox(checked = false, onCheckedChange = {})
        "defect-missing-control" -> Box(Modifier.size(48.dp))
        "defect-neighbour-role" -> Button(
            onClick = {},
            colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.secondary),
        ) { Text("Save") }
        "defect-corner" -> Field("Moby-Dick", shape = RoundedCornerShape(topStart = 6.dp, topEnd = 6.dp))
        "defect-thin-indicator" -> ThinField("Moby-Dick")
        "defect-label-colour" -> Field(
            "Moby-Dick",
            colors = TextFieldDefaults.colors(unfocusedLabelColor = MaterialTheme.colorScheme.tertiary),
        )
        "defect-label-missing" -> Field("Moby-Dick", label = null)
        "defect-value-colour" -> Field(
            "Moby-Dick",
            colors = TextFieldDefaults.colors(unfocusedTextColor = MaterialTheme.colorScheme.tertiary),
        )
        else -> error("no specimen $id")
    }
}

@Composable
private fun Field(
    value: String,
    enabled: Boolean = true,
    focused: Boolean = false,
    shape: androidx.compose.ui.graphics.Shape = TextFieldDefaults.shape,
    colors: androidx.compose.material3.TextFieldColors = TextFieldDefaults.colors(),
    label: String? = "Title",
) {
    val focus = remember { FocusRequester() }
    TextField(
        value = value,
        onValueChange = {},
        modifier = Modifier.width(280.dp).focusRequester(focus),
        enabled = enabled,
        singleLine = true,
        label = label?.let { { Text(it) } },
        shape = shape,
        colors = colors,
    )
    if (focused) LaunchedEffect(Unit) { focus.requestFocus() }
}

/** A focused filled field whose indicator is 1 dp where Material's is 2: `TextField` built from its own parts. */
@Composable
private fun ThinField(value: String) {
    val focus = remember { FocusRequester() }
    val interaction = remember { MutableInteractionSource() }
    val colors = TextFieldDefaults.colors()
    BasicTextField(
        value = value,
        onValueChange = {},
        modifier = Modifier.width(280.dp).defaultMinSize(minHeight = TextFieldDefaults.MinHeight).focusRequester(focus),
        textStyle = LocalTextStyle.current.copy(color = MaterialTheme.colorScheme.onSurface),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        interactionSource = interaction,
        singleLine = true,
    ) { inner ->
        TextFieldDefaults.DecorationBox(
            value = value,
            innerTextField = inner,
            enabled = true,
            singleLine = true,
            visualTransformation = VisualTransformation.None,
            interactionSource = interaction,
            label = { Text("Title") },
            container = {
                TextFieldDefaults.Container(
                    enabled = true,
                    isError = false,
                    interactionSource = interaction,
                    colors = colors,
                    focusedIndicatorLineThickness = 1.dp,
                )
            },
        )
    }
    LaunchedEffect(Unit) { focus.requestFocus() }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TopBar(title: String, up: Boolean) {
    // The tag on a box round the bar: on the bar itself it lands on a node the window's height.
    Box(Modifier.fillMaxWidth().wrapContentHeight(Alignment.Top).testTag(SPECIMEN)) {
        TopAppBar(
            title = { Text(title) },
            navigationIcon = {
                if (up) IconButton(onClick = {}) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") }
            },
            windowInsets = WindowInsets(0),
        )
    }
}

@Composable
private fun NavBar(modifier: Modifier, lowLabel: Boolean) {
    NavigationBar(modifier.testTag(SPECIMEN), windowInsets = WindowInsets(0)) {
        val items = listOf("Home" to Icons.Filled.Home, "Starred" to Icons.Filled.Star, "Settings" to Icons.Filled.Settings)
        items.forEachIndexed { i, (label, icon) ->
            NavigationBarItem(
                selected = i == 0,
                onClick = {},
                icon = { Icon(icon, contentDescription = null) },
                label = { Text(label, if (lowLabel && i == 0) Modifier.offset(y = 4.dp) else Modifier) },
            )
        }
    }
}
