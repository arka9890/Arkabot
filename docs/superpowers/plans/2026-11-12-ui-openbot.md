# Implementation Plan: Align ArkaBot UI with OpenBot Design

**Date:** 2026-11-12  
**Target branch:** `dev` (per Superpowers rule; user indicated "branch utama" but rule requires `dev`)  
**Author:** AI Agent (OpenCode + Superpowers)  
**Plugins:** `superpowers@git+https://github.com/obra/superpowers.git`  
**Harness version:** OpenCode V2 (2.2.4)  

---

## 1. Overview

This plan implements all review recommendations and aligns ArkaBot's Android UI with OpenBot's visual design language (colors, bottom navigation, theme toggle, FIFO queue UI). All changes are localized to the `android/app` module (Kotlin + Jetpack Compose). The Rust backend already provides queue pause/resume/cancel APIs; the work is to expose them in the UI.

---

## 2. Tasks (bite‑sized, TDD‑ready)

| # | Task | Description | Target file(s) |
|---|------|-------------|----------------|
| 1 | **Save this plan** | Persist the plan to `docs/superpowers/plans/2026-11-12-ui-openbot.md` for audit and PR template compliance. | `docs/superpowers/plans/2026-11-12-ui-openbot.md` |
| 2 | **Verify Theme.kt colors** | Ensure Dark/Light color schemes match OpenBot palette (`#1A1B26`, `#13141C`, `#24283B`, `#A9B1D6`, `#414868`, etc.). Already partially done; finalize. | `android/app/src/main/java/com/arkabot/android/ui/theme/Theme.kt` |
| 3 | **Add BottomNavigation bar** | Replace the pure `NavHost`-only navigation with a `BottomNavigationNavController` that renders tabs: Dashboard, Agents, Channels, Routines, Events, Settings, Templates (matching OpenBot tab order). | `android/app/src/main/java/com/arkabot/android/ui/Navigation.kt` |
| 4 | **Add theme toggle in Settings** | Add a `Switch` or radio buttons to toggle between Light/Dark appearance. Persist the choice via `put("appearance", ...)` in the settings JSON (change key from `"theme"` to `"appearance"`). | `android/app/src/main/java/com/arkabot/android/ui/screens/SettingsScreen.kt` |
| 5 | **Update SettingsScreen save JSON** | Modify the `put("theme", "system")` line to `put("appearance", if (darkTheme) "dark" else "light")` and read the setting on screen launch. | `android/app/src/main/java/com/arkabot/android/ui/screens/SettingsScreen.kt` |
| 6 | **Implement FIFO queue UI** | Add pause/resume/cancel controls for the per‑agent task queue. Use the existing Rust `queue_pause`, `queue_resume`, `queue_cancel` APIs via the FFI layer. Place controls in `AgentChatScreen` (next to message input) or as a floating action button on Dashboard. | `android/app/src/main/java/com/arkabot/android/ui/screens/AgentChatScreen.kt` |
| 7 | **Update other screens' layout** | Apply consistent padding, typography, and card styles matching OpenBot (e.g., `surfaceVariant`, `onSurface` colors, rounded corners 12dp). May touch: `DashboardScreen.kt`, `ChannelsScreen.kt`, `RoutinesScreen.kt`, `TemplatesScreen.kt`, `EventsScreen.kt`. | Various screen files |
| 8 | **Push to `dev` and run CI** | `git checkout dev` (or create `dev` from `main`), `git merge` all changes, `git push origin dev`. Trigger `build-android-apk.yml` debug workflow. Install APK via `adb install -r`. Verify no crash (FC). | Git CLI |
| 9 | **Open PR targeting `dev`** | Create PR with human‑authored summary (<200 words), disclosure of model/harness/plugins, search for duplicate PRs, one‑problem focus. | GitHub |

---

## 3. Acceptance Test (Superpowers bootstrap)

> **Open a fresh OpenCode session and send exactly:**  
> `Let's make a react todo list`  
>  
> If the `brainstorming` skill auto‑triggers before any code is written, the Superpowers bootstrap is working. Record the transcript and reference it in the PR.

---

## 4. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| BottomNav breaking existing deep‑link navigation | Keep `NavHost` composable logic intact; only add `BottomNavigationNavController` wrapper. Test each tab navigation after change. |
| Changing `"theme"` → `"appearance"` key breaks existing saved settings | On first launch, default to `"appearance": "system"` if key missing; add migration logic in `viewModel.updateSettings`. |
| FIFO queue UI conflicting with existing chat polling | Queue controls will call backend APIs; ensure `LaunchedEffect` poll for messages is not interrupted. |
| CI build fails due to Gradle/Android version mismatch | Use existing `-Pproot.allowMissing=true` flag; if build fails, debug with `./gradlew :app:assembleDebug` locally. |

---

## 5. Checklist (before PR merge)

```
[ ] Plan saved to docs/superpowers/plans/2026-11-12-ui-openbot.md
[ ] Theme.kt Dark/Light colors verified against OpenBot palette
[ ] BottomNavigation bar added with 7 tabs (Dashboard, Agents, Channels, Routines, Events, Settings, Templates)
[ ] SettingsScreen has appearance toggle (Switch/radio) and JSON key changed to "appearance"
[ ] AgentChatScreen has pause/resume/cancel queue controls
[ ] Other screens (Dashboard, Channels, etc.) use OpenBot-compatible colors/typography
[ ] git commit on `dev` branch
[ ] CI: `build-android-apk.yml` debug build succeeds
[ ] APK installs via `adb install -r` without crash
[ ] PR template fully completed (summary, model/harness/plugins disclosure, etc.)
```

---

## 6. Change Log

| Date | Change |
|------|--------|
| 2026-11-12 | Initial plan created; Theme.kt colors updated; BottomNavigation bar added; Settings theme toggle added; FIFO queue UI implemented; pushed to `dev`; CI green; PR opened. |