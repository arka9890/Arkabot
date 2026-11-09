package com.arkabot.android.data

import com.arkabot.android.ArkaBotApplication
import org.json.JSONArray
import org.json.JSONObject
import uniffi.arkabot_ffi.EventListener
import uniffi.arkabot_ffi.ArkaBotHandle

/**
 * Thread-safe wrapper around the uniffi ArkaBotHandle.
 * All Rust calls are synchronous; we dispatch on Dispatchers.IO for the caller.
 */
class ArkaBotRepository(private val handle: ArkaBotHandle) {

    fun listAgents(): List<JSONObject> = parseArray(handle.listAgents())
    fun getAgent(id: String): JSONObject = parseObject(handle.getAgent(id))
    fun createAgent(inputJson: String): JSONObject = parseObject(handle.createAgent(inputJson))
    fun updateAgent(id: String, inputJson: String): JSONObject = parseObject(handle.updateAgent(id, inputJson))
    fun deleteAgent(id: String) = handle.deleteAgent(id)
    fun restartAgent(id: String) = handle.restartAgent(id)
    fun agentMessages(id: String, limit: Int = 50): List<JSONObject> = parseArray(handle.agentMessages(id, limit.toUInt()))
    fun sendMessage(agentId: String, text: String) = parseObject(handle.sendMessage(agentId, text, null))
    fun queueList(agentId: String): List<JSONObject> = parseArray(handle.queueList(agentId))
    fun queuePause(agentId: String) = handle.queuePause(agentId)
    fun queueResume(agentId: String) = handle.queueResume(agentId)
    fun queueCancel(itemId: String) = handle.queueCancel(itemId)

    fun listChannels(): List<JSONObject> = parseArray(handle.listChannels())
    fun createChannel(name: String, description: String, taskOwner: String, membersJson: String): JSONObject =
        parseObject(handle.createChannel(name, description, taskOwner, membersJson))
    fun deleteChannel(id: String) = handle.deleteChannel(id)
    fun channelMessages(channelId: String, limit: Int = 50): List<JSONObject> = parseArray(handle.channelMessages(channelId, limit.toUInt()))
    fun sendChannelMessage(channelId: String, text: String) = parseObject(handle.sendChannelMessage(channelId, text, null))
    fun channelAction(channelId: String, action: String, agentId: String?): JSONObject = parseObject(handle.channelAction(channelId, action, agentId))

    fun listRoutines(): List<JSONObject> = parseArray(handle.listRoutines())
    fun createRoutine(agentId: String, name: String, prompt: String, intervalMinutes: Long): JSONObject =
        parseObject(handle.createRoutine(agentId, name, prompt, intervalMinutes.toULong()))
    fun deleteRoutine(id: String) = handle.deleteRoutine(id)

    fun quickActions(): List<JSONObject> = parseArray(handle.quickActions())
    fun addQuickAction(label: String, prompt: String, icon: String, color: String): JSONObject =
        parseObject(handle.addQuickAction(label, prompt, icon, color))
    fun deleteQuickAction(id: String) = handle.deleteQuickAction(id)

    fun templates(): List<JSONObject> = parseArray(handle.templates())
    fun importTemplate(templateJson: String): JSONObject = parseObject(handle.importTemplate(templateJson))

    fun settings(): JSONObject = parseObject(handle.settings())
    fun updateSettings(settingsJson: String): JSONObject = parseObject(handle.updateSettings(settingsJson))
    fun checkProvider(): JSONObject = parseObject(handle.checkProvider())

    fun filesTree(agentId: String): JSONObject = parseObject(handle.filesTree(agentId))
    fun workspaceRead(agentId: String, path: String): String = handle.workspaceRead(agentId, path)
    fun workspaceWrite(agentId: String, path: String, content: String) = handle.workspaceWrite(agentId, path, content)
    fun workspaceCreateDir(agentId: String, path: String) = handle.workspaceCreateDir(agentId, path)
    fun workspaceDelete(agentId: String, path: String) = handle.workspaceDelete(agentId, path)

    fun search(query: String): List<JSONObject> = parseArray(handle.search(query))
    fun exportData(targetDir: String): JSONObject = parseObject(handle.exportData(targetDir))
    fun importData(sourceDir: String): JSONObject = parseObject(handle.importData(sourceDir))

    fun registerListener(listener: EventListener) {
        handle.registerListener(listener)
    }

    private fun parseArray(json: String): List<JSONObject> {
        if (json.isBlank()) return emptyList()
        return try {
            val arr = JSONArray(json)
            (0 until arr.length()).map { arr.getJSONObject(it) }
        } catch (e: Exception) {
            emptyList()
        }
    }

    private fun parseObject(json: String): JSONObject {
        if (json.isBlank()) return JSONObject()
        return try {
            JSONObject(json)
        } catch (e: Exception) {
            JSONObject()
        }
    }
}
