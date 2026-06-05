import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import { useProjectStore } from "../project/projectStore";
import { SessionCanvas } from "./SessionCanvas";
import { SessionChat } from "./SessionChat";
import { SessionToolbar } from "./SessionToolbar";
import { useSessionStore } from "./sessionStore";

export function SessionWorkspace() {
  const currentProject = useProjectStore((state) => state.currentProject);
  const { createSession, initialize, sessions } = useSessionStore();
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [chatSessionId, setChatSessionId] = useState<string | null>(null);

  useEffect(() => {
    initialize().catch((initError) => {
      setError(formatError(initError));
      console.error("Failed to initialize sessions:", initError);
    });
  }, [initialize]);

  useEffect(() => {
    const handleDblClick = (e: Event) => {
      const customEvent = e as CustomEvent<{ id: string }>;
      setChatSessionId(customEvent.detail.id);
    };
    window.addEventListener("session:dblclick", handleDblClick);
    return () => window.removeEventListener("session:dblclick", handleDblClick);
  }, []);

  const projectSessions = useMemo(() => {
    if (!currentProject) return new Map();

    return new Map(
      Array.from(sessions.entries()).filter(([, session]) => session.project_id === currentProject.id),
    );
  }, [currentProject, sessions]);

  const createSessionNode = async () => {
    if (!currentProject) {
      setError("Create or select a project before adding sessions.");
      return;
    }

    const directory = await open({ directory: true });
    if (!directory) return;

    setIsCreating(true);
    setError(null);

    try {
      await createSession(currentProject.id, directory);
    } catch (createError) {
      setError(formatError(createError));
      console.error("Failed to create session:", createError);
    } finally {
      setIsCreating(false);
    }
  };

  const chatSession = chatSessionId ? sessions.get(chatSessionId) : null;

  return (
    <section className="flex h-screen min-w-0 flex-1 flex-col bg-[#1e1e2e]">
      <SessionToolbar
        isCreating={isCreating}
        error={error}
        projectName={currentProject?.name ?? null}
        onCreateSession={createSessionNode}
        onRefresh={initialize}
      />
      <div className="flex min-h-0 flex-1">
        <div className="min-w-0 flex-1 overflow-hidden">
          <SessionCanvas sessions={projectSessions} />
        </div>
      </div>
      {chatSession && (
        <SessionChat
          session={chatSession}
          onClose={() => setChatSessionId(null)}
        />
      )}
    </section>
  );
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}