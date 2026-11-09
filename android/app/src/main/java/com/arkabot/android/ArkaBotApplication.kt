package com.arkabot.android

import android.app.Application
import uniffi.arkabot_ffi.ArkaBotHandle

/**
 * Application-scoped ArkaBot handle. Created once, shared across all screens.
 * The Rust runtime (tokio + agent engine) lives inside this handle.
 */
class ArkaBotApplication : Application() {
    lateinit var openbot: ArkaBotHandle
        private set

    override fun onCreate() {
        super.onCreate()
        val dataDir = filesDir.absolutePath
        openbot = ArkaBotHandle(dataDir)
    }
}
