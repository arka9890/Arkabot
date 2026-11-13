package com.arkabot.android.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Dashboard
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.Forum
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.SmartToy
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.arkabot.android.ui.screens.AgentChatScreen
import com.arkabot.android.ui.screens.AgentEditScreen
import com.arkabot.android.ui.screens.ChannelsScreen
import com.arkabot.android.ui.screens.DashboardScreen
import com.arkabot.android.ui.screens.EventsScreen
import com.arkabot.android.ui.screens.RoutinesScreen
import com.arkabot.android.ui.screens.SettingsScreen
import com.arkabot.android.ui.screens.TemplatesScreen

object Routes {
    const val DASHBOARD = "dashboard"
    const val AGENTS = "agents"
    const val AGENT_CHAT = "agent_chat/{agentId}"
    const val AGENT_EDIT = "agent_edit/{agentId}"
    const val AGENT_EDIT_NEW = "new" // sentinel: "agent_edit/new" = create mode
    const val CHANNELS = "channels"
    const val ROUTINES = "routines"
    const val SETTINGS = "settings"
    const val EVENTS = "events"
    const val TEMPLATES = "templates"
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ArkaBotNavigation() {
    val navController = rememberNavController()
    val backStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = backStackEntry?.destination?.route

    Scaffold(
        bottomBar = {
            NavigationBar(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp),
                containerColor = Color.Unspecified
            ) {
                navigationTab(
                    navController = navController,
                    route = Routes.DASHBOARD,
                    currentRoute = currentRoute,
                    contentDescription = "Dashboard",
                    label = "Dashboard"
                ) { Icon(Icons.Filled.Dashboard, contentDescription = "Dashboard") }

                navigationTab(
                    navController = navController,
                    route = Routes.AGENTS,
                    currentRoute = currentRoute,
                    contentDescription = "Agents",
                    label = "Agents"
                ) { Icon(Icons.Filled.SmartToy, contentDescription = "Agents") }

                navigationTab(
                    navController = navController,
                    route = Routes.CHANNELS,
                    currentRoute = currentRoute,
                    contentDescription = "Channels",
                    label = "Channels"
                ) { Icon(Icons.Filled.Forum, contentDescription = "Channels") }

                navigationTab(
                    navController = navController,
                    route = Routes.ROUTINES,
                    currentRoute = currentRoute,
                    contentDescription = "Routines",
                    label = "Routines"
                ) { Icon(Icons.Filled.Schedule, contentDescription = "Routines") }

                navigationTab(
                    navController = navController,
                    route = Routes.EVENTS,
                    currentRoute = currentRoute,
                    contentDescription = "Events",
                    label = "Events"
                ) { Icon(Icons.Filled.Event, contentDescription = "Events") }

                navigationTab(
                    navController = navController,
                    route = Routes.SETTINGS,
                    currentRoute = currentRoute,
                    contentDescription = "Settings",
                    label = "Settings"
                ) { Icon(Icons.Filled.Settings, contentDescription = "Settings") }

                navigationTab(
                    navController = navController,
                    route = Routes.TEMPLATES,
                    currentRoute = currentRoute,
                    contentDescription = "Templates",
                    label = "Templates"
                ) { Icon(Icons.Filled.ContentPaste, contentDescription = "Templates") }
            }
        }
    ) { padding ->
        NavHost(
            navController = navController,
            startDestination = Routes.DASHBOARD,
            modifier = Modifier
                .fillMaxSize()
                .padding(bottom = padding.calculateBottomPadding())
        ) {
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
                )
            }
            composable(Routes.AGENTS) {
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
                )
            }
            composable(Routes.AGENT_CHAT) {
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
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun androidx.compose.foundation.layout.RowScope.navigationTab(
    navController: NavHostController,
    route: String,
    currentRoute: String?,
    contentDescription: String,
    label: String,
    icon: @Composable () -> Unit
) {
    NavigationBarItem(
        icon = icon,
        label = { Text(label) },
        selected = currentRoute == route,
        onClick = {
            navController.navigate(route) {
                popUpTo(navController.graph.startDestinationId) {
                    saveState = true
                }
                launchSingleTop = true
                restoreState = true
            }
        }
    )
}
