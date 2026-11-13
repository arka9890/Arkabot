package com.arkabot.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val settings by viewModel.settings.collectAsState()
    val error by viewModel.error.collectAsState()
    var provider by remember { mutableStateOf("simulated") }
    var baseUrl by remember { mutableStateOf("") }
    var apiKey by remember { mutableStateOf("") }
    var model by remember { mutableStateOf("") }
    var temperature by remember { mutableStateOf("0.7") }
    var demoMode by remember { mutableStateOf(true) }
    var appearance by remember { mutableStateOf("system") }

    LaunchedEffect(settings) {
        provider = settings.optString("provider", "simulated")
        baseUrl = settings.optString("base_url", "")
        apiKey = settings.optString("api_key", "")
        model = settings.optString("model", "")
        temperature = settings.optDouble("temperature", 0.7).toString()
        demoMode = settings.optBoolean("demo_mode", true)
        appearance = settings.optString("appearance", settings.optString("theme", "system"))
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
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
                .verticalScroll(androidx.compose.foundation.rememberScrollState())
        ) {
            Text("Provider", fontWeight = FontWeight.Bold)
            Spacer(Modifier.height(8.dp))

            Text("Provider Type", style = MaterialTheme.typography.labelLarge)
            FilterChip(
                selected = provider == "simulated",
                onClick = { provider = "simulated" },
                label = { Text("Simulated (no server)") }
            )
            Spacer(Modifier.width(8.dp))
            FilterChip(
                selected = provider == "ollama",
                onClick = { provider = "ollama" },
                label = { Text("Ollama") }
            )
            Spacer(Modifier.width(8.dp))
            FilterChip(
                selected = provider == "lmstudio",
                onClick = { provider = "lmstudio" },
                label = { Text("LM Studio") }
            )
            Spacer(Modifier.width(8.dp))
            FilterChip(
                selected = provider == "openai-compatible",
                onClick = { provider = "openai-compatible" },
                label = { Text("OpenAI-compatible") }
            )

            Spacer(Modifier.height(16.dp))

            if (provider != "simulated") {
                OutlinedTextField(
                    value = baseUrl,
                    onValueChange = { baseUrl = it },
                    label = { Text("Base URL") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    supportingText = { Text("HP: http://IP-KOMPUTER-ANDA:11434 · Emulator: http://10.0.2.2:11434") }
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = apiKey,
                    onValueChange = { apiKey = it },
                    label = { Text("API Key (optional)") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
                )
                Spacer(Modifier.height(8.dp))
            }

            OutlinedTextField(
                value = model,
                onValueChange = { model = it },
                label = { Text("Default Model") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = temperature,
                onValueChange = { temperature = it },
                label = { Text("Temperature") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal)
            )
            Spacer(Modifier.height(16.dp))

            Text("Appearance", fontWeight = FontWeight.Bold)
            Spacer(Modifier.height(8.dp))
            Text("Theme", style = MaterialTheme.typography.labelLarge)
            FilterChip(
                selected = appearance == "light",
                onClick = { appearance = "light" },
                label = { Text("Light") }
            )
            Spacer(Modifier.width(8.dp))
            FilterChip(
                selected = appearance == "dark",
                onClick = { appearance = "dark" },
                label = { Text("Dark") }
            )
            Spacer(Modifier.width(8.dp))
            FilterChip(
                selected = appearance == "system",
                onClick = { appearance = "system" },
                label = { Text("System") }
            )

            Spacer(Modifier.height(16.dp))

            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                Text("Demo Mode")
                Spacer(Modifier.weight(1f))
                Switch(checked = demoMode, onCheckedChange = { demoMode = it })
            }
            Text("Demo mode uses a simulated provider — no API key or server needed.", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)

            Spacer(Modifier.height(24.dp))

            if (error != null) {
                Text(
                    text = "Save gagal: $error",
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall
                )
                Spacer(Modifier.height(8.dp))
            }

            Button(
                onClick = {
                    val json = org.json.JSONObject().apply {
                        put("provider", provider)
                        put("base_url", baseUrl)
                        put("api_key", apiKey)
                        put("model", model)
                        put("temperature", temperature.toDoubleOrNull() ?: 0.7)
                        put("appearance", appearance)
                        put("busy_mode", "queue")
                        put("demo_mode", demoMode)
                    }.toString()
                    viewModel.clearError()
                    // Navigate back only after the save succeeds; on failure
                    // the error above stays visible instead of crashing.
                    viewModel.updateSettings(json) { onBack() }
                },
                modifier = Modifier.fillMaxWidth()
            ) { Text("Save Settings") }
        }
    }
}
