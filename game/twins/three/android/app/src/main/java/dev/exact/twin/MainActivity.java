package dev.exact.twin;

import android.app.Activity;
import android.os.Bundle;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.webkit.WebView;
import android.webkit.WebSettings;

/** A full-screen system WebView over the three.js twin: `am start -n dev.exact.twin/.MainActivity --es url <url>`. */
public class MainActivity extends Activity {
    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        // Edge to edge, under the bars and the cutout, as the reader draws Exact's full-screen canvas: the same pixels.
        getWindow().setDecorFitsSystemWindows(false);
        getWindow().getAttributes().layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS;
        WebView.setWebContentsDebuggingEnabled(true);
        WebView web = new WebView(this);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        setContentView(web);
        getWindow().getInsetsController().hide(WindowInsets.Type.systemBars());
        String url = getIntent().getStringExtra("url");
        web.loadUrl(url != null ? url : "http://localhost:8790/bench.html");
    }
}
