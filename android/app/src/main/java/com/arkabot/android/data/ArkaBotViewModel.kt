package com.arkabot.android.data

import android.Manifest
import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.arkabot.android.MainActivity
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uniffi.arkabot_ffi.EventListener

data class EventItem(
    val kind: String,
    val agentId: String?,
    val channelId: String?,
    val payload: String,
    val timestamp: Long = System.currentTimeMillis()
)

class ArkaBotViewModel(app: Application) : AndroidViewModel(app) {

    private val repo = ArkaBotRepository((app as com.arkabot.android.ArkaBotApplication).openbot)

    private val _agents = MutableStateFlow<List<JSONObject>>(emptyList())
    val agents: StateFlow<List<JSONObject>> = _agents.asStateFlow()

    private val _channels = MutableStateFlow<List<JSONObject>>(emptyList())
    val channels: StateFlow<List<JSONObject>> = _channels.asStateFlow()

    private val _routines = MutableStateFlow<List<JSONObject>>(emptyList())
    val routines: StateFlow<List<JSONObject>> = _routines.asStateFlow()

    private val _quickActions = MutableStateFlow<List<JSONObject>>(emptyList())
    val quickActions: StateFlow<List<JSONObject>> = _quickActions.asStateFlow()

    private val _templates = MutableStateFlow<List<JSONObject>>(emptyList())
    val templates: StateFlow<List<JSONObject>> = _templates.asStateFlow()

    private val _settings = MutableStateFlow<JSONObject>(JSONObject())
    val settings: StateFlow<JSONObject> = _settings.asStateFlow()

    private val _providerStatus = MutableStateFlow<JSONObject>(JSONObject())
    val providerStatus: StateFlow<JSONObject> = _providerStatus.asStateFlow()

    private val _events = MutableStateFlow<List<EventItem>>(emptyList())
    val events: StateFlow<List<EventItem>> = _events.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    init {
        refreshAll()
        repo.registerListener(object : EventListener {
            override fun onEvent(kind: String, agentId: String?, channelId: String?, payload: String) {
                _events.value = _events.value + EventItem(kind, agentId, channelId, payload)
                if (kind == "message_added") notifyAgentReply(agentId, payload)
            }
        })
    }

    fun refreshAll() {
        launchIO { _agents.value = repo.listAgents() }
        launchIO { _channels.value = repo.listChannels() }
        launchIO { _routines.value = repo.listRoutines() }
        launchIO { _quickActions.value = repo.quickActions() }
        launchIO { _templates.value = repo.templates() }
        launchIO { _settings.value = repo.settings() }
        launchIO { _providerStatus.value = repo.checkProvider() }
    }

    fun refreshAgents() = launchIO { _agents.value = repo.listAgents() }
    fun refreshChannels() = launchIO { _channels.value = repo.listChannels() }
    fun refreshQuickActions() = launchIO { _quickActions.value = repo.quickActions() }
    fun refreshTemplates() = launchIO { _templates.value = repo.templates() }

    fun createAgent(inputJson: String, onDone: (JSONObject) -> Unit = {}) = launchIO {
        val created = repo.createAgent(inputJson)
        // Refresh the list: without this the new agent never appears on the
        // dashboard until a manual refresh.
        _agents.value = repo.listAgents()
        onDone(created)
    }

    fun updateAgent(id: String, inputJson: String, onDone: (JSONObject) -> Unit = {}) = launchIO {
        val updated = repo.updateAgent(id, inputJson)
        _agents.value = repo.listAgents()
        onDone(updated)
    }

    fun deleteAgent(id: String) {
        launchIO {
            repo.deleteAgent(id)
            _agents.value = repo.listAgents()
        }
    }

    fun restartAgent(id: String) {
        launchIO {
            repo.restartAgent(id)
            _agents.value = repo.listAgents()
        }
    }

    fun sendMessage(agentId: String, text: String) {
        launchIO {
            repo.sendMessage(agentId, text)
        }
    }

    fun agentMessages(agentId: String): List<JSONObject> {
        return runBlockingIO { repo.agentMessages(agentId) }
    }

    /** Non-blocking variant for Compose `LaunchedEffect` callers. */
    suspend fun loadMessages(agentId: String, limit: Int = 100): List<JSONObject> =
        withContext(Dispatchers.IO) { repo.agentMessages(agentId, limit) }

