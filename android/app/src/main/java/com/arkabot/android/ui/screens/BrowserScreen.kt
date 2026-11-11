package com.arkabot.android.ui.screens

import android.webkit.WebView
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel
import com.arkabot.android.webview.BrowserBridgeServer

/**
 * The embedded browser.
 *
 * Each profile gets its own WebView and its own loopback bridge, so two
 * agents never share cookies or a page. The bridge's port is handed to the
 * Rust engine, which drives the page with the browser tools.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BrowserScreen(onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val agents by viewModel.agents.collectAsState()
    val notice by viewModel.notice.collectAsState()
    val error by viewModel.error.collectAsState()

    // The profile name binds the WebView, the bridge and the agent together.
    var profile by remember { mutableStateOf("default") }
    var url by remember { mutableStateOf("https://example.com") }
    var connected by remember { mutableStateOf(false) }

    var webView by remember { mutableStateOf<WebView?>(null) }
    var bridge by remember { mutableStateOf<BrowserBridgeServer?>(null) }

    // One WebView and one bridge per profile.
    LaunchedEffect(profile) {
        bridge?.stop()
        bridge = null
        connected = false
        val view = WebView(context)
        BrowserBridgeServer.prepare(view)
        webView = view
        val server = BrowserBridgeServer(view) { message -> viewModel.logNotice(message) }
        val port = server.start()
        bridge = server
        viewModel.connectBrowser(profile, "http://127.0.0.1:$port")
        connected = true
        view.loadUrl(url)
    }
    DisposableEffect(Unit) {
        onDispose {
            bridge?.stop()
            webView?.destroy()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Browser") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            Text("Profil", fontWeight = FontWeight.Bold)
            Spacer(Modifier.height(4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("default" to "Shared").let { defaults ->
                    defaults.forEach { (id, label) ->
                        FilterChip(
                            selected = profile == id,
                            onClick = { profile = id },
                            label = { Text(label) }
                        )
                    }
                }
            }
            Spacer(Modifier.height(4.dp))
            // Agents pick their own profile by name in the agent editor.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                agents.forEach { agent ->
                    val agentProfile = agent.optString("browser_profile", "")
                    if (agentProfile.isNotEmpty()) {
                        FilterChip(
                            selected = profile == agentProfile,
                            onClick = { profile = agentProfile },
                            label = { Text(agent.optString("avatar", "🤖") + " " + agentProfile) }
                        )
                    }
                }
            }

            Spacer(Modifier.height(8.dp))
            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text("URL") },
                    modifier = Modifier.weight(1f),
                    singleLine = true
                )
                Spacer(Modifier.width(8.dp))
                Button(onClick = { webView?.loadUrl(url) }) { Text("Go") }
            }

            Spacer(Modifier.height(8.dp))
            if (!connected) {
                Text("Menyiapkan bridge…", style = MaterialTheme.typography.labelSmall)
            }
            if (notice != null) {
                Text(
                    notice!!,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary
                )
            }
            if (error != null) {
                Text(
                    "Gagal: $error",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error
                )
            }

            Spacer(Modifier.height(8.dp))
            AndroidView(
                factory = { context ->
                    webView ?: WebView(context).also { BrowserBridgeServer.prepare(it) }
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
            )
        }
    }
}
