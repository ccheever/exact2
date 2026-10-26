// @exact/speech-synthesis on Android (LLP 1133.012). Stage 4 builds TextToSpeech; until then this implementation
// says speech is not available here, so an app that imports the package builds and runs.
package exact.speechsynthesis

import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

class SpeechSynthesisModule(context: ExtensionContext) : ExactExtension(context) {
    private fun idle() = JSONObject()
        .put("available", false).put("speaking", false).put("pending", false).put("paused", false)
        .put("pauseReasons", JSONArray()).put("audioSessionType", "auto").put("utterance", JSONObject.NULL)
        .put("charIndex", 0).put("charLength", 0).put("name", "").put("elapsedTime", 0)
        .put("queue", JSONArray()).put("apple", JSONObject.NULL)

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = reply(
        when (name) {
            "speechSynthesisState" -> idle()
            "speechSynthesisVoices" -> JSONObject().put("voices", JSONArray())
            else -> JSONObject().put("error", "NotSupportedError")
        }
    )
}
