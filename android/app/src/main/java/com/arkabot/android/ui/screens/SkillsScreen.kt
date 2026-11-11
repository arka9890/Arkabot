package com.arkabot.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel
import org.json.JSONObject

/**
 * The local skill library: reusable `SKILL.md` workflows that an agent can
 * install into its workspace.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SkillsScreen(onBack: () -> Unit, viewModel: ArkaBotViewModel = viewModel()) {
    val skills by viewModel.skills.collectAsState()
    val agents by viewModel.agents.collectAsState()
    val selectedAgentId by viewModel.skillAgentId.collectAsState()
    val notice by viewModel.notice.collectAsState()
    val error by viewModel.error.collectAsState()

    var editorOpen by remember { mutableStateOf(false) }
    var editingId by remember { mutableStateOf("") }
    var editingName by remember { mutableStateOf("") }
    var editingBody by remember { mutableStateOf("") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Skills") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    TextButton(onClick = {
                        editingId = ""
                        editingName = ""
                        editingBody = "---\nname: \ndescription: \nversion: 1\n---\n\n"
                        editorOpen = true
                    }) { Text("Baru") }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            Text("Agent", fontWeight = FontWeight.Bold)
            Spacer(Modifier.height(4.dp))
            // Which agent the install/uninstall buttons act on.
            SingleChoiceSegmentedButtonRow {
                agents.take(4).forEach { agent ->
                    SegmentedButton(
                        selected = selectedAgentId == agent.optString("id"),
                        onClick = { viewModel.selectSkillAgent(agent.optString("id")) },
                        shape = SegmentedButtonDefaults.itemShape(0, agents.size.coerceAtMost(4))
                    ) {
                        Text(agent.optString("avatar", "🤖") + " " + agent.optString("name", ""))
                    }
                }
            }
            if (agents.isEmpty()) {
                Text(
                    "Buat agent dulu untuk memasang skill.",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Spacer(Modifier.height(12.dp))

            if (notice != null) {
                Text(
                    notice!!,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary
                )
                Spacer(Modifier.height(8.dp))
            }
            if (error != null) {
                Text(
                    "Gagal: $error",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error
                )
                Spacer(Modifier.height(8.dp))
            }

            if (skills.isEmpty()) {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)
                ) {
                    Column(
                        modifier = Modifier.padding(24.dp),
                        horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally
                    ) {
                        Text("Belum ada skill", style = MaterialTheme.typography.titleMedium)
                        Text(
                            "Skill adalah workflow SKILL.md yang bisa dipasang ke agent.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            } else {
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(skills) { skill ->
                        SkillCard(
                            skill = skill,
                            onInstall = { viewModel.installSkill(skill.optString("id")) },
                            onUninstall = { viewModel.uninstallSkill(skill.optString("id")) },
                            onEdit = {
                                editingId = skill.optString("id")
                                editingName = skill.optString("name", "")
                                editingBody = skill.optString("body", "")
                                editorOpen = true
                            },
                            onDelete = { viewModel.deleteSkill(skill.optString("id")) }
                        )
                    }
                }
            }
        }
    }

    if (editorOpen) {
        AlertDialog(
            onDismissRequest = { editorOpen = false },
            title = { Text(if (editingId.isEmpty()) "Skill Baru" else "Edit ${editingName}") },
            text = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(320.dp)
                        .verticalScroll(rememberScrollState())
                ) {
                    OutlinedTextField(
                        value = editingName,
                        onValueChange = { editingName = it },
                        label = { Text("Nama skill (id folder)") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true
                    )
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(
                        value = editingBody,
                        onValueChange = { editingBody = it },
                        label = { Text("Isi SKILL.md") },
                        modifier = Modifier.fillMaxWidth(),
                        minLines = 10
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    val id = editingName.trim().lowercase().replace(Regex("[^a-z0-9._-]"), "-")
                    if (id.isNotEmpty()) {
                        viewModel.saveSkill(id, editingBody, null)
                    }
                    editorOpen = false
                }) { Text("Simpan") }
            },
            dismissButton = {
                TextButton(onClick = { editorOpen = false }) { Text("Batal") }
            }
        )
    }
}

@Composable
private fun SkillCard(
    skill: JSONObject,
    onInstall: () -> Unit,
    onUninstall: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit
) {
    val installed = skill.optBoolean("installed", false)
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        skill.optString("name", "Skill"),
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold
                    )
                    val description = skill.optString("description", "")
                    if (description.isNotEmpty()) {
                        Text(
                            description,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Text(
                        "versi ${skill.optString("version", "1")} · id ${skill.optString("id", "")}",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                if (installed) {
                    AssistChip(onClick = onUninstall, label = { Text("Lepas") })
                } else {
                    AssistChip(onClick = onInstall, label = { Text("Pasang") })
                }
            }
            Row {
                TextButton(onClick = onEdit) { Text("Edit") }
                TextButton(onClick = onDelete) { Text("Hapus") }
            }
        }
    }
}
