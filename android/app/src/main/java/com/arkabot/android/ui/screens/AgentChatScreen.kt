package com.arkabot.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Send
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel
import org.json.JSONObject

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentChatScreen(agentId: String, onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    var messages by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var messageInput by remember { mutableStateOf("") }
    val agents by viewModel.agents.collectAsState()
    val agentInfo = agents.firstOrNull { it.optString("id") == agentId }

    LaunchedEffect(agentId) {
        // Initial load, then poll for new messages every 2 seconds.
        while (true) {
            messages = viewModel.loadMessages(agentId)
            kotlinx.coroutines.delay(2000)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(agentInfo?.optString("name", "Agent") ?: "Agent", fontWeight = FontWeight.Bold) },
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
        ) {
            LazyColumn(
                modifier = Modifier
                    .weight(1f)
                    .padding(horizontal = 8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                items(messages) { msg ->
                    MessageBubble(msg)
                }
            }

            // Input row
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(8.dp),
                verticalAlignment = Alignment.Bottom
            ) {
                OutlinedTextField(
                    value = messageInput,
                    onValueChange = { messageInput = it },
                    modifier = Modifier.weight(1f),
                    placeholder = { Text("Message agent...") },
                    singleLine = true,
                    shape = RoundedCornerShape(20.dp)
                )
                Spacer(modifier = Modifier.width(8.dp))
                IconButton(
                    onClick = {
                        val text = messageInput.trim()
                        if (text.isNotBlank()) {
                            viewModel.sendMessage(agentId, text)
                            messageInput = ""
                        }
                    },
                    modifier = Modifier
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primary)
                        .padding(0.dp)
                ) {
                    Icon(Icons.Default.Send, contentDescription = "Send", tint = Color.White)
                }
            }
        }
    }
}

/**
 * Message rows are serialized by serde as `sender = {"kind":"user"}` /
 * `{"kind":"agent","agent_id":...}` / `{"kind":"routine",...}`. Older rows may
 * still be a plain string, so both shapes are accepted.
 */
private fun senderKind(msg: JSONObject): String {
    val sender = msg.optJSONObject("sender")
    return when {
        sender != null -> sender.optString("kind", "user")
        else -> msg.optString("sender", "user")
    }
}

@Composable
private fun MessageBubble(msg: JSONObject) {
    val kind = senderKind(msg)
    val isUser = kind == "user"
    val text = msg.optString("text", "")
    val bgColor = if (isUser) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant
    val fgColor = if (isUser) Color.White else MaterialTheme.colorScheme.onSurface

    Column(modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp)) {
        Row(modifier = Modifier.fillMaxWidth()) {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(start = if (isUser) 32.dp else 0.dp, end = if (!isUser) 32.dp else 0.dp)
            ) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(12.dp))
                        .background(bgColor)
                        .padding(12.dp)
                ) {
                    Text(text, color = fgColor, style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}
