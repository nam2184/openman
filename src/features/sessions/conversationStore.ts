import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface ConversationMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: string;
}

export interface ConversationFile {
  session_id: string;
  messages: ConversationMessage[];
  summary: string | null;
}

type AgentLlmEvent = {
  type: string;
  id?: string;
  text?: string;
  name?: string;
  message?: string;
  input?: unknown;
  result?: unknown;
  output?: string | null;
  provider_executed?: boolean | null;
};

export type AgentStreamEvent =
  | { type: "started"; session_id: string }
  | { type: "llm_event"; session_id: string; step: number; event: AgentLlmEvent }
  | { type: "finished"; session_id: string; response: string }
  | { type: "error"; session_id: string; message: string };

interface ConversationState {
  activeConversation: ConversationFile | null;
  streamingMessageId: string | null;
  loadConversation: (sessionId: string) => Promise<void>;
  loadUiConversation: (sessionId: string) => Promise<void>;
  appendMessage: (sessionId: string, role: "user" | "assistant" | "system", content: string) => Promise<string>;
  beginStreamingMessage: (sessionId: string, content: string) => void;
  applyAgentEvent: (event: AgentStreamEvent) => void;
  failStreamingMessage: (sessionId: string, message: string) => void;
  finishStreamingMessage: (sessionId: string) => void;
  compactConversation: (sessionId: string, summary: string) => Promise<void>;
  clearConversation: () => void;
}

function createTempMessage(role: ConversationMessage["role"], content: string): ConversationMessage {
  return {
    id: `temp-${role}-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    role,
    content,
    timestamp: new Date().toISOString(),
  };
}

function ensureConversation(sessionId: string, current: ConversationFile | null): ConversationFile {
  if (current?.session_id === sessionId) {
    return current;
  }

  return {
    session_id: sessionId,
    messages: [],
    summary: null,
  };
}

function upsertAssistantDraft(
  conversation: ConversationFile,
  streamingMessageId: string | null,
  update: (content: string) => string,
) {
  const messageId = streamingMessageId ?? createTempMessage("assistant", "").id;
  const existingIndex = conversation.messages.findIndex((message) => message.id === messageId);

  if (existingIndex === -1) {
    return {
      conversation: {
        ...conversation,
        messages: [...conversation.messages, { ...createTempMessage("assistant", update("")), id: messageId }],
      },
      streamingMessageId: messageId,
    };
  }

  const messages = [...conversation.messages];
  const existing = messages[existingIndex];
  messages[existingIndex] = {
    ...existing,
    content: update(existing.content),
  };

  return {
    conversation: {
      ...conversation,
      messages,
    },
    streamingMessageId: messageId,
  };
}

export const useConversationStore = create<ConversationState>((set) => ({
  activeConversation: null,
  streamingMessageId: null,

  loadConversation: async (sessionId) => {
    const conv = await invoke<ConversationFile>("get_ai_conversation", { sessionId });
    set({ activeConversation: conv, streamingMessageId: null });
  },

  loadUiConversation: async (sessionId) => {
    const conv = await invoke<ConversationFile>("get_ui_conversation", { sessionId });
    set({ activeConversation: conv, streamingMessageId: null });
  },

  appendMessage: async (sessionId, role, content) => {
    const messageId = await invoke<string>("append_message", {
      sessionId,
      role,
      content,
    });
    const conv = await invoke<ConversationFile>("get_ui_conversation", { sessionId });
    set({ activeConversation: conv, streamingMessageId: null });
    return messageId;
  },

  beginStreamingMessage: (sessionId, content) => {
    set((state) => {
      const conversation = ensureConversation(sessionId, state.activeConversation);
      const assistant = createTempMessage("assistant", "");

      return {
        activeConversation: {
          ...conversation,
          messages: [
            ...conversation.messages,
            createTempMessage("user", content),
            assistant,
          ],
        },
        streamingMessageId: assistant.id,
      };
    });
  },

  applyAgentEvent: (event) => {
    set((state) => {
      if (!state.activeConversation || state.activeConversation.session_id !== event.session_id) {
        return state;
      }

      if (event.type === "error") {
        const { conversation, streamingMessageId } = upsertAssistantDraft(
          state.activeConversation,
          state.streamingMessageId,
          (current) => appendErrorToDraft(current, event.message),
        );
        return { activeConversation: conversation, streamingMessageId };
      }

      if (event.type !== "llm_event") {
        return state;
      }

      if (event.event.type === "text_delta") {
        const { conversation, streamingMessageId } = upsertAssistantDraft(
          state.activeConversation,
          state.streamingMessageId,
          (current) => current + (event.event.text ?? ""),
        );
        return { activeConversation: conversation, streamingMessageId };
      }

      if (event.event.type === "tool_call") {
        const { conversation, streamingMessageId } = upsertAssistantDraft(
          state.activeConversation,
          state.streamingMessageId,
          (current) => current || `Running ${event.event.name ?? "tool"}...`,
        );
        return { activeConversation: conversation, streamingMessageId };
      }

      if (event.event.type === "tool_error") {
        const { conversation, streamingMessageId } = upsertAssistantDraft(
          state.activeConversation,
          state.streamingMessageId,
          (current) => `${current}\n${event.event.name ?? "tool"} failed: ${event.event.message ?? "Unknown error"}`.trim(),
        );
        return { activeConversation: conversation, streamingMessageId };
      }

      if (event.event.type === "provider_error") {
        const { conversation, streamingMessageId } = upsertAssistantDraft(
          state.activeConversation,
          state.streamingMessageId,
          (current) => appendErrorToDraft(current, event.event.message ?? "Unknown LLM provider error"),
        );
        return { activeConversation: conversation, streamingMessageId };
      }

      return state;
    });
  },

  failStreamingMessage: (sessionId, message) => {
    set((state) => {
      const conversation = ensureConversation(sessionId, state.activeConversation);
      const { conversation: updatedConversation, streamingMessageId } = upsertAssistantDraft(
        conversation,
        state.streamingMessageId,
        (current) => appendErrorToDraft(current, message),
      );
      return { activeConversation: updatedConversation, streamingMessageId };
    });
  },

  finishStreamingMessage: (sessionId) => {
    set((state) => {
      if (state.activeConversation?.session_id !== sessionId) {
        return state;
      }

      return { streamingMessageId: null };
    });
  },

  compactConversation: async (sessionId, summary) => {
    await invoke("compact_conversation", { sessionId, summary });
    const conv = await invoke<ConversationFile>("get_ai_conversation", { sessionId });
    set({ activeConversation: conv, streamingMessageId: null });
  },

  clearConversation: () => set({ activeConversation: null, streamingMessageId: null }),
}));

function appendErrorToDraft(current: string, message: string) {
  const errorText = `Error: ${message}`;
  if (!current.trim()) {
    return errorText;
  }
  if (current.includes(errorText) || current.includes(message)) {
    return current;
  }
  return `${current.trim()}\n\n${errorText}`;
}
