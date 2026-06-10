# Streaming + thinking + .env todo

Goal: get the chat to stream incrementally, surface MiniMax-M3's
`reasoning_content` (thinking) in the UI, and let you test with a `.env`
file at the project root.

Context: streaming is already wired on the Rust side — `OpenAiCompatibleChatProvider::stream` reads SSE chunks off the wire and `SessionRunner` emits each `LlmEvent` to the Tauri event bus one by one. The two real gaps are (1) `parse_openai_chunk` in `agents/src/llm/providers/mod.rs:88` only reads `delta.content` and `delta.finish_reason`, so `delta.reasoning_content` from MiniMax is dropped before any event is created, and (2) `src/features/sessions/conversationStore.ts:148` `applyAgentEvent` only handles `text_delta` / `tool_call` / `tool_error` / `provider_error` and ignores everything else. The `.env` path needs a new dep on the Tauri side and a small override in `ProviderService`.

---

## 1. Add `.env` support

### 1a. Add `dotenvy` to `src-tauri/Cargo.toml`

```
dotenvy = "0.15"
```

### 1b. Load `.env` at startup — `src-tauri/src/main.rs`

In `run()` (line 53), before `setup_logging()`:

```rust
let _ = dotenvy::dotenv();
```

This loads `.env` from CWD and the parent chain. It does **not** override
existing env vars, so a real `MINIMAX_TOKEN_PLAN_KEY` in the shell still wins.

### 1c. Create `.env.example` at the project root

```
# MiniMax Token Plan API key (used when ProviderConfig.api_key is null)
MINIMAX_TOKEN_PLAN_KEY=
MINIMAX_API_KEY=

# OpenAI
OPENAI_API_KEY=

# Anthropic
ANTHROPIC_API_KEY=
```

The file is already allowed by the existing `!.env.example` line in `.gitignore`.

### 1d. Make env vars feed into `ProviderService` — `agents/src/provider_service.rs`

Two options. Pick **(B)** — it's strictly better.

**(A)** Env wins over DB (replace the DB key on load): easy to lose user-typed keys.

**(B)** DB key wins; env is a fallback when DB is `None`. This is what the
`MiniMaxTokenPlanProvider::new` fallback chain already does
(`agents/src/llm/providers/minimax_token_plan.rs:16-19`), so all you have to
do is nothing — just confirm the fallback chain reads `MINIMAX_TOKEN_PLAN_KEY`
and `MINIMAX_API_KEY` (it does) and you're done.

To verify locally:

```bash
echo 'MINIMAX_TOKEN_PLAN_KEY=eyJ...' > .env
RUST_LOG=openman_agents=debug npm run tauri:dev
```

On first request the new debug line in `openai_compatible_chat.rs` will print
`llm request header: ... Authorization="Bearer eyJ..."` — confirming the env
var is being read.

If you ever want to **force** the env var to win over the DB value, add this
in `provider_service.rs` after `let mut configs = ProviderConfigRepository::list(&db)?;`
(around line 60):

```rust
for cfg in configs.iter_mut() {
    if cfg.api_key.is_none() {
        if let Some(key) = std::env::var("MINIMAX_TOKEN_PLAN_KEY").ok()
            .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
        {
            cfg.api_key = Some(key);
        }
    }
}
```

…but the existing provider-side fallback is enough; skip this unless you
want DB keys to be overridable.

---

## 2. Surface MiniMax thinking in `parse_openai_chunk`

File: `agents/src/llm/providers/mod.rs:88-112`

Replace the function body to also read `delta.reasoning_content` and
`delta.reasoning_details[0].text`. MiniMax-M3 sends either, so check both.
Emit a `ReasoningDelta` (and lazily a `ReasoningStart`) so the UI can
distinguish thinking from final answer.

```rust
pub fn parse_openai_chunk(text: &str) -> Option<LlmEvent> {
    use crate::llm::events::LlmEvent;

    let json: Value = serde_json::from_str(text).ok()?;
    let choices = json.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let delta = choice.get("delta")?;

    // 1. MiniMax-M3 reasoning stream.
    let reasoning = delta
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            delta
                .get("reasoning_details")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|d| d.get("text"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });

    if let Some(reason) = reasoning {
        if !reason.is_empty() {
            return Some(LlmEvent::ReasoningDelta {
                id: "reasoning".to_string(),
                text: reason,
            });
        }
    }

    // 2. Regular content.
    if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            return Some(LlmEvent::TextDelta {
                id: "text".to_string(),
                text: text.to_string(),
            });
        }
    }

    // 3. Finish.
    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
        let usage = json.get("usage").and_then(|u| parse_openai_usage(u));
        return Some(LlmEvent::Finish {
            reason: FinishReason::from(reason),
            usage,
        });
    }

    None
}
```

