import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface SessionGroup {
  id: string;
  name?: string | null;
  session_ids: string[];
  created_at: string;
}

export interface AgentSession {
  id: string;
  project_id: string;
  directory: string;
  provider: string;
  model: string;
  group_id?: string;
  created_at: string;
}

interface SessionInitPayload {
  sessions: AgentSession[];
  groups: SessionGroup[];
}

interface SessionState {
  sessions: Map<string, AgentSession>;
  groups: Map<string, SessionGroup>;
  activeSessionId: string | null;
  initialize: () => Promise<void>;
  createSession: (projectId: string, directory: string) => Promise<string>;
  setActiveSession: (id: string) => void;
  deleteSession: (id: string) => Promise<void>;
  createGroup: (sessionIds: string[]) => Promise<string>;
  deleteGroup: (id: string) => Promise<void>;
  renameGroup: (id: string, name: string) => Promise<void>;
  addSessionToGroup: (sessionId: string, groupId: string) => Promise<void>;
  removeSessionFromGroup: (sessionId: string) => Promise<void>;
}

function mapById<T extends { id: string }>(items: T[]) {
  return new Map(items.map((item) => [item.id, item]));
}

export const useSessionStore = create<SessionState>((set, get) => {
  const refreshSessions = async () => {
    const sessions = await invoke<AgentSession[]>("get_all_sessions");
    set({ sessions: mapById(sessions) });
  };

  const refreshGroups = async () => {
    const groups = await invoke<SessionGroup[]>("get_all_session_groups");
    set({ groups: mapById(groups) });
  };

  const refreshAll = async () => {
    const { sessions, groups } = await invoke<SessionInitPayload>("init_sessions");
    set({ sessions: mapById(sessions), groups: mapById(groups) });
  };

  return {
    sessions: new Map(),
    groups: new Map(),
    activeSessionId: null,

    initialize: refreshAll,

    createSession: async (projectId, directory) => {
      const id = await invoke<string>("create_session", {
        projectId,
        directory,
        provider: "anthropic",
        model: "claude-3-5-sonnet-20241022",
      });

      await refreshSessions();
      set({ activeSessionId: id });
      return id;
    },

    setActiveSession: (id) => set({ activeSessionId: id }),

    deleteSession: async (id) => {
      await invoke("delete_session", { id });
      await refreshAll();
      if (get().activeSessionId === id) {
        set({ activeSessionId: null });
      }
    },

    createGroup: async (sessionIds) => {
      const id = await invoke<string>("create_session_group", { sessionIds });
      await refreshAll();
      return id;
    },

    deleteGroup: async (id) => {
      await invoke("delete_session_group", { id });
      await refreshAll();
    },

    renameGroup: async (id, name) => {
      const trimmedName = name.trim();
      await invoke("rename_session_group", {
        id,
        name: trimmedName ? trimmedName : null,
      });
      await refreshGroups();
    },

    addSessionToGroup: async (sessionId, groupId) => {
      await invoke("add_session_to_group", { sessionId, groupId });
      await refreshAll();
    },

    removeSessionFromGroup: async (sessionId) => {
      await invoke("remove_session_from_group", { sessionId });
      await refreshAll();
    },
  };
});
