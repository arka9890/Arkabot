package com.arkabot.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.Send
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel
import kotlinx.coroutines.flow.collect
import org.json.JSONObject

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentChatScreen(agentId: String, onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val agents by viewModel.agents.collectAsState()
    val agentInfo = agents.firstOrNull { it.optString("id") == agentId }
    val agentName = agentInfo?.optString("name", "Agent") ?: "Agent"
    val agentAvatar = agentInfo?.optString("avatar", "🤖") ?: "🤖"

    var messageInput by remember { mutableStateOf("") }
    var messages by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var streamingText by remember { mutableStateOf("") }
    var streamingTurnId by remember { mutableStateOf<String?>(null) }
    var turnError by remember { mutableStateOf<String?>(null) }

    val listState = remember { LazyListState() }

    LaunchedEffect(agentId) {
        messages = viewModel.loadMessages(agentId)
        viewModel.events.collect { eventList ->
            eventList.forEach { event ->
                val inThisAgent = event.agentId == agentId
                when (event.kind) {
                    "message_added" -> {
                        if (inThisAgent) {
                            val data = event.payload.let { runCatching { JSONObject(it) }.getOrNull() }
                            val sender = data?.optString("sender", "") ?: ""
                            if (sender == "user") {
                                streamingText = ""
                                streamingTurnId = null
                                turnError = null
                            }
                            messages = viewModel.loadMessages(agentId)
                        }
                    }
                    "chat_delta" -> {
                        if (inThisAgent) {
                            val data = event.payload.let { runCatching { JSONObject(it) }.getOrNull() }
                            val turnId = data?.optString("turn_id", "") ?: ""
                            val delta = data?.optString("delta", "") ?: ""
                            if (turnId.isNotEmpty() && streamingTurnId != null && streamingTurnId != turnId) {
                                messages = viewModel.loadMessages(agentId)
                                streamingText = ""
                            }
                            if (delta.isNotEmpty()) {
                                streamingTurnId = turnId
                                turnError = null
                                streamingText += delta
                            }
                        }
                    }
                    "turn_failed" -> {
                        if (inThisAgent) {
                            val data = event.payload.let { runCatching { JSONObject(it) }.getOrNull() }
                            turnError = data?.optString("error", "provider gagal") ?: "provider gagal"
                            streamingText = ""
                            streamingTurnId = null
                        }
                    }
                    "turn_completed" -> {
                        if (inThisAgent) {
                            streamingText = ""
                            streamingTurnId = null
                            messages = viewModel.loadMessages(agentId)
                        }
                    }
                }
            }
        }
    }

    val totalRows = messages.size + if (streamingText.isNotEmpty()) 1 else 0 +
        if (turnError != null) 1 else 0
    LaunchedEffect(totalRows) {
        listState.scrollToItem((totalRows - 1).coerceAtLeast(0))
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(agentAvatar, fontSize = 20.sp)
                        Spacer(Modifier.width(8.dp))
                        Text(agentName, fontWeight = FontWeight.Bold)
                        if (streamingText.isNotEmpty()) {
                            Spacer(Modifier.width(8.dp))
                            Text("mengetik...", fontSize = 12.sp, color = MaterialTheme.colorScheme.outline)
                        }
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Kembali")
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
                    .fillMaxWidth(),
                state = listState,
                verticalArrangement = Arrangement.spacedBy(8.dp),
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp)
            ) {
                items(messages, key = { it.optString("id", "") }) { msg ->
                    MessageBubble(msg, agentName, agentAvatar)
                }
                if (streamingText.isNotEmpty()) {
                    item(key = "streaming") {
                        StreamingBubble(streamingText, agentName, agentAvatar)
                    }
                }
                turnError?.let { err ->
                    item(key = "turn_error") {
                        ErrorBubble(err)
                    }
                }
            }

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surface)
                    .padding(horizontal = 12.dp, vertical = 8.dp),
                verticalAlignment = Alignment.Bottom
            ) {
                OutlinedTextField(
                    value = messageInput,
                    onValueChange = { messageInput = it },
                    modifier = Modifier.weight(1f),
                    placeholder = { Text("Kirim pesan ke ${agentName}...") },
                    minLines = 1,
                    maxLines = 4,
                    shape = RoundedCornerShape(24.dp)
                )
                Spacer(Modifier.width(8.dp))
                IconButton(
                    onClick = {
                        val text = messageInput.trim()
                        if (text.isNotEmpty()) {
                            viewModel.sendMessage(agentId, text)
                            messageInput = ""
                        }
                    },
                    modifier = Modifier
                        .size(48.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primary)
                ) {
                    Icon(
                        Icons.Default.Send,
                        contentDescription = "Kirim",
                        tint = Color.White,
                        modifier = Modifier.padding(12.dp)
                    )
                }
            }
        }
    }
}

