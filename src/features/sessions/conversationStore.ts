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

interface ConversationState {
  activeConversation: ConversationFile | null;
  loadConversation: (sessionId: string) => Promise<void>;
  loadUiConversation: (sessionId: string) => Promise<void>;
  appendMessage: (sessionId: string, role: "user" | "assistant" | "system", content: string) => Promise<string>;
  compactConversation: (sessionId: string, summary: string) => Promise<void>;
  clearConversation: () => void;
}

export const useConversationStore = create<ConversationState>((set) => ({
  activeConversation: null,

  loadConversation: async (sessionId) => {
    const conv = await invoke<ConversationFile>("get_ai_conversation", { sessionId });
    set({ activeConversation: conv });
  },

  loadUiConversation: async (sessionId) => {
    const conv = await invoke<ConversationFile>("get_ui_conversation", { sessionId });
    set({ activeConversation: conv });
  },

  appendMessage: async (sessionId, role, content) => {
    const messageId = await invoke<string>("append_message", {
      sessionId,
      role,
      content,
    });
    const conv = await invoke<ConversationFile>("get_ai_conversation", { sessionId });
    set({ activeConversation: conv });
    return messageId;
  },

  compactConversation: async (sessionId, summary) => {
    await invoke("compact_conversation", { sessionId, summary });
    const conv = await invoke<ConversationFile>("get_ai_conversation", { sessionId });
    set({ activeConversation: conv });
  },

  clearConversation: () => set({ activeConversation: null }),
}));