The `LlmEvent::ReasoningDelta` variant already exists (`agents/src/llm/events.rs:144`).

> **Why not also fire a `ReasoningStart`?** A `ReasoningStart` followed by
> zero or more `ReasoningDelta` and a `ReasoningEnd` is the proper protocol.
> Right now the parser just emits `ReasoningDelta`. The UI handler we'll add
> in step 3b will work with that, but if you want strict start/delta/end
> semantics, wrap the first `ReasoningDelta` after an empty buffer with a
> `ReasoningStart` — easiest in the store, not the parser. The runner already
> accumulates reasoning into `assistant_parts` (`session.rs:146-150`); that's
> fine to leave alone.

---

## 3. Show the thinking in the UI

### 3a. Extend the event type — `src/features/sessions/conversationStore.ts:17-27`

The `AgentLlmEvent` interface already has `type: string`, so `reasoning_delta`
will deserialize. Nothing to change there, but also extend the `ConversationMessage`
type to keep a separate `reasoning` field:

```ts
export interface ConversationMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  reasoning?: string;        // ← add
  timestamp: string;
}
```

### 3b. Update `applyAgentEvent` — `conversationStore.ts:148-205`

The current `upsertAssistantDraft` only mutates `content`. Add a sibling
helper that mutates `reasoning`, then handle `reasoning_delta` in the
switch:

```ts
function upsertAssistantReasoning(
  conversation: ConversationFile,
  streamingMessageId: string | null,
  update: (reasoning: string) => string,
) {
  const messageId = streamingMessageId ?? createTempMessage("assistant", "").id;
  const existingIndex = conversation.messages.findIndex((m) => m.id === messageId);
  if (existingIndex === -1) {
    return {
      conversation: {
        ...conversation,
        messages: [...conversation.messages, { ...createTempMessage("assistant", ""), reasoning: update(""), id: messageId }],
      },
      streamingMessageId: messageId,
    };
  }
  const messages = [...conversation.messages];
  const existing = messages[existingIndex];
  messages[existingIndex] = { ...existing, reasoning: update(existing.reasoning ?? "") };
  return { conversation: { ...conversation, messages }, streamingMessageId: messageId };
}
```

And in `applyAgentEvent`, add this branch (anywhere among the `if (event.event.type === ...)` blocks):

```ts
if (event.event.type === "reasoning_delta") {
  const { conversation, streamingMessageId } = upsertAssistantReasoning(
    state.activeConversation,
    state.streamingMessageId,
    (current) => current + (event.event.text ?? ""),
  );
  return { activeConversation: conversation, streamingMessageId };
}
```

If you want a strict start/delta/end, also fire a `ReasoningStart` before the
first delta: track the last reasoning-emitted timestamp on the message
(currently `existing.reasoning` is `""`) and yield a `reasoning_start`
notification. For now, the simpler "just append" path is enough.

### 3c. Render the reasoning in `SessionChat` — `src/components/sessions/SessionChat.tsx:283-292`

In the assistant-bubble branch, show the `reasoning` (if any) above the
content in a TUI dim style:

```tsx
<div
  className={cn(
    "max-w-[80%] whitespace-pre-wrap break-words rounded-none border border-[#1f1f1f] bg-[#0a0a0a] px-4 py-2 text-sm",
  )}
>
  {message.reasoning && (
    <details className="mb-2 text-xs text-[#737373]" open>
      <summary className="cursor-pointer select-none text-[#737373]">
        ◌ thinking
      </summary>
      <pre className="mt-1 whitespace-pre-wrap text-[#737373]">
        {message.reasoning}
      </pre>
    </details>
  )}
  <div className="text-[#f5f5f5]">{content}</div>
</div>
```

The `message.reasoning` field is on the `SessionChatMessage` interface at
`SessionChat.tsx:11-16`. Extend it the same way:

```ts
export interface SessionChatMessage {
  id?: string;
  role: "user" | "assistant" | "system";
  content: string;
  reasoning?: string;        // ← add
  timestamp: string;
}
```

And in `formatMessageContent` (~line 330) the JSON `parts` parsing should
collect `reasoning` parts separately. For now the live stream is what
matters; the persisted conversation JSON doesn't need to round-trip
reasoning (the assistant `content` field is enough for the final answer).

### 3d. Hide the typing dots once reasoning has started

The current `◌ ◌ ◌ thinking` placeholder in `SessionChat.tsx:299-307` should
only show if `message.reasoning` is empty **and** content is empty. Otherwise
suppress it:

```tsx
{isSending && !message.content && !message.reasoning && (
  <div ...>...</div>
)}
```

