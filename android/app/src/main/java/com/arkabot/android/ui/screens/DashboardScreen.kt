package com.arkabot.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.horizontalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Extension
import androidx.compose.material.icons.filled.Forum
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Language
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Settings
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
fun DashboardScreen(
    onOpenChat: (String) -> Unit,
    onOpenEdit: (String?) -> Unit,
    onOpenChannels: () -> Unit,
    onOpenRoutines: () -> Unit,
    onOpenSettings: () -> Unit,
    onOpenEvents: () -> Unit,
    onOpenTemplates: () -> Unit,
    onOpenSkills: () -> Unit,
    onOpenBrowser: () -> Unit,
    viewModel: ArkaBotViewModel = viewModel()
) {
    val agents by viewModel.agents.collectAsState()
    val channels by viewModel.channels.collectAsState()
    val quickActions by viewModel.quickActions.collectAsState()
    val settings by viewModel.settings.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("ArkaBot", fontWeight = FontWeight.Bold) },
                actions = {
                    IconButton(onClick = { viewModel.refreshAll() }) {
                        Icon(Icons.Default.Refresh, contentDescription = "Refresh")
                    }
                    IconButton(onClick = onOpenEvents) {
                        Icon(Icons.Default.History, contentDescription = "Events")
                    }
                    IconButton(onClick = onOpenSettings) {
                        Icon(Icons.Default.Settings, contentDescription = "Settings")
                    }
                }
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { onOpenEdit(null) },
                icon = { Icon(Icons.Default.Add, contentDescription = null) },
                text = { Text("New Agent") }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            val providerName = settings.optString("provider", "simulated")
            val model = settings.optString("model", "")
            val demoMode = settings.optBoolean("demo_mode", true)
            if (demoMode) {
                Text(
                    text = "Demo Mode · $providerName${if (model.isNotEmpty()) " · $model" else ""}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary
                )
                Spacer(modifier = Modifier.height(8.dp))
            }

            if (quickActions.isNotEmpty()) {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(quickActions) { action ->
                        QuickActionChip(action, viewModel)
                    }
                }
                Spacer(modifier = Modifier.height(16.dp))
            }

            Text("Agents", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            Spacer(modifier = Modifier.height(8.dp))

            if (agents.isEmpty()) {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Text("No agents yet", style = MaterialTheme.typography.titleMedium)
                        Text(
                            "Tap + to create your first agent",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            } else {
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(agents) { agent ->
                        AgentCard(
                            agent = agent,
                            onClick = { onOpenChat(agent.getString("id")) },
                            onEdit = { onOpenEdit(agent.getString("id")) }
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onOpenChannels() }
                    .padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(Icons.Default.Forum, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Spacer(modifier = Modifier.width(8.dp))
                Text("Channels (${channels.size})", style = MaterialTheme.typography.bodyLarge)
            }

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onOpenSkills() }
                    .padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(Icons.Default.Extension, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Spacer(modifier = Modifier.width(8.dp))
                Text("Skills", style = MaterialTheme.typography.bodyLarge)
            }

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onOpenBrowser() }
                    .padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(Icons.Default.Language, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Spacer(modifier = Modifier.width(8.dp))
                Text("Browser", style = MaterialTheme.typography.bodyLarge)
            }
        }
    }
}

@Composable
private fun QuickActionChip(action: JSONObject, viewModel: ArkaBotViewModel) {
    val label = action.optString("label", "Action")
    val icon = action.optString("icon", "⚡")
    Card(
        modifier = Modifier
            .clickable { viewModel.runQuickAction(action) },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(icon, style = MaterialTheme.typography.headlineSmall)
            Spacer(modifier = Modifier.width(8.dp))
            Text(label, style = MaterialTheme.typography.labelLarge)
        }
    }
}

@Composable
private fun AgentCard(agent: JSONObject, onClick: () -> Unit, onEdit: () -> Unit) {
    val avatar = agent.optString("avatar", "🤖")
    val color = agent.optString("avatar_color", "#7AA2F7")
    val avatarColor = parseHexColor(color)
    val name = agent.optString("name", "Unknown")
    val phase = agent.optString("phase", "idle")
    val phaseColor = when (phase) {
        "ready" -> Color(0xFF43BF71)
        "starting", "restarting" -> Color(0xFFE5AF8D)
        "blocked" -> Color(0xFFE46D7C)
        else -> Color(0xFF888888)
    }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(48.dp)
                    .clip(CircleShape)
                    .background(avatarColor),
                contentAlignment = Alignment.Center
            ) {
                Text(avatar, style = MaterialTheme.typography.headlineMedium)
            }
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                Text(phase, style = MaterialTheme.typography.labelSmall, color = phaseColor)
            }
            IconButton(onClick = onEdit) {
                Icon(Icons.Default.Settings, contentDescription = "Edit", modifier = Modifier.size(20.dp))
            }
        }
    }
}

/**
 * `androidx.compose.ui.graphics.Color` has no `parseColor`, so delegate to
 * `android.graphics.Color` and fall back to the default avatar color when the
 * stored value is not a valid `#RRGGBB`/`#AARRGGBB` string.
 */
private fun parseHexColor(value: String): Color = try {
    Color(android.graphics.Color.parseColor(value))
} catch (_: Exception) {
    Color(0xFF7AA2F7)
}
