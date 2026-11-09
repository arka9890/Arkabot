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
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentChatScreen(agentId: String, onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val messages by remember { mutableStateOf(viewModel.agentMessages(agentId)) }
    val agent = remember { viewModel.agents.value.find { it.getString("id") == agentId } }
    val scaffoldState = rememberScaffoldState()
    val scope = rememberCoroutineScope()

    LaunchedEffect(agentId) {
        // Poll for new messages every 2 seconds
        while (true) {
            kotlinx.coroutines.delay(2000)
            messages.clear()
            messages.addAll(viewModel.agentMessages(agentId))
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(agent?.optString("name", "Agent") ?: "Agent") },
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
                        if (messageInput.isNotBlank()) {
                            viewModel.sendMessage(agentId, messageInput)
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

@Composable
private fun MessageBubble(msg: JSONObject) {
    val isUser = msg.optString("sender", "user") == "user"
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