Apply that guard inside the `messages.map(...)` so it keys per-message, or
move the typing indicator out of the map and gate on `messages.every(m => !m.content && !m.reasoning)`.

---

## 4. Persist the API key from the UI to the DB (only if you want it)

If you want the `.env` to be authoritative **and** survive DB reloads, do
nothing in the DB. The provider's env-var fallback chain already handles
`MINIMAX_TOKEN_PLAN_KEY` / `MINIMAX_API_KEY` / `OPENAI_API_KEY` /
`ANTHROPIC_API_KEY` (`openai_compatible_chat.rs:32` and
`minimax_token_plan.rs:16-19`).

If you also want the UI's Settings → Providers field to **show** the env key
when DB is null, add a small `if cfg.api_key.is_none() { cfg.api_key = std::env::var(...).ok() }`
loop in `ProviderService::load()` — see step 1d for the snippet. Skip
otherwise; the running provider will pick the key up via the fallback chain
regardless.

---

## 5. Test the streaming + thinking

### 5a. End-to-end via UI

1. `echo 'MINIMAX_TOKEN_PLAN_KEY=eyJhbGciOi...' > .env`
2. `RUST_LOG=openman_agents=debug npm run tauri:dev`
3. In the app, pick a session whose provider is `minimax` and model
   `MiniMax-M3`. Send a prompt that triggers thinking: "walk me through how
   this directory's code is organized".
4. Tail the terminal. You should see:
   - `llm request header: provider=minimax url=https://api.minimaxi.io/v1/chat/completions model=MiniMax-M3 Authorization="Bearer eyJh…"`
   - A stream of `LlmEvent` lines (if you bump to `RUST_LOG=openman_agents=trace`)
5. The chat should:
   - Show a `◌ thinking` block expand as MiniMax streams reasoning.
   - Append visible text in the assistant bubble character-by-character.
   - The `◌ ◌ ◌ thinking` placeholder at the bottom should disappear as soon
     as the first real event lands.

### 5b. Direct curl sanity check (no app involved)

```bash
curl -sS https://api.minimaxi.io/v1/chat/completions \
  -H "Authorization: Bearer $MINIMAX_TOKEN_PLAN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "MiniMax-M3",
    "messages": [{"role":"user","content":"hi"}],
    "stream": true,
    "stream_options": {"include_usage": true}
  }'
```

You should see SSE lines like `data: {"choices":[{"delta":{"reasoning_content":"..."}}]}`.
If you don't see `reasoning_content` on MiniMax, double-check the model
name — only the `MiniMax-M*` reasoning models emit it. `gpt-4o` and
`claude-3-5-sonnet` do not.

---

## 6. Files touched

- `src-tauri/Cargo.toml` — add `dotenvy = "0.15"`
- `src-tauri/src/main.rs` — call `dotenvy::dotenv()` at the top of `run()`
- `.env.example` — new file
- `agents/src/llm/providers/mod.rs` — extend `parse_openai_chunk` (step 2)
- `src/features/sessions/conversationStore.ts` — add `reasoning` field,
  `upsertAssistantReasoning`, and `reasoning_delta` branch
- `src/components/sessions/SessionChat.tsx` — extend `SessionChatMessage`,
  render reasoning block, gate the typing indicator

Nothing else in the data path needs to change: `SessionRunner` already calls
`self.emit_event(...)` for every event (`agents/src/llm/session.rs:134`) and
`AgentService::send_message` already forwards them through the Tauri event
bus (`src-tauri/src/services/agent_service.rs:124-133`).

---

## 7. Verification

- `npm run build` (frontend) — should still pass; no TypeScript types changed
  in a breaking way.
- `cargo check` (Rust) — needs `libssl-dev` and `libcairo2-dev` on Debian/Ubuntu
  to even compile. If those aren't installed: `sudo apt install libssl-dev
  libcairo2-dev pkg-config` (or equivalent), then `cargo check` from the
  project root.
- Manual: send a MiniMax-M3 message in the app, confirm reasoning streams in
  the `◌ thinking` block.

---

## 8. Optional follow-ups (not in scope, flagged for later)

- **Strict start/delta/end for reasoning.** Emit a `ReasoningStart` on the
  first non-empty delta, `ReasoningEnd` on a delta-to-content transition.
  Both events exist in `LlmEvent`; just unused.
- **Persist `reasoning` to disk.** The runner already accumulates reasoning
  into `assistant_parts` and writes them via `ContentPart::reasoning(...)`,
  but the UI's `get_ui_conversation` command strips them. To persist, modify
  the conversation service to include a `reasoning` field in the JSON
  content, then surface it on reload.
- **Stop the request mid-flight.** `LlmStream` carries an `abort_tx: Option<Arc<oneshot::Sender>>` — wire a Tauri command or a UI button to send on it.
