// The activity Health Connect opens from its permission screen (LLP 1133.026 D4): the rationale on Android 13 and lower
// (`androidx.health.ACTION_SHOW_PERMISSIONS_RATIONALE`), the permission usage alias on 14 and later. It shows the
// app's `privacy_policy`, which the manifest fragment puts in the application's `exact.privacy_policy` meta-data, in
// the browser, and closes.
package exact.health

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle

class PrivacyPolicyActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val url = packageManager.getApplicationInfo(packageName, PackageManager.GET_META_DATA).metaData?.getString("exact.privacy_policy")
        if (!url.isNullOrEmpty()) {
            try { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) } catch (e: ActivityNotFoundException) { /* no browser: nothing to show */ }
        }
        finish()
    }
}
