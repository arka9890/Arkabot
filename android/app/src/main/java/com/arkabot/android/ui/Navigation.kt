package com.arkabot.android.ui

import androidx.compose.runtime.Composable
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.arkabot.android.ui.screens.AgentChatScreen
import com.arkabot.android.ui.screens.AgentEditScreen
import com.arkabot.android.ui.screens.BrowserScreen
import com.arkabot.android.ui.screens.ChannelsScreen
import com.arkabot.android.ui.screens.DashboardScreen
import com.arkabot.android.ui.screens.EventsScreen
import com.arkabot.android.ui.screens.RoutinesScreen
import com.arkabot.android.ui.screens.SettingsScreen
import com.arkabot.android.ui.screens.SkillsScreen
import com.arkabot.android.ui.screens.TemplatesScreen

object Routes {
    const val DASHBOARD = "dashboard"
    const val AGENT_CHAT = "agent_chat/{agentId}"
    const val AGENT_EDIT = "agent_edit/{agentId}"
    const val AGENT_EDIT_NEW = "new" // sentinel: "agent_edit/new" = create mode
    const val CHANNELS = "channels"
    const val ROUTINES = "routines"
    const val SETTINGS = "settings"
    const val EVENTS = "events"
    const val TEMPLATES = "templates"
    const val SKILLS = "skills"
    const val BROWSER = "browser"
}

@Composable
fun ArkaBotNavigation() {
    val navController = rememberNavController()
    NavHost(navController = navController, startDestination = Routes.DASHBOARD) {
        composable(Routes.DASHBOARD) {
            DashboardScreen(
                onOpenChat = { agentId -> navController.navigate("agent_chat/$agentId") },
                onOpenEdit = { agentId ->
                    navController.navigate("agent_edit/${agentId ?: Routes.AGENT_EDIT_NEW}")
                },
                onOpenChannels = { navController.navigate(Routes.CHANNELS) },
                onOpenRoutines = { navController.navigate(Routes.ROUTINES) },
                onOpenSettings = { navController.navigate(Routes.SETTINGS) },
                onOpenEvents = { navController.navigate(Routes.EVENTS) },
                onOpenTemplates = { navController.navigate(Routes.TEMPLATES) },
                onOpenSkills = { navController.navigate(Routes.SKILLS) },
                onOpenBrowser = { navController.navigate(Routes.BROWSER) },
            )
        }
        composable("agent_chat/{agentId}") {
            val agentId = it.arguments?.getString("agentId") ?: ""
            AgentChatScreen(agentId = agentId, onBack = { navController.popBackStack() })
        }
        composable(Routes.AGENT_EDIT) {
            val raw = it.arguments?.getString("agentId")
            val agentId = raw?.takeIf { id -> id != Routes.AGENT_EDIT_NEW }
            AgentEditScreen(agentId = agentId, onBack = { navController.popBackStack() })
        }
        composable(Routes.CHANNELS) {
            ChannelsScreen(onBack = { navController.popBackStack() })
        }
        composable(Routes.ROUTINES) {
            RoutinesScreen(onBack = { navController.popBackStack() })
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(onBack = { navController.popBackStack() })
        }
        composable(Routes.EVENTS) {
            EventsScreen(onBack = { navController.popBackStack() })
        }
        composable(Routes.TEMPLATES) {
            TemplatesScreen(onBack = { navController.popBackStack() })
        }
        composable(Routes.SKILLS) {
            SkillsScreen(onBack = { navController.popBackStack() })
        }
        composable(Routes.BROWSER) {
            BrowserScreen(onBack = { navController.popBackStack() })
        }
    }
}
