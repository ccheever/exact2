package exact.app

import android.util.Log
import dev.exact.canvas.ExactApp
import dev.exact.canvas.ExactElement
import dev.exact.canvas.ExactHatches
import dev.exact.canvas.ExactPicker
import java.time.DayOfWeek

/**
 * The native-module fixture's Kotlin hatches on Android (LLP 1115.001.000 D3, D4), beside its Rust ones
 * (`modules/linux`), which hear the same moments. Each moment is logged under `ExactHatch`, with the view a node
 * gives (the `feed` field's `EditText`); the `due` hatch sets what its date and time pickers offer: its own titles,
 * no weekend day while the node says `data-weekends="off"`, and a 24-hour clock.
 */
class Hatches : ExactHatches() {
    override fun app(app: ExactApp) {
        Log.i(TAG, "app ${if (app.isNew) "built" else "changed"} mood=${app.data["mood"]}")
    }

    override fun element(element: ExactElement) {
        val moment = if (element.isNew) "built" else "changed"
        Log.i(TAG, "element ${element.hatch} #${element.node} $moment ${element.frame} ${element.words} view=${element.view?.javaClass?.simpleName}")
    }

    override fun elementEnded(element: ExactElement) {
        Log.i(TAG, "element ${element.hatch} #${element.node} ended")
    }

    override fun picker(element: ExactElement, picker: ExactPicker) {
        if (element.hatch != "due") return
        Log.i(TAG, "picker ${picker.kind} for #${element.node} at ${picker.value}")
        if (picker.kind == "time") {
            picker.title = "Due at"
            picker.hour24 = true
        } else {
            picker.title = "Due"
            if (element.data("weekends") == "off") picker.selectable = { it.dayOfWeek != DayOfWeek.SATURDAY && it.dayOfWeek != DayOfWeek.SUNDAY }
        }
    }

    private companion object {
        const val TAG = "ExactHatch"
    }
}
