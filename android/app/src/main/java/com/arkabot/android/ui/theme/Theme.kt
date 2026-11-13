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
    surface = Color(0xFF13141C),
    surfaceVariant = Color(0xFF24283B),
    onBackground = Color(0xFFA9B1D6),
    onSurface = Color(0xFFA9B1D4),
    onPrimary = Color(0xFF1A1B26),
    onSecondary = Color(0xFF1A1B26),
    onTertiary = Color(0xFF1A1B26),
    outline = Color(0xFF414868),
    error = Color(0xFFF7768E),
)

private val LightColors = lightColorScheme(
    primary = Color(0xFF3B5BDB),
    secondary = Color(0xFF6B5B95),
    tertiary = Color(0xFFD67E5A),
    background = Color(0xFFFFFFFF),
    surface = Color(0xFFFFFFFF),
    surfaceVariant = Color(0xFFF0F0F0),
    onBackground = Color(0xFF1A1B26),
    onSurface = Color(0xFF1A1B26),
    onPrimary = Color(0xFFFFFFFF),
    onSecondary = Color(0xFFFFFFFF),
    onTertiary = Color(0xFFFFFFFF),
    outline = Color(0xFF414868),
    error = Color(0xFFF7768E),
)

@Composable
fun ArkaBotTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        content = content
    )
}
