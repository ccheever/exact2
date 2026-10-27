// @exact/device on Android (LLP 1133.031 D1–D5, D8, D9): BatteryManager's sticky broadcast, ConnectivityManager's
// default network and PowerManager, answered in the interface's shapes. The build makes one per host when the app
// binds the extension, outside the agent; under the agent the runner's substitute answers and none is made.
//
// - Android runs one session a process, so this instance's observers are the process's (D5 item 1). Each starts at its
//   source's first read and stops at its `release`, when the runner's last reader goes.
// - A source announces only when an answered field changes (D5 item 3): `ACTION_BATTERY_CHANGED` also arrives for
//   voltage and temperature, and `level` is compared in hundredths, BATTERY_PROPERTY_CAPACITY's percent.
// - The host's resume reads every observed source again (D5 item 4).
// - `ACCESS_NETWORK_STATE`, a normal permission, comes from the implementation's manifest fragment (D8).
package exact.device

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.BatteryManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject
import java.util.concurrent.Executor

class DeviceModule(context: ExtensionContext) : ExactExtension(context) {
    private val main = Handler(Looper.getMainLooper())
    private val app: Context get() = context.application
    private val power get() = app.getSystemService(Context.POWER_SERVICE) as PowerManager
    private val connectivity get() = app.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private val batteries get() = app.getSystemService(Context.BATTERY_SERVICE) as BatteryManager

