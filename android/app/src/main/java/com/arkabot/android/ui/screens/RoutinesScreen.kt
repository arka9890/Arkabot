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
fun RoutinesScreen(onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val routines by viewModel.routines.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Routines") },
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
            items(routines) { r ->
                Card(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(r.optString("name", "?"), fontWeight = FontWeight.Bold)
                        Text(r.optString("prompt", ""), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.height(4.dp))
                        val interval = r.optLong("interval_minutes", 0)
                        Text("Every $interval min · ${r.optString("status", "active")}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                    }
                }
            }
            if (routines.isEmpty()) {
                item {
                    Column(modifier = Modifier.padding(24.dp)) {
                        Text("No routines", style = MaterialTheme.typography.titleMedium)
                        Text("Routines run agent prompts on a schedule", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}
