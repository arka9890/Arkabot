package com.arkabot.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EventsScreen(onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val events by viewModel.events.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Events") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp)
        ) {
            items(events.reversed()) { e ->
                Card(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(e.kind, fontWeight = FontWeight.Bold)
                        Text(e.payload, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.height(4.dp))
                        Text("${e.agentId ?: "—"} · ${e.channelId ?: "—"}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                    }
                }
            }
            if (events.isEmpty()) {
                item {
                    Column(modifier = Modifier.padding(24.dp)) {
                        Text("No events yet", style = MaterialTheme.typography.titleMedium)
                        Text("Agent activity events appear here", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}
