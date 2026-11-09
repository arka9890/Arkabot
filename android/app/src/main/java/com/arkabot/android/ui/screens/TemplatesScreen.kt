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
fun TemplatesScreen(onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val templates = remember { mutableStateListOf<JSONObject>() }
    val vm = viewModel()

    LaunchedEffect(Unit) {
        templates.addAll(vm.quickActions().let { vm.agents.value.let { agents -> if (agents.isEmpty()) emptyList() else listOf(JSONObject()) } }.let { emptyList() })
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Templates") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        Column(modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp)) {
            Text("Agent templates let you create pre-configured agents quickly.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(16.dp))
            if (templates.isEmpty()) {
                Column(modifier = Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("No templates", style = MaterialTheme.typography.titleMedium)
                }
            }
        }
    }
}