    /** Each observed source's last answer, compared as text. */
    private val last = HashMap<String, String>()
    private var batteryReceiver: BroadcastReceiver? = null
    private var powerReceiver: BroadcastReceiver? = null
    private var thermalListener: PowerManager.OnThermalStatusChangedListener? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    /** The default network's capabilities, from the callback; null until it first answers or when there is none. */
    private var capabilities: NetworkCapabilities? = null
    private var networkKnown = false

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        main.post {
            when (name) {
                "getBattery" -> { startBattery(); reply(remember(name, battery(null))) }
                "connection" -> { startNetwork(); reply(remember(name, network())) }
                "powerState" -> { startPower(); reply(remember(name, powerState())) }
                else -> reply(JSONObject().put("error", "NotSupportedError"))
            }
        }
    }

    private fun remember(name: String, answer: JSONObject): JSONObject {
        last[name] = answer.toString()
        return answer
    }

    /** A new answer for [name]: announced when it differs from the last. */
    private fun heard(name: String, answer: JSONObject) {
        val text = answer.toString()
        if (!last.containsKey(name) || last[name] == text) return
        last[name] = text
        context.changed("device.$name")
    }

    override fun release(name: String) {
        main.post {
            last.remove(name)
            when (name) {
                "getBattery" -> batteryReceiver?.let { app.unregisterReceiver(it) }.also { batteryReceiver = null }
                "connection" -> networkCallback?.let { connectivity.unregisterNetworkCallback(it) }.also {
                    networkCallback = null; capabilities = null; networkKnown = false
                }
                "powerState" -> {
                    powerReceiver?.let { app.unregisterReceiver(it) }
                    powerReceiver = null
                    if (Build.VERSION.SDK_INT >= 29) thermalListener?.let { power.removeThermalStatusListener(it) }
                    thermalListener = null
                }
            }
        }
    }

    override fun foreground(shown: Boolean) {
        if (!shown) return
        main.post {
            if (batteryReceiver != null) heard("getBattery", battery(null))
            if (networkCallback != null) heard("connection", network())
            if (powerReceiver != null) heard("powerState", powerState())
        }
    }

    // The battery (D1): the sticky ACTION_BATTERY_CHANGED, registered on the application context.

    private fun startBattery() {
        if (batteryReceiver != null) return
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context, intent: Intent) = heard("getBattery", battery(intent))
        }
        batteryReceiver = receiver
        app.registerReceiver(receiver, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
    }

    private fun battery(given: Intent?): JSONObject {
        val intent = given ?: app.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val present = intent?.getBooleanExtra(BatteryManager.EXTRA_PRESENT, false) ?: false
        val status = intent?.getIntExtra(BatteryManager.EXTRA_STATUS, BatteryManager.BATTERY_STATUS_UNKNOWN) ?: BatteryManager.BATTERY_STATUS_UNKNOWN
        val plugged = intent?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0
        val android = JSONObject()
            .put("status", when (status) {
                BatteryManager.BATTERY_STATUS_CHARGING -> "charging"
                BatteryManager.BATTERY_STATUS_DISCHARGING -> "discharging"
                BatteryManager.BATTERY_STATUS_FULL -> "full"
                BatteryManager.BATTERY_STATUS_NOT_CHARGING -> "notCharging"
                else -> "unknown"
            })
            .put("plugged", when (plugged) {
                BatteryManager.BATTERY_PLUGGED_AC -> "ac"
                BatteryManager.BATTERY_PLUGGED_USB -> "usb"
                BatteryManager.BATTERY_PLUGGED_WIRELESS -> "wireless"
                8 -> "dock" // BATTERY_PLUGGED_DOCK, API 33
                else -> "none"
            })
            .put("batteryLow", Build.VERSION.SDK_INT >= 28 && intent?.getBooleanExtra(BatteryManager.EXTRA_BATTERY_LOW, false) == true)
        // No battery: the specification's full battery, charging (D1).
        if (!present) return JSONObject().put("level", 1.0).put("charging", true).put("android", android)
        var percent = batteries.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
        if (percent !in 0..100) {
            val level = intent?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
            val scale = intent?.getIntExtra(BatteryManager.EXTRA_SCALE, -1) ?: -1
            percent = if (level >= 0 && scale > 0) Math.round(level * 100f / scale) else 100
        }
        return JSONObject()
            .put("level", percent / 100.0)
            .put("charging", status != BatteryManager.BATTERY_STATUS_DISCHARGING)
            .put("android", android)
    }

    // The network (D2): the default network's capabilities, and Data Saver.

    private fun startNetwork() {
        if (networkCallback != null) return
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onCapabilitiesChanged(changed: Network, caps: NetworkCapabilities) {
                main.post { capabilities = caps; networkKnown = true; heard("connection", network()) }
            }
            override fun onLost(lost: Network) {
                main.post { capabilities = null; networkKnown = true; heard("connection", network()) }
            }
            override fun onBlockedStatusChanged(blockedNetwork: Network, blocked: Boolean) {
                main.post { heard("connection", network()) }
            }
        }
        networkCallback = callback
        connectivity.registerDefaultNetworkCallback(callback, main)
        // The answer now, before the callback's first: the active network, or none.
        val active = connectivity.activeNetwork
        capabilities = active?.let { connectivity.getNetworkCapabilities(it) }
        networkKnown = true
    }

    private fun network(): JSONObject {
        val caps = capabilities
        val transports = listOf(
            NetworkCapabilities.TRANSPORT_WIFI to "wifi",
            NetworkCapabilities.TRANSPORT_CELLULAR to "cellular",
            NetworkCapabilities.TRANSPORT_ETHERNET to "ethernet",
            NetworkCapabilities.TRANSPORT_BLUETOOTH to "bluetooth",
        ).filter { caps?.hasTransport(it.first) == true }.map { it.second }
        val type = when {
            !networkKnown -> "unknown"
            caps == null -> "none"
            transports.size > 1 -> "mixed"
            transports.size == 1 -> transports[0]
            else -> "other" // VPN alone, USB, Thread, satellite
        }
        val metered = caps != null && !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
        val restrict = when (connectivity.restrictBackgroundStatus) {
            ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED -> "enabled"
            ConnectivityManager.RESTRICT_BACKGROUND_STATUS_WHITELISTED -> "whitelisted"
            else -> "disabled"
        }
        val android = JSONObject()
            .put("metered", metered)
            .put("temporarilyNotMetered", if (Build.VERSION.SDK_INT >= 30 && caps != null) caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_TEMPORARILY_NOT_METERED) else JSONObject.NULL)
            .put("roaming", caps != null && !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_ROAMING))
            .put("restrictBackgroundStatus", restrict)
        return JSONObject()
            .put("type", type)
            .put("saveData", restrict != "disabled" && metered)
            .put("android", android)
    }

    // The power state (D3): Battery Saver and the thermal status.

    private fun startPower() {
        if (powerReceiver != null) return
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context, intent: Intent) = heard("powerState", powerState())
        }
        powerReceiver = receiver
        app.registerReceiver(receiver, IntentFilter(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED))
        if (Build.VERSION.SDK_INT >= 29) {
            val listener = PowerManager.OnThermalStatusChangedListener { main.post { heard("powerState", powerState()) } }
            thermalListener = listener
            power.addThermalStatusListener(Executor { main.post(it) }, listener)
        }
    }

    private fun powerState(): JSONObject {
        val answer = JSONObject().put("lowPowerMode", power.isPowerSaveMode)
        if (Build.VERSION.SDK_INT < 29) return answer.put("thermalState", "nominal").put("android", JSONObject.NULL)
        val status = power.currentThermalStatus
        val thermal = when (status) {
            PowerManager.THERMAL_STATUS_NONE -> "nominal"
            PowerManager.THERMAL_STATUS_LIGHT, PowerManager.THERMAL_STATUS_MODERATE -> "fair"
            PowerManager.THERMAL_STATUS_SEVERE -> "serious"
            else -> "critical"
        }
        val word = when (status) {
            PowerManager.THERMAL_STATUS_NONE -> "none"
            PowerManager.THERMAL_STATUS_LIGHT -> "light"
            PowerManager.THERMAL_STATUS_MODERATE -> "moderate"
            PowerManager.THERMAL_STATUS_SEVERE -> "severe"
            PowerManager.THERMAL_STATUS_CRITICAL -> "critical"
            PowerManager.THERMAL_STATUS_EMERGENCY -> "emergency"
            else -> "shutdown"
        }
        return answer.put("thermalState", thermal).put("android", JSONObject().put("thermalStatus", word))
    }
}
