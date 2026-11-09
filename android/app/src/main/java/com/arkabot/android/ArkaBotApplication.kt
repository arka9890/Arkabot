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

    /**
     * Non-null when the native/Rust startup failed. Instead of force-closing
     * without a trace, MainActivity shows the persisted report so the user can
     * copy/share it (no adb required).
     */
    var startupError: Throwable? = null
        private set

    override fun onCreate() {
        super.onCreate()
        // Uncaught errors on any thread (FFI callbacks, Rust event forwarder)
        // are persisted before the process dies.
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            CrashReporter.write(this, error)
            previous?.uncaughtException(thread, error)
        }
        try {
            val dataDir = filesDir.absolutePath
            openbot = ArkaBotHandle(dataDir)
        } catch (t: Throwable) {
            // UnsatisfiedLinkError is an Error, not an Exception — catch both.
            startupError = t
            CrashReporter.write(this, t)
        }
    }
}
