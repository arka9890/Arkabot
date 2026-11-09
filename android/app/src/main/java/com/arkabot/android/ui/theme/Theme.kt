package com.arkabot.android.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val DarkColors = darkColorScheme(
    primary = Color(0xFF7AA2F7),
    secondary = Color(0xFFBB9CE7),
    tertiary = Color(0xFFE5AF8D),
    background = Color(0xFF1A1B26),
    surface = Color(0xFF1F2033),
    surfaceVariant = Color(0xFF2A2B3F),
    onBackground = Color(0xFFC0C5D4),
    onSurface = Color(0xFFC0C5D4),
)

private val LightColors = lightColorScheme(
    primary = Color(0xFF3B5BDB),
    secondary = Color(0xFF6B5B95),
    tertiary = Color(0xFFD67E5A),
    background = Color(0xFFFAFAFA),
    surface = Color(0xFFFFFFFF),
    surfaceVariant = Color(0xFFF0F0F0),
    onBackground = Color(0xFF1A1B26),
    onSurface = Color(0xFF1A1B26),
)

@Composable
fun ArkaBotTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        content = content
    )
}
