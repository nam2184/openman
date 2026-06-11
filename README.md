# Arachne

> _A network-weaving AI coding agent — many threads, one web._

Arachne is an open-source AI coding agent that lets you spin up multiple LLM
sessions, lay them out on a visual canvas as a graph, and have them share
context with each other in real time. Sessions can `task` out subtasks to
children, `ask_peer` a sibling session for a second opinion, and `interject`
snippets of local context (file contents, search hits, shell output) directly
into the active conversation. Built with Rust and Tauri.

An arachnid doesn't think in a straight line — it spins a web. Each strand is
its own thread of reasoning, anchored at a point but connected to every other
strand. Arachne applies the same idea to coding sessions:

- **A session is a strand** — an isolated conversation with a chosen
  provider/model that produces its own output.
- **The canvas is the web** — sessions are nodes you can drag, connect, and
  group. `parent_session_id` on every strand records the genealogy.
- **The runner is the spinner** — the central loop that pulls LLM events,
  weaves them into the conversation file, and dispatches tool calls.
- **Sub-agents are forked threads** — a `task` spawns a child session,
  `ask_peer` queries a sibling; both return into the parent's web.


## Features

- **Canvas-based session management** — Visual graph (React Flow) of AI
  coding sessions with drag-and-drop nodes and edges
- **Peer-to-peer session context** — A `parent_session_id` link records the
  genealogy; child sessions feed results back into the parent's
  conversation
- **Snippet interjection** — Pull a file range, a search hit, or shell
  output into the active turn without rewriting the prompt
- **Real-time streaming** — Live message updates during agent execution,
  with structured `LlmEvent` deltas (text, reasoning, tool calls, finish)
- **Plan / Build permission modes** — Read-only `plan` mode denies
  mutations; `build` mode allows them.
- **Multiple LLM providers** — Pluggable OpenAI-compatible and Anthropic
  transports
- **Project management** — Create and switch between projects, each with
  its own directory, tech-stack detection, and session set
- **Dark / Light theme** — Toggle in settings

## Providers

Arachne supports multiple LLM providers through a protocol-based
architecture. Each provider implements one of two API systems:

### OpenAI-Compatible Chat (`Protocol::OpenAI`)

Providers that expose an OpenAI Chat Completions-compatible endpoint at
`/v1/chat/completions`. These reuse a shared streaming transport and SSE
parsing implementation.

| Provider | Base URL | Notes |
|----------|----------|-------|
| **OpenAI** | `https://api.openai.com/v1` | GPT-4o, GPT-4o-mini, o3, o4-mini |
| **MiniMax Token Plan** | `https://api.minimax.io/v1` | MiniMax-M3, MiniMax-M2.7, MiniMax-M2.5 |

### Anthropic Messages (`Protocol::Anthropic`)

Providers that implement the Anthropic Messages API at `/v1/messages`,
with support for streaming, tool use, and extended thinking.

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
|----------|----------------------|
| OpenAI | `OPENAI_API_KEY` |
| MiniMax Token Plan | `MINIMAX_TOKEN_PLAN_KEY` or `MINIMAX_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |

## Architecture

**Frontend** — React + TypeScript + Vite + Zustand + React Flow

**Backend** — Rust + Tauri (`src-tauri/`) + `arachne-agents` crate
(`agents/`)

The Tauri shell owns windows, IPC, and the `arachne` Tauri command surface.
All LLM, tool, and session logic lives in the `arachne-agents` crate and is
exposed to the frontend as `#[tauri::command]` functions in
`src-tauri/src/commands/`.

## Persistence

| What | Where |
|------|-------|
| Session / project / message metadata | `arachne.sqlite` in the OS data dir |
| Per-session AI conversation (the LLM's view) | `<data>/conversations/<session_id>.json` |
| Per-session UI conversation (the user's view) | `<data>/conversations/<session_id>.ui.json` |
| User settings (theme, font size, node skin) | `~/.config/arachne/settings.json` |
| Permission ruleset | `~/.config/arachne/config.json` (or `<cwd>/arachne.json`) |

The data dir is resolved by `directories::ProjectDirs::from("ai",
"arachne", "arachne")`.

## Development

```bash
# Install dependencies
npm install

# Run frontend dev server
npm run dev

# Run Tauri dev (from project root)
npm run tauri:dev
```

Set `RUST_LOG=arachne=debug,tauri=info` to see the per-event LLM stream
log (every `LlmEvent` flowing through the runner) and the persistence
log emitted at the end of each turn.

## License

TBD.
