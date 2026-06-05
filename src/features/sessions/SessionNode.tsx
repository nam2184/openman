import { Handle, Position, type NodeProps } from "reactflow";
import { cn } from "../../lib/utils";
import { useSessionStore } from "./sessionStore";

interface SessionNodeData {
  label?: string;
}

export function SessionNode({ id, selected }: NodeProps<SessionNodeData>) {
  const { sessions, setActiveSession } = useSessionStore();
  const session = sessions.get(id);

  if (!session) return null;

  const directoryName = session.directory.split(/[\\/]/).filter(Boolean).pop() ?? session.directory;

  const handleDoubleClick = () => {
    setActiveSession(id);
    window.dispatchEvent(new CustomEvent("session:dblclick", { detail: { id } }));
  };

  return (
    <div className="flex flex-col items-center gap-1">
      <div
        className={cn(
          "relative flex h-10 w-10 cursor-pointer items-center justify-center rounded-full",
          selected && "ring-2 ring-[#89b4fa] ring-offset-2 ring-offset-[#181825]",
        )}
        onClick={() => setActiveSession(id)}
        onDoubleClick={handleDoubleClick}
      >
        <Handle type="target" position={Position.Top} className="!border-[#11111b] !bg-[#89b4fa]" />
        <svg viewBox="0 0 24 24" className="h-5 w-5">
          <defs>
            <radialGradient id={`diffuse-${id}`} cx="50%" cy="50%" r="50%">
              <stop offset="0%" stopColor="#89b4fa" stopOpacity="0.8" />
              <stop offset="70%" stopColor="#89b4fa" stopOpacity="0.3" />
              <stop offset="100%" stopColor="#89b4fa" stopOpacity="0" />
            </radialGradient>
          </defs>
          <circle cx="12" cy="12" r="10" fill={`url(#diffuse-${id})`} />
          <circle cx="12" cy="12" r="3" fill="#89b4fa" />
        </svg>
        <Handle type="source" position={Position.Bottom} className="!border-[#11111b] !bg-[#89b4fa]" />
      </div>
      <span className="truncate text-[10px] text-[#6c7086]">{directoryName}</span>
    </div>
  );
}
