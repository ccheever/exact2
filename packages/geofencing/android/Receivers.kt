// The package's two receivers (LLP 1133.021 D14), declared in its `exact.json` and written into the manifest by the
// build: they run with no session, in a process the system may have started for them alone.
package exact.geofencing

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.google.android.gms.location.Geofence
import com.google.android.gms.location.GeofenceStatusCodes
import com.google.android.gms.location.GeofencingEvent
import dev.exact.canvas.ExactRegions
import org.json.JSONObject

/**
 * Play services' broadcast of a crossing: parsed, each transition written to the store at once, before any session
 * exists, then a region wake asked for. `goAsync()` keeps the process until that is done.
 */
class GeofenceReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val result = goAsync()
        try {
            val event = GeofencingEvent.fromIntent(intent) ?: return
            if (event.hasError()) {
                // Location turned off: Play services dropped every fence, which the next launch or boot re-arms.
                Log.i("exact", "geofencing: ${GeofenceStatusCodes.getStatusCodeString(event.errorCode)}")
                return
            }
            val inside = when (event.geofenceTransition) {
                Geofence.GEOFENCE_TRANSITION_ENTER, Geofence.GEOFENCE_TRANSITION_DWELL -> true
                Geofence.GEOFENCE_TRANSITION_EXIT -> false
                else -> return
            }
            val at = System.currentTimeMillis().toDouble()
            val where = event.triggeringLocation?.let {
                JSONObject().put("latitude", it.latitude).put("longitude", it.longitude).put("accuracy", it.accuracy.toDouble())
            }
            val store = ExactRegions.store(context)
            for (fence in event.triggeringGeofences.orEmpty()) store.transition(fence.requestId, inside, at, where)
            if (!ExactRegions.wake(context)) Log.i("exact", "geofencing: no session; the transition is kept for the next launch")
        } finally {
            result.finish()
        }
    }
}

/** After a reboot Play services has no fences: the store's are added again, raising nothing for one the device is
 * already inside, running no app code and booting no session. */
class GeofenceBootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        val result = goAsync()
        GeofencingModule.arm(context.applicationContext, ExactRegions.store(context).list(), initial = false) { e ->
            if (e != null) Log.i("exact", "geofencing: re-arm at boot failed: $e")
            result.finish()
        }
    }
}
