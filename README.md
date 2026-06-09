# Openman

AI-assisted coding environment.

## Features

- **Canvas-based session management** — Visual graph of AI coding sessions with drag-and-drop nodes
- **Multi-provider AI support** — Configure Anthropic, OpenAI, Minimax providers with API keys and default models
- **Dark/Light theme** — Toggle between themes in settings
- **Session groups** — Organize sessions into collapsible groups
- **Real-time streaming** — Live message updates during agent execution
- **Project management** — Create and switch between projects

## Architecture

**Frontend** (React + TypeScript + Vite)
- AppShell — Root layout with sidebar
- SessionWorkspace — Canvas + chat UI
- SettingsPage — Theme + provider config

**Backend** (Rust + Tauri)
- Commands — Tauri IPC handlers (settings, sessions, providers)
- Services — Business logic (agent, project, settings)
- Agent — LLM provider integration (anthropic, openai, minimax)

**State** (Zustand)
- appStore — View state, theme preference
- projectStore — Projects, current selection
- sessionStore — Sessions, groups, active session
- conversationStore — Messages, streaming state