fun senderKind(msg: JSONObject): String {
    val sender = msg.optJSONObject("sender")
    return when {
        sender != null -> sender.optString("kind", "user")
        else -> msg.optString("sender", "user")
    }
}

@Composable
fun MessageBubble(msg: JSONObject, agentName: String, agentAvatar: String) {
    val isUser = senderKind(msg) == "user"
    val text = msg.optString("text", "")
    val bgColor = if (isUser) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant
    val fgColor = if (isUser) Color.White else MaterialTheme.colorScheme.onSurface

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = if (isUser) Arrangement.End else Arrangement.Start,
    ) {
        if (!isUser) {
            Box(
                modifier = Modifier
                    .padding(top = 14.dp)
                    .size(32.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.surfaceVariant),
                contentAlignment = Alignment.Center
            ) {
                Text(agentAvatar, fontSize = 16.sp)
            }
            Spacer(Modifier.width(8.dp))
        }

        Column(
            modifier = Modifier
                .widthIn(max = 320.dp)
                .clip(
                    RoundedCornerShape(
                        topStart = 16.dp,
                        topEnd = if (isUser) 4.dp else 16.dp,
                        bottomStart = if (isUser) 4.dp else 16.dp,
                        bottomEnd = 16.dp
                    )
                )
                .background(bgColor)
                .padding(horizontal = 14.dp, vertical = 10.dp)
        ) {
            if (!isUser) {
                Text(agentName, fontSize = 11.sp, color = MaterialTheme.colorScheme.outline)
                Spacer(Modifier.height(2.dp))
            }
            Text(text, color = fgColor, fontSize = 14.sp)
        }

        if (isUser) Spacer(Modifier.width(8.dp))
    }
}

@Composable
fun StreamingBubble(text: String, agentName: String, agentAvatar: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.Start
    ) {
        Box(
            modifier = Modifier
                .padding(top = 16.dp)
                .size(32.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.surfaceVariant),
            contentAlignment = Alignment.Center
        ) {
            Text(agentAvatar, fontSize = 16.sp)
        }
        Spacer(Modifier.width(8.dp))

        Column(
            modifier = Modifier
                .widthIn(max = 320.dp)
                .clip(
                    RoundedCornerShape(
                        topStart = 16.dp,
                        topEnd = 16.dp,
                        bottomStart = 4.dp,
                        bottomEnd = 16.dp
                    )
                )
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .padding(horizontal = 14.dp, vertical = 10.dp)
        ) {
            Text(agentName, fontSize = 11.sp, color = MaterialTheme.colorScheme.outline)
            Spacer(Modifier.height(2.dp))
            Text(text, color = MaterialTheme.colorScheme.onSurface, fontSize = 14.sp)
            Spacer(Modifier.height(4.dp))
            Text("mengetik...", fontSize = 11.sp, color = MaterialTheme.colorScheme.outline)
        }
    }
}

@Composable
fun ErrorBubble(message: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(Color(0xFFE57373).copy(alpha = 0.12f))
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            Icons.Default.Error,
            contentDescription = "Error",
            tint = Color(0xFFE57373),
            modifier = Modifier.size(18.dp)
        )
        Spacer(Modifier.width(8.dp))
        Text(
            "Gagal: $message",
            color = Color(0xFFE57373),
            fontSize = 13.sp
        )
    }
}
