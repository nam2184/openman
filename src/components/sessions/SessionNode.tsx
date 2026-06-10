import { Handle, Position, type NodeProps } from "reactflow";
import { cn } from "@/lib/utils";
import type { AgentSession } from "@/features/sessions/sessionStore";
import type { NodeSkin } from "@/features/app/appStore";

interface SessionNodeData {
  session: AgentSession;
  skin: NodeSkin;
  onSelect: (id: string) => void;
  onOpenChat: (id: string) => void;
}

export function SessionNode({ id, selected, data }: NodeProps<SessionNodeData>) {
  const { session, skin, onSelect, onOpenChat } = data;

  const directoryName = session.directory.split(/[\\/]/).filter(Boolean).pop() ?? session.directory;

  const handleDoubleClick = () => {
    onOpenChat(id);
  };

  if (skin === "minimal") {
    return (
      <div className="flex flex-col items-center gap-1">
        <div
          className={cn(
            "relative flex h-3 w-3 cursor-pointer items-center justify-center rounded-full bg-[#f5f5f5]",
            selected && "ring-2 ring-white ring-offset-2 ring-offset-black",
          )}
          onClick={() => onSelect(id)}
          onDoubleClick={handleDoubleClick}
        >
          <Handle type="target" position={Position.Top} className="!h-1 !w-1 !border-0 !bg-[#737373]" />
          <Handle type="source" position={Position.Bottom} className="!h-1 !w-1 !border-0 !bg-[#737373]" />
        </div>
        <span className="truncate text-[10px] text-[#737373]">{directoryName}</span>
      </div>
    );
  }

  if (skin === "tui") {
    return (
      <div className="flex flex-col items-start gap-0">
        <div
          className={cn(
            "relative flex h-10 w-10 cursor-pointer items-center justify-center border border-[#f5f5f5] bg-black",
            selected && "bg-[#f5f5f5]",
          )}
          onClick={() => onSelect(id)}
          onDoubleClick={handleDoubleClick}
        >
          <Handle type="target" position={Position.Top} className="!h-1 !w-1 !border-0 !bg-[#f5f5f5]" />
          <span className={cn("text-xs", selected ? "text-black" : "text-[#f5f5f5]")}>
            ◉
          </span>
          <Handle type="source" position={Position.Bottom} className="!h-1 !w-1 !border-0 !bg-[#f5f5f5]" />
        </div>
        <span className="truncate border-l border-r border-b border-[#f5f5f5] px-1 text-[10px] text-[#f5f5f5]">
          {directoryName}
        </span>
      </div>
    );
  }

  // default: original diffused-orb design
  return (
    <div className="flex flex-col items-center gap-1">
      <div
        className={cn(
          "relative flex h-10 w-10 cursor-pointer items-center justify-center rounded-full",
          selected && "ring-2 ring-white ring-offset-2 ring-offset-black",
        )}
        onClick={() => onSelect(id)}
        onDoubleClick={handleDoubleClick}
      >
        <Handle type="target" position={Position.Top} className="!border-black !bg-white" />
        <svg viewBox="0 0 24 24" className="h-5 w-5">
          <defs>
            <radialGradient id={`diffuse-${id}`} cx="50%" cy="50%" r="50%">
              <stop offset="0%" stopColor="#ffffff" stopOpacity="0.9" />
              <stop offset="70%" stopColor="#ffffff" stopOpacity="0.22" />
              <stop offset="100%" stopColor="#ffffff" stopOpacity="0" />
            </radialGradient>
          </defs>
          <circle cx="12" cy="12" r="10" fill={`url(#diffuse-${id})`} />
          <circle cx="12" cy="12" r="3" fill="#ffffff" />
        </svg>
        <Handle type="source" position={Position.Bottom} className="!border-black !bg-white" />
      </div>
      <span className="truncate text-[10px] text-[#737373]">{directoryName}</span>
    </div>
  );
}
