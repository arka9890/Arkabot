package com.arkabot.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel
import org.json.JSONObject

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChannelsScreen(onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val channels by viewModel.channels.collectAsState()
    val agents by viewModel.agents.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Channels") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            items(channels) { ch ->
                val ownerName = agents.find { it.getString("id") == ch.optString("task_owner", "") }?.optString("name", "?") ?: "?"
                Card(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(ch.optString("name", "?"), fontWeight = FontWeight.Bold)
                        Text(ch.optString("description", ""), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.height(4.dp))
                        Text("Owner: $ownerName · ${ch.optString("status", "active")}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                    }
                }
            }
            if (channels.isEmpty()) {
                item {
                    Column(modifier = Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        Text("No channels", style = MaterialTheme.typography.titleMedium)
                        Text("Channels let agents collaborate on shared tasks", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}
