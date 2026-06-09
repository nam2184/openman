import { Handle, Position, type NodeProps } from "reactflow";
import { cn } from "../../lib/utils";
import type { AgentSession } from "../../features/sessions/sessionStore";

interface SessionNodeData {
  session: AgentSession;
  onSelect: (id: string) => void;
  onOpenChat: (id: string) => void;
}

export function SessionNode({ id, selected, data }: NodeProps<SessionNodeData>) {
  const { session, onSelect, onOpenChat } = data;

  const directoryName = session.directory.split(/[\\/]/).filter(Boolean).pop() ?? session.directory;

  const handleDoubleClick = () => {
    onOpenChat(id);
  };

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
