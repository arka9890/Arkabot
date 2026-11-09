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
    val templates by viewModel.templates.collectAsState()

    LaunchedEffect(Unit) {
        viewModel.refreshTemplates()
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
            Text(
                "Agent templates let you create pre-configured agents quickly.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(16.dp))

            if (templates.isEmpty()) {
                Column(
                    modifier = Modifier.fillMaxWidth().padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text("No templates", style = MaterialTheme.typography.titleMedium)
                }
            } else {
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(templates) { template ->
                        TemplateCard(
                            template = template,
                            onUse = { viewModel.createFromTemplate(template) }
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun TemplateCard(template: JSONObject, onUse: () -> Unit) {
    val name = template.optString("name", "Template")
    val description = template.optString("description", "")
    val avatar = template.optString("avatar", "🤖")
    val model = template.optString("model", "")
    val provider = template.optString("provider", "simulated")
    val tags = buildList {
        val arr = template.optJSONArray("tags")
        if (arr != null) {
            for (i in 0 until arr.length()) add(arr.optString(i))
        }
    }

    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(avatar, style = MaterialTheme.typography.headlineSmall)
                Spacer(Modifier.width(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                    if (description.isNotEmpty()) {
                        Text(
                            description,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
                Button(onClick = onUse) { Text("Use") }
            }

            if (model.isNotEmpty() || provider.isNotEmpty() || tags.isNotEmpty()) {
                Spacer(Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (provider.isNotEmpty()) TagChip(provider)
                    if (model.isNotEmpty()) TagChip(model)
                    tags.take(3).forEach { TagChip(it) }
                }
            }
        }
    }
}

@Composable
private fun TagChip(label: String) {
    SuggestionChip(
        onClick = {},
        label = { Text(label, style = MaterialTheme.typography.labelSmall) }
    )
}
