package com.arkabot.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
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
    var name by remember { mutableStateOf("") }
    var avatar by remember { mutableStateOf("🤖") }
    var instructions by remember { mutableStateOf("") }
    var model by remember { mutableStateOf("") }
    var temperature by remember { mutableStateOf("0.7") }

    LaunchedEffect(agentId) {
        if (isEdit && agentId != null) {
            val agent = viewModel.agents.value.find { it.getString("id") == agentId }
            if (agent != null) {
                name = agent.optString("name", "")
                avatar = agent.optString("avatar", "🤖")
                instructions = agent.optString("instructions", "")
                model = agent.optString("model", "")
                temperature = agent.optDouble("temperature", 0.7).toString()
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
                        val inputJson = org.json.JSONObject().apply {
                            put("name", name)
                            put("avatar", avatar)
                            put("instructions", instructions)
                            put("model", model)
                            put("temperature", temperature.toDoubleOrNull() ?: 0.7)
                            put("description", "")
                            put("provider", "simulated")
                            put("max_tool_iterations", 8)
                            put("compaction_messages", 60)
                            put("busy_mode", "queue")
                            put("capabilities", org.json.JSONArray(["chat", "files", "task_queue", "notifications"]))
                        }.toString()
                        if (isEdit) {
                            viewModel.updateAgent(agentId!!, inputJson)
                        } else {
                            viewModel.createAgent(inputJson)
                        }
                        onBack()
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
                value = instructions,
                onValueChange = { instructions = it },
                label = { Text("Instructions (AGENTS.md)") },
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Multiline),
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
