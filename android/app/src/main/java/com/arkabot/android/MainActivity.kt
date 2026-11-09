package com.arkabot.android

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import com.arkabot.android.ui.ArkaBotNavigation
import com.arkabot.android.ui.theme.ArkaBotTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val app = application as ArkaBotApplication
        // Agent-reply notifications need the runtime permission on Android 13+.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            ActivityCompat.requestPermissions(
                this,
                arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                REQUEST_POST_NOTIFICATIONS
            )
        }
        val startupError = app.startupError
        val reportFile = CrashReporter.file(this)
        val crashReport = when {
            startupError != null ->
                reportFile.takeIf { it.exists() }?.readText()
                    ?: startupError.stackTraceToString()
            reportFile.exists() -> reportFile.readText()
            else -> null
        }
        setContent {
            ArkaBotTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    if (crashReport != null) {
                        // A failed native class init can't recover in-process:
                        // offer "retry" only when this run actually started.
                        val canRetry = startupError == null
                        StartupErrorScreen(
                            report = crashReport,
                            actionLabel = if (canRetry) "Hapus & coba lagi" else "Keluar",
                            onAction = {
                                reportFile.delete()
                                if (canRetry) recreate() else finish()
                            }
                        )
                    } else {
                        ArkaBotNavigation()
                    }
                }
            }
        }
    }

    companion object {
        private const val REQUEST_POST_NOTIFICATIONS = 1001
    }
}

/**
 * Shown instead of the app UI when startup failed (or a previous run crashed).
 * Gives the user the full report without needing adb/Android Studio.
 */
@Composable
private fun StartupErrorScreen(report: String, actionLabel: String, onAction: () -> Unit) {
    val context = LocalContext.current
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            text = "ArkaBot gagal start",
            style = MaterialTheme.typography.titleLarge
        )
        Text(
            text = "Salin atau bagikan laporan di bawah ini:",
            style = MaterialTheme.typography.bodyMedium
        )
        Text(
            text = report,
            style = MaterialTheme.typography.bodySmall,
            fontFamily = FontFamily.Monospace,
            modifier = Modifier
                .fillMaxWidth()
                .padding(8.dp)
        )
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Button(onClick = {
                val clipboard = context.getSystemService(ClipboardManager::class.java)
                clipboard.setPrimaryClip(ClipData.newPlainText("ArkaBot error report", report))
            }) { Text("Salin") }
            OutlinedButton(onClick = {
                val send = Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_TEXT, report)
                }
                context.startActivity(Intent.createChooser(send, "Bagikan laporan error"))
            }) { Text("Bagikan") }
            OutlinedButton(onClick = onAction) { Text(actionLabel) }
        }
    }
}
