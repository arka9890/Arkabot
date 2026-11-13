package com.arkabot.android.ui

import androidx.compose.foundation.layout.*
import androidx.navigation.compose.*
import androidx.navigation.compose.rememberNavController
import androidx.compose.material.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Dashboard
import androidx.compose.material.icons.filled.Actor
import androidx.compose.material.icons.filled.Forum
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material3.NavHost
import androidx.compose.material3.composable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.arkabot.android.data.ArkaBotViewModel
import org.json.JSONObject

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ArkaBotNavigation() {
    val navController = rememberNavController()
    val navBackStackEntry = navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry?.destination?.route

    LaunchedEffect(navController) {
        val start = Routes.DASHBOARD
        navController.navigate(start) {
            popUpTo(navController.graph.startDestinationId) {
                inclusive = true
            }
        }
    }

    val bottomBar: @Composable (PaddingValues) -> Unit = { padding ->
        BottomNavigation(
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp)
                .padding(horizontal = 8.dp),
            backgroundColor = MaterialTheme.colorScheme.surface,
            contentColor = MaterialTheme.colorScheme.onSurface,
            elevation = 8.dp
        ) {
            BottomNavigationItem(
                icon = { Icon(Icons.Default.Dashboard, contentDescription = "Dashboard") },
                label = { Text("Dashboard") },
                selected = currentRoute == Routes.DASHBOARD,
                onClick = {
                    navController.navigate(Routes.DASHBOARD) {
                        popUpTo(navController.graph.startDestinationId) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
            BottomNavigationItem(
                icon = { Icon(Icons.Default.Actor, contentDescription = "Agents") },
                label = { Text("Agents") },
                selected = currentRoute == Routes.AGENTS,
                onClick = {
                    navController.navigate(Routes.AGENTS) {
                        popUpTo(navController.graph.startDestinationId) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
            BottomNavigationItem(
                icon = { Icon(Icons.Default.Forum, contentDescription = "Channels") },
                label = { Text("Channels") },
                selected = currentRoute == Routes.CHANNELS,
                onClick = {
                    navController.navigate(Routes.CHANNELS) {
                        popUpTo(navController.graph.startDestinationId) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
            BottomNavigationItem(
                icon = { Icon(Icons.Default.Schedule, contentDescription = "Routines") },
                label = { Text("Routines") },
                selected = currentRoute == Routes.ROUTINES,
                onClick = {
                    navController.navigate(Routes.ROUTINES) {
                        popUpTo(navController.graph.startDestinationId) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
            BottomNavigationItem(
                icon = { Icon(Icons.Default.Event, contentDescription = "Events") },
                label = { Text("Events") },
                selected = currentRoute == Routes.EVENTS,
                onClick = {
                    navController.navigate(Routes.EVENTS) {
                        popUpTo(navController.graph.startDestinationId) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
            BottomNavigationItem(
                icon = { Icon(Icons.Default.Settings, contentDescription = "Settings") },
                label = { Text("Settings") },
                selected = currentRoute == Routes.SETTINGS,
                onClick = {
                    navController.navigate(Routes.SETTINGS) {
                        popUpTo(navController.graph.startDestinationId) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
            BottomNavigationItem(
                icon = { Icon(Icons.Default.ContentPaste, contentDescription = "Templates") },
                label = { Text("Templates") },
                selected = currentRoute == Routes.TEMPLATES,
                onClick = {
                    navController.navigate(Routes.TEMPLATES) {
                        popUpTo(navController.graph.startDestinationId) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
        }
    }

    Scaffold(
        bottomBar = bottomBar
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
        }
    }
}