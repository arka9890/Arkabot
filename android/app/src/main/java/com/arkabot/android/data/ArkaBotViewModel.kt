package com.arkabot.android.data

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
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
            }
        })
    }

    fun refreshAll() {
        launchIO { _agents.value = repo.listAgents() }
        launchIO { _channels.value = repo.listChannels() }
        launchIO { _routines.value = repo.listRoutines() }
        launchIO { _quickActions.value = repo.quickActions() }
        launchIO { _settings.value = repo.settings() }
        launchIO { _providerStatus.value = repo.checkProvider() }
    }

    fun refreshAgents() = launchIO { _agents.value = repo.listAgents() }
    fun refreshChannels() = launchIO { _channels.value = repo.listChannels() }
    fun refreshQuickActions() = launchIO { _quickActions.value = repo.quickActions() }

    fun createAgent(inputJson: String): JSONObject {
        return runBlockingIO { repo.createAgent(inputJson) }
    }

    fun updateAgent(id: String, inputJson: String): JSONObject {
        return runBlockingIO { repo.updateAgent(id, inputJson) }
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

    fun updateSettings(settingsJson: String): JSONObject {
        return runBlockingIO {
            val result = repo.updateSettings(settingsJson)
            _settings.value = repo.settings()
            result
        }
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
}
