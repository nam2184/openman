import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { cn } from "../../lib/utils";
import type { AgentSession } from "./sessionStore";
import { ScrollArea } from "../../components/ui/scroll-area";

interface SessionChatProps {
  session: AgentSession;
  onClose: () => void;
}
interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: Date;
}

export function SessionChat({ session, onClose }: SessionChatProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isSending, setIsSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSend = async () => {
    if (!input.trim() || isSending) return;

    const userMessage: ChatMessage = {
      role: "user",
      content: input.trim(),
      timestamp: new Date(),
    };

    setMessages((prev) => [...prev, userMessage]);
    setInput("");
    setIsSending(true);

    setTimeout(() => {
      const assistantMessage: ChatMessage = {
        role: "assistant",
        content: `Echo: ${userMessage.content}`,
        timestamp: new Date(),
      };
      setMessages((prev) => [...prev, assistantMessage]);
      setIsSending(false);
    }, 500);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const directoryName = session.directory.split(/[\\/]/).filter(Boolean).pop() ?? session.directory;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        className={cn(
          "flex h-[600px] w-[700px] flex-col  border border-[#313244] bg-[#1e1e2e]",
        )}
      >
        <div className="flex items-center justify-between border-b border-[#313244] px-6 py-4">
          <div className="flex flex-col">
            <h2 className="text-sm font-semibold text-[#cdd6f4]">{directoryName}</h2>
            <p className="text-xs text-[#6c7086]">{session.directory}</p>
          </div>
          <Button variant="ghost" size="icon" onClick={onClose}>
            <X className="h-4 w-4" />
          </Button>
        </div>
        <ScrollArea className="flex-1 px-6 py-4" ref={scrollRef}>
          <div className="space-y-4">
            {messages.length === 0 ? (
              <div className="flex h-full items-center justify-center">
                <p className="text-sm text-[#6c7086]">
                  Chat with {directoryName} session. Ask about the codebase or files in this directory.
                </p>
              </div>
            ) : (
              messages.map((message, index) => (
                <div
                  key={index}
                  className={cn(
                    "flex",
                    message.role === "user" ? "justify-end" : "justify-start",
                  )}
                >
                  <div
                    className={cn(
                      "max-w-[80%] rounded-xl px-4 py-2 text-sm",
                      message.role === "user"
                        ? "bg-[#89b4fa] text-[#11111b]"
                        : "bg-[#313244] text-[#cdd6f4]",
                    )}
                  >
                    {message.content}
                  </div>
                </div>
              ))
            )}
            {isSending && (
              <div className="flex justify-start">
                <div className="flex items-center gap-2 rounded-xl bg-[#313244] px-4 py-2">
                  <div className="h-2 w-2 animate-bounce rounded-full bg-[#89b4fa]" />
                  <div className="h-2 w-2 animate-bounce rounded-full bg-[#89b4fa]" />
                  <div className="h-2 w-2 animate-bounce rounded-full bg-[#89b4fa]" />
                </div>
              </div>
            )}
          </div>
        </ScrollArea>

        <div className="border-t border-[#313244] px-6 py-4">
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