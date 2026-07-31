package io.muxport.mobile

import android.os.Bundle
import android.view.WindowManager
import io.flutter.embedding.android.FlutterFragmentActivity

class MainActivity : FlutterFragmentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Muxport can display source code, commands, approvals, and account
        // metadata. Keep all of it out of screenshots, screen recording, and
        // Android's recent-app preview.
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
    }
}
