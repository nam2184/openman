import { invoke } from "@tauri-apps/api/core";
import { Check, GripHorizontal, X } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { ScrollArea } from "../ui/scroll-area";
import { cn } from "../../lib/utils";
import { getDefaultModel, getModelOptions } from "../../features/sessions/providerModels";
import type { AgentSession, ProviderConfig } from "../../features/sessions/sessionStore";

export interface SessionChatMessage {
  id?: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: string;
}

interface SessionChatProps {
  session: AgentSession;
  messages: SessionChatMessage[];
  isSending: boolean;
  onSendMessage: (content: string) => void | Promise<void>;
  onUpdateSessionProvider: (sessionId: string, provider: string, model: string) => Promise<void>;
  onClose: () => void;
}

const CHAT_WIDTH = 700;
const CHAT_HEIGHT = 600;
const EDGE_PADDING = 16;

export function SessionChat({
  session,
  messages,
  isSending,
  onSendMessage,
  onUpdateSessionProvider,
  onClose,
}: SessionChatProps) {
  const [input, setInput] = useState("");
  const [position, setPosition] = useState(() => initialChatPosition());
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [providerDraft, setProviderDraft] = useState(session.provider);
  const [modelDraft, setModelDraft] = useState(session.model);
  const [configStatus, setConfigStatus] = useState<string | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);
  const [isConfigSaving, setIsConfigSaving] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const dragRef = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    invoke<ProviderConfig[]>("get_provider_configs")
      .then((configs) => setProviders(configs))
      .catch((error) => setConfigError(formatError(error)));
  }, []);

  useEffect(() => {
    setProviderDraft(session.provider);
    setModelDraft(session.model);
    setConfigStatus(null);
    setConfigError(null);
  }, [session.id, session.provider, session.model]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [isSending, messages]);

  const handleSend = async () => {
    const content = input.trim();
    if (!content || isSending) return;

    setInput("");
    await onSendMessage(content);
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleDragStart = (event: PointerEvent<HTMLDivElement>) => {
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: position.x,
      originY: position.y,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const handleDragMove = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    setPosition(clampChatPosition({
      x: drag.originX + event.clientX - drag.startX,
      y: drag.originY + event.clientY - drag.startY,
    }));
  };

  const handleDragEnd = (event: PointerEvent<HTMLDivElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) {
      dragRef.current = null;
    }
  };

  const saveSessionConfig = async () => {
    const provider = providerDraft.trim();
    const model = modelDraft.trim();
    if (!provider || !model) {
      setConfigError("Provider and model are required.");
      return;
    }

    setIsConfigSaving(true);
    setConfigError(null);
    setConfigStatus(null);

    try {
      await onUpdateSessionProvider(session.id, provider, model);
      setConfigStatus("Saved");
    } catch (error) {
      setConfigError(formatError(error));
    } finally {
      setIsConfigSaving(false);
    }
  };

  const configChanged = providerDraft !== session.provider || modelDraft !== session.model;
  const providerOptions = providers.some((provider) => provider.name === providerDraft)
    ? providers
    : providerDraft
      ? [
          {
            name: providerDraft,
            model: modelDraft,
            api_key: null,
            base_url: null,
            enabled: true,
          },
          ...providers,
        ]
      : providers;
  const modelOptions = getModelOptions(providerDraft, modelDraft);

  const directoryName = session.directory.split(/[\\/]/).filter(Boolean).pop() ?? session.directory;

  return (
    <div className="pointer-events-none fixed inset-0 z-50">
      <div
        className={cn(
          "pointer-events-auto fixed flex h-[600px] max-h-[calc(100vh-32px)] w-[700px] max-w-[calc(100vw-32px)] flex-col overflow-hidden rounded-2xl border border-[#2a2a2a] bg-[#050505]/95 text-white shadow-2xl shadow-black/70 backdrop-blur",
        )}
        role="dialog"
        aria-modal="false"
        aria-label={`${directoryName} chat`}
        style={{ left: position.x, top: position.y }}
      >
        <div
          className="flex cursor-grab items-center justify-between border-b border-[#1f1f1f] px-6 py-4 active:cursor-grabbing"
          onPointerDown={handleDragStart}
          onPointerMove={handleDragMove}
          onPointerUp={handleDragEnd}
          onPointerCancel={handleDragEnd}
        >
          <div className="flex min-w-0 items-center gap-3">
            <GripHorizontal className="h-4 w-4 shrink-0 text-[#737373]" />
            <div className="flex min-w-0 flex-col">
              <h2 className="truncate text-sm font-semibold text-white">{directoryName}</h2>
              <p className="truncate text-xs text-[#8a8a8a]">{session.directory}</p>
            </div>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onPointerDown={(event) => event.stopPropagation()}
            onClick={onClose}
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
        <div className="border-b border-[#1f1f1f] bg-[#0a0a0a] px-6 py-3">
          <div className="grid grid-cols-1 gap-2 md:grid-cols-[minmax(140px,0.8fr)_minmax(220px,1fr)_auto]">
            <label className="space-y-1">
              <span className="text-[10px] font-medium uppercase tracking-[0.18em] text-[#737373]">Provider</span>
              {providers.length > 0 ? (
                <select
                  value={providerDraft}
                  onChange={(event) => {
                    const nextProvider = event.target.value;
                    const provider = providerOptions.find((config) => config.name === event.target.value);
                    setProviderDraft(nextProvider);
                    setModelDraft(provider?.model ?? getDefaultModel(nextProvider, modelDraft));
                    setConfigStatus(null);
                    setConfigError(null);
                  }}
                  className="h-9 w-full rounded-md border border-[#2a2a2a] bg-black px-3 text-sm text-white outline-none transition-colors hover:border-[#4a4a4a] focus:border-white"
                >
                  {providerOptions.map((provider) => (
                    <option key={provider.name} value={provider.name}>{provider.name}</option>
                  ))}
                </select>
              ) : (
                <Input
                  value={providerDraft}
                  onChange={(event) => setProviderDraft(event.target.value)}
                  placeholder="anthropic"
                />
              )}
            </label>
            <label className="space-y-1">
              <span className="text-[10px] font-medium uppercase tracking-[0.18em] text-[#737373]">Model</span>
              <select
                value={modelDraft}
                onChange={(event) => {
                  setModelDraft(event.target.value);
                  setConfigStatus(null);
                  setConfigError(null);
                }}
                className="h-9 w-full rounded-md border border-[#2a2a2a] bg-black px-3 text-sm text-white outline-none transition-colors hover:border-[#4a4a4a] focus:border-white disabled:cursor-not-allowed disabled:opacity-50"
                disabled={modelOptions.length === 0}
              >
                {modelOptions.length === 0 ? (
                  <option value="">Add models in src/config/provider-models.json</option>
                ) : (
                  modelOptions.map((model) => (
                    <option key={model} value={model}>{model}</option>
                  ))
                )}
              </select>
            </label>
            <div className="flex items-end gap-2">
              <Button
                variant="secondary"
                className="h-9 border border-[#2a2a2a]"
                onClick={saveSessionConfig}
                disabled={isConfigSaving || !configChanged}
              >
                <Check className="h-4 w-4" />
                {isConfigSaving ? "Saving" : "Save"}
              </Button>
            </div>
          </div>
          {(configStatus || configError) && (
            <p className={cn("mt-2 text-xs", configError ? "text-[#ff5f5f]" : "text-[#bdbdbd]")}>{configError ?? configStatus}</p>
          )}
        </div>
        <ScrollArea className="flex-1 px-6 py-4" ref={scrollRef}>
          <div className="space-y-4">
            {messages.length === 0 ? (
              <div className="flex h-full items-center justify-center">
                <p className="text-sm text-[#737373]">
                  Chat with {directoryName} session. Ask about the codebase or files in this directory.
                </p>
              </div>
            ) : (
              messages.map((message, index) => {
                const content = formatMessageContent(message.content);

                return (
                  <div
                    key={message.id ?? `${message.timestamp}-${index}`}
                    className={cn(
                      "flex",
                      message.role === "user" ? "justify-end" : "justify-start",
                    )}
                  >
                    <div
                      className={cn(
                        "max-w-[80%] whitespace-pre-wrap break-words rounded-xl px-4 py-2 text-sm",
                        message.role === "user"
                          ? "bg-white text-black"
                          : "border border-[#2a2a2a] bg-[#111111] text-[#f5f5f5]",
                      )}
                    >
                      {content}
                    </div>
                  </div>
                );
              })
            )}
            {isSending && (
              <div className="flex justify-start">
                <div className="flex items-center gap-2 rounded-xl border border-[#2a2a2a] bg-[#111111] px-4 py-2">
                  <div className="h-2 w-2 animate-bounce rounded-full bg-white" />
                  <div className="h-2 w-2 animate-bounce rounded-full bg-white" />
                  <div className="h-2 w-2 animate-bounce rounded-full bg-white" />
                </div>
              </div>
            )}
          </div>
        </ScrollArea>

        <div className="border-t border-[#1f1f1f] px-6 py-4">
          <div className="flex gap-2">
            <Input
              ref={inputRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Ask about the codebase..."
              className="flex-1"
              disabled={isSending}
            />
            <Button onClick={handleSend} disabled={isSending || !input.trim()}>
              Send
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function formatMessageContent(content: string) {
  try {
    const parts = JSON.parse(content) as Array<{ type?: string; text?: string; name?: string }>;
    if (!Array.isArray(parts)) return content;

    const text = parts
      .map((part) => {
        if (part.type === "text" || part.type === "reasoning") {
          return part.text ?? "";
        }

        if (part.type === "tool_call") {
          return part.name ? `Running ${part.name}...` : "Running tool...";
        }

        return "";
      })
      .filter(Boolean)
      .join("\n");

    return text || content;
  } catch {
    return content;
  }
}

function initialChatPosition() {
  if (typeof window === "undefined") {
    return { x: 360, y: 80 };
  }

  return clampChatPosition({
    x: window.innerWidth - CHAT_WIDTH - 24,
    y: 80,
  });
}

function clampChatPosition(position: { x: number; y: number }) {
  if (typeof window === "undefined") {
    return position;
  }

  const width = Math.min(CHAT_WIDTH, window.innerWidth - EDGE_PADDING * 2);
  const height = Math.min(CHAT_HEIGHT, window.innerHeight - EDGE_PADDING * 2);
  const maxX = Math.max(EDGE_PADDING, window.innerWidth - width - EDGE_PADDING);
  const maxY = Math.max(EDGE_PADDING, window.innerHeight - height - EDGE_PADDING);

  return {
    x: Math.min(Math.max(EDGE_PADDING, position.x), maxX),
    y: Math.min(Math.max(EDGE_PADDING, position.y), maxY),
  };
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
