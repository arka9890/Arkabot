# ArkaBot Android

A local-first Android AI agent workspace — a Rust reimplementation of the ArkaBot desktop experience, running entirely on-device.

## Features

- **Agents** — Create, edit, and chat with AI agents. Each agent has its own workspace, instructions (AGENTS.md), and model settings.
- **Channels** — Multi-agent collaboration channels where agents can delegate tasks to each other.
- **Routines** — Scheduled prompts that run agent tasks on a recurring interval.
- **Task Queue** — Per-agent queue with pause/resume/cancel controls.
- **Quick Actions** — One-tap prompt shortcuts.
- **Templates** — Pre-configured agent templates for quick setup.
- **Files** — Workspace file tree, read/write/delete.
- **Events** — Real-time event feed from the Rust runtime.
- **Notifications** — Foreground service for agent activity notifications.

## Architecture

```
Rust Core (workspace crates)
├── openbot-core      — domain models, IDs, error types
├── storage           — SQLite persistence (sled-compatible)
├── llm               — OpenAI-compatible API client
├── agent-engine      — conversation loop, tool execution
├── messaging         — channel/message routing
├── browser           — browser control interface
├── server            — HTTP API (not used on Android)
├── app-core          — high-level App orchestration
└── ffi               — uniffi FFI bindings → Kotlin

Android App (Kotlin + Compose)
├── ArkaBotApplication — creates the Rust runtime
├── MainActivity       — Compose navigation host
├── data/              — Repository + ViewModel
├── ui/                — Compose screens
└── notifications/     — AgentNotificationService
```

## Provider Configuration

The app supports any OpenAI-compatible API endpoint:

| Provider | Base URL | Notes |
|----------|----------|-------|
| Simulated | — | No server needed, built-in demo mode |
| Ollama | `http://10.0.2.2:11434` (emulator) or `http://localhost:11434` | Local models |
| LM Studio | `http://10.0.2.2:1234` (emulator) | Local models |
| OpenAI-compatible | Any URL | OpenAI, Azure, Groq, Together, etc. |

## Building

### Prerequisites

- Rust (stable toolchain)
- Android Studio (Gradle 8.7+, AGP 8.7.3, NDK 27.0.12077973)
- cargo-ndk (`cargo install cargo-ndk`)
- Java 17

### Build Debug APK

```bash
# 1. Build Rust for all Android ABIs
bash scripts/build-rust-android.sh

# 2. Generate Kotlin bindings
bash scripts/gen-kotlin.sh

# 3. Build the APK
cd android && ./gradlew assembleDebug
```

The APK will be at `android/app/build/outputs/apk/debug/app-debug.apk`.

### Build Release APK

```bash
export ANDROID_KEYSTORE_FILE=/path/to/keystore.jks
export ANDROID_KEYSTORE_PASSWORD=yourpassword
export ANDROID_KEY_ALIAS=youralias
export ANDROID_KEY_PASSWORD=yourkeypassword

cd android && ./gradlew assembleRelease
```

## Push to GitHub

```bash
# Create a new repository (or use an existing one)
gh repo create arkabot-android --private --clone

# Or manually:
git init
git add .
git commit -m "Initial commit: ArkaBot Android"
git branch -M main
git remote add origin https://github.com/YOUR_USERNAME/arkabot-android.git
git push -u origin main
```

## Demo Mode

By default, the app runs in **Demo Mode** with the simulated provider — no API key or server required. Agents will respond with simulated messages. To use a real provider, go to Settings and configure your endpoint.

## License

MIT
