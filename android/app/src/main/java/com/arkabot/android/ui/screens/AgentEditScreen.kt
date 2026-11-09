package com.arkabot.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentEditScreen(agentId: String?, onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val isEdit = agentId != null
    val error by viewModel.error.collectAsState()
    var name by remember { mutableStateOf("") }
    var avatar by remember { mutableStateOf("🤖") }
    var avatarColor by remember { mutableStateOf("#7AA2F7") }
    var instructions by remember { mutableStateOf("") }
    var model by remember { mutableStateOf("") }
    var temperature by remember { mutableStateOf("0.7") }
    var provider by remember { mutableStateOf("simulated") }
    var busyMode by remember { mutableStateOf("queue") }
    var maxToolIterations by remember { mutableStateOf(8) }
    var compactionMessages by remember { mutableStateOf(60) }

    LaunchedEffect(agentId) {
        if (isEdit && agentId != null) {
            val agent = viewModel.agents.value.find { it.getString("id") == agentId }
            if (agent != null) {
                name = agent.optString("name", "")
                avatar = agent.optString("avatar", "🤖")
                avatarColor = agent.optString("avatar_color", "#7AA2F7")
                instructions = agent.optString("instructions", "")
                model = agent.optString("model", "")
                temperature = agent.optDouble("temperature", 0.7).toString()
                provider = agent.optString("provider", "simulated")
                busyMode = agent.optString("busy_mode", "queue")
                maxToolIterations = agent.optInt("max_tool_iterations", 8)
                compactionMessages = agent.optInt("compaction_messages", 60)
            }
        }
    }

    val scope = rememberCoroutineScope()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (isEdit) "Edit Agent" else "New Agent") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = {
                        // AgentInput on the Rust side is strict: every field
                        // (including avatar_color) must be present.
                        val inputJson = org.json.JSONObject().apply {
                            put("name", name)
                            put("avatar", avatar)
                            put("avatar_color", avatarColor)
                            put("instructions", instructions)
                            put("model", model)
                            put("temperature", temperature.toDoubleOrNull() ?: 0.7)
                            put("description", "")
                            put("provider", provider)
                            put("max_tool_iterations", maxToolIterations)
                            put("compaction_messages", compactionMessages)
                            put("busy_mode", busyMode)
                            put("capabilities", org.json.JSONArray().apply {
                                put("chat")
                                put("files")
                                put("task_queue")
                                put("notifications")
                            })
                        }.toString()
                        viewModel.clearError()
                        // Non-blocking save; navigate back only on success so
                        // failures show below instead of crashing/going blank.
                        if (isEdit) {
                            viewModel.updateAgent(agentId!!, inputJson) { onBack() }
                        } else {
                            viewModel.createAgent(inputJson) { onBack() }
                        }
                    }) {
                        Icon(Icons.Default.Check, contentDescription = "Save")
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
            if (error != null) {
                Text(
                    text = "Save gagal: $error",
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall
                )
                Spacer(Modifier.height(8.dp))
            }

            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Name") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = avatar,
                onValueChange = { avatar = it },
                label = { Text("Avatar (emoji)") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = avatarColor,
                onValueChange = { avatarColor = it },
                label = { Text("Avatar color (#RRGGBB)") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = instructions,
                onValueChange = { instructions = it },
                label = { Text("Instructions (AGENTS.md)") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Text),
                minLines = 4
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = model,
                onValueChange = { model = it },
                label = { Text("Model (e.g. llama3.2)") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = temperature,
                onValueChange = { temperature = it },
                label = { Text("Temperature") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                singleLine = true
            )
        }
    }
}
