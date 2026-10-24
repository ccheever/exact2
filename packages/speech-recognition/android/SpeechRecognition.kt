// @exact/speech-recognition on Android (LLP 1133.012). Stage 4 builds SpeechRecognizer; until then this
// implementation says recognition is not available here, so an app that imports the package builds and runs.
package exact.speechrecognition

import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

class SpeechRecognitionModule(context: ExtensionContext) : ExactExtension(context) {
    private fun unavailable() = JSONObject()
        .put("available", false).put("state", "unknown").put("inputs", JSONArray()).put("local", false)
        .put("apple", JSONObject.NULL).put("android", JSONObject.NULL)

    private fun none() = JSONObject()
        .put("id", "").put("state", "ended").put("audio", false).put("sound", false).put("speech", false)
        .put("results", JSONArray()).put("finalCount", 0).put("interim", JSONObject.NULL).put("leavesDevice", false)
        .put("error", JSONObject.NULL).put("message", JSONObject.NULL)

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = reply(
        when {
            name == "speechRecognitionStatus" -> unavailable()
            name == "speechRecognitionAvailability" -> JSONObject().put("availability", "unavailable")
            name == "speechRecognitionState" && args.optString("recognition").isEmpty() -> none()
            else -> JSONObject().put("error", "NotSupportedError")
        }
    )
}