    /** Create a new agent pre-filled from a marketplace template. */
    fun createFromTemplate(template: JSONObject) {
        launchIO {
            val input = JSONObject().apply {
                put("name", template.optString("name", "Agent"))
                put("description", template.optString("description", ""))
                put("avatar", template.optString("avatar", "🤖"))
                put("avatar_color", template.optString("avatar_color", "#7AA2F7"))
                put("instructions", template.optString("instructions", ""))
                put("provider", template.optString("provider", "simulated"))
                put("model", template.optString("model", ""))
                put("temperature", template.optDouble("temperature", 0.7))
                put("max_tool_iterations", 8)
                put("compaction_messages", 60)
                put("busy_mode", "queue")
                put(
                    "capabilities",
                    JSONArray().apply {
                        put("chat")
                        put("files")
                        put("task_queue")
                        put("notifications")
                    }
                )
            }.toString()
            repo.createAgent(input)
            _agents.value = repo.listAgents()
        }
    }

    fun createChannel(name: String, description: String, taskOwner: String, membersJson: String) {
        launchIO {
            repo.createChannel(name, description, taskOwner, membersJson)
            _channels.value = repo.listChannels()
        }
    }

    fun sendChannelMessage(channelId: String, text: String) {
        launchIO {
            repo.sendChannelMessage(channelId, text)
        }
    }

    fun updateSettings(settingsJson: String, onDone: () -> Unit = {}) = launchIO {
        // Non-blocking: the Rust FFI call used to run a network health check
        // with a 300 s timeout on this thread, freezing the UI (blank screen).
        repo.updateSettings(settingsJson)
        _settings.value = repo.settings()
        _providerStatus.value = repo.checkProvider()
        onDone()
    }

    fun runQuickAction(action: JSONObject) {
        launchIO {
            val prompt = action.optString("prompt", "")
            val agents = repo.listAgents()
            if (agents.isNotEmpty()) {
                repo.sendMessage(agents[0].getString("id"), prompt)
            }
        }
    }

    fun clearError() {
        _error.value = null
    }

    /**
     * Post a system notification when an agent replies. Runs off the main
     * thread (event callbacks come from the Rust runtime) and respects the
     * POST_NOTIFICATIONS runtime permission on Android 13+.
     */
    private fun notifyAgentReply(agentId: String?, payload: String) {
        try {
            val data = JSONObject(payload)
            if (data.optString("sender") != "agent") return
            val text = data.optString("text", "").trim()
            if (text.isEmpty()) return

            val app = getApplication<Application>()
            val name = _agents.value
                .firstOrNull { it.optString("id") == agentId }
                ?.optString("name", "Agent") ?: "Agent"

            val manager = NotificationManagerCompat.from(app)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                manager.createNotificationChannel(
                    NotificationChannel(
                        CHANNEL_AGENT_REPLIES,
                        "Agent Replies",
                        NotificationManager.IMPORTANCE_DEFAULT
                    )
                )
            }

            val pendingIntent = PendingIntent.getActivity(
                app,
                0,
                Intent(app, MainActivity::class.java),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )

            val notification = NotificationCompat.Builder(app, CHANNEL_AGENT_REPLIES)
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setContentTitle(name)
                .setContentText(text)
                .setAutoCancel(true)
                .setContentIntent(pendingIntent)
                .build()

            val canPost = Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
                ContextCompat.checkSelfPermission(app, Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
            if (canPost) {
                manager.notify(System.currentTimeMillis().toInt(), notification)
            }
        } catch (_: Exception) {
            // Never let notification problems break the event stream.
        }
    }

    private fun launchIO(block: suspend () -> Unit) {
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) { block() }
            } catch (e: Exception) {
                _error.value = e.message ?: e.toString()
            }
        }
    }

    private fun <T> runBlockingIO(block: suspend () -> T): T {
        var result: T? = null
        var error: Throwable? = null
        kotlinx.coroutines.runBlocking {
            try {
                result = withContext(Dispatchers.IO) { block() }
            } catch (e: Throwable) {
                error = e
            }
        }
        error?.let { throw it }
        return result!!
    }

    companion object {
        private const val CHANNEL_AGENT_REPLIES = "agent_replies"
    }
}
