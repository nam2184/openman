# Openman

An open-source AI coding agent with peer to peer session context aggregation and snippet focused interjection built with Rust and Tauri.

## Features

- **Canvas-based session management** — Visual graph of AI coding sessions with drag-and-drop nodes
- **Dark/Light theme** — Toggle between themes in settings
- **Session groups** — Organize sessions into collapsible groups
- **Real-time streaming** — Live message updates during agent execution
- **Project management** — Create and switch between projects

## Providers

Openman supports multiple LLM providers through a protocol-based architecture. Each provider implements one of two API systems:

### OpenAI-Compatible Chat (`Protocol::OpenAI`)

Providers that expose an OpenAI Chat Completions-compatible endpoint at `/v1/chat/completions`. These reuse a shared streaming transport and SSE parsing implementation.

| Provider | Base URL | Notes |
|----------|----------|-------|
| **OpenAI** | `https://api.openai.com/v1` | GPT-4o, GPT-4o-mini, o3, o4-mini |
| **MiniMax Token Plan** | `https://api.minimax.io/v1` | MiniMax-M3, MiniMax-M2.7, MiniMax-M2.5 |

### Anthropic Messages (`Protocol::Anthropic`)

Providers that implement the Anthropic Messages API at `/v1/messages`, with support for streaming, tool use, and extended thinking.

| Provider | Base URL | Notes |
|----------|----------|-------|
| **Anthropic** | `https://api.anthropic.com/v1` | Claude Sonnet, Claude Opus, Claude Haiku |

### Provider Configuration

Each provider is configured via `ProviderConfig` which stores:

- `name` — Provider identifier (e.g. `"openai"`, `"anthropic"`, `"minimax"`)
- `model` — Default model ID (e.g. `"gpt-4o"`, `"MiniMax-M3"`)
- `api_key` — API key (from config or environment variable)
- `base_url` — Optional custom endpoint override
- `protocol` — Which API system to use (`OpenAI` or `Anthropic`)
- `enabled` — Whether to use as default for new sessions

Environment variable fallbacks:

| Provider | Environment Variable |
|----------|---------------------|
| OpenAI | `OPENAI_API_KEY` |
| MiniMax Token Plan | `MINIMAX_TOKEN_PLAN_KEY` or `MINIMAX_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |

## Architecture

**Frontend** — React + TypeScript + Vite + Zustand

**Backend** — Rust + Tauri (`src-tauri/`) + `openman-agents` crate (`agents/`)

The `agents/` crate contains:
- `llm/providers/` — Provider implementations with shared `OpenAiCompatibleChatProvider` API-system
- `database/` — SQLite persistence via `rusqlite`
- `domain.rs` — Core domain types (`ProviderConfig`, `ProviderProtocol`, `Agent`, `Project`)
- `sessions.rs` — Session management and conversation handling

## Development

```bash
# Install dependencies
npm install

# Run frontend dev server
npm run dev

# Run Tauri dev (from project root)
npm run tauri:dev
```