package com.arkabot.android

import android.content.Context
import android.os.Build
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Persists startup/crash reports to internal storage so users without adb can
 * copy/share them from the in-app error screen (see MainActivity).
 */
object CrashReporter {
    private const val FILE_NAME = "crash_report.txt"

    fun file(context: Context): File = File(context.filesDir, FILE_NAME)

    fun write(context: Context, error: Throwable) {
        runCatching {
            val sb = StringBuilder()
            sb.append("time: ")
                .append(SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.US).format(Date()))
                .append('\n')
            sb.append("device: ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL).append('\n')
            sb.append("android: ").append(Build.VERSION.RELEASE)
                .append(" (SDK ").append(Build.VERSION.SDK_INT).append(")\n")
            sb.append("abis: ").append(Build.SUPPORTED_ABIS.joinToString(",")).append('\n')
            sb.append('\n')
            sb.append(error.stackTraceToString())
            file(context).writeText(sb.toString())
        }
    }
}
