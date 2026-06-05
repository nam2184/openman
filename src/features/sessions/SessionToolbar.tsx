import { Plus, RefreshCcw } from "lucide-react";
import { Button } from "../../components/ui/button";

interface SessionToolbarProps {
  isCreating: boolean;
  error: string | null;
  projectName: string | null;
  onCreateSession: () => void;
  onRefresh: () => void;
}

export function SessionToolbar({
  isCreating,
  error,
  projectName,
  onCreateSession,
  onRefresh,
}: SessionToolbarProps) {
  return (
    <header className="flex h-[61px] items-center justify-between border-b border-[#313244] bg-[#181825] px-4">
      <div>
        <h1 className="text-sm font-semibold text-[#cdd6f4]">
          {projectName ? `${projectName} Sessions` : "No Project Selected"}
        </h1>
        <p className="text-xs text-[#6c7086]">
          {projectName
            ? "Add sessions by choosing directories, then group nodes by connecting them."
            : "Create or select a project before adding sessions."}
        </p>
      </div>
      <div className="flex items-center gap-2">
        {error && <span className="max-w-[360px] truncate text-xs text-[#f38ba8]">{error}</span>}
        <Button variant="secondary" size="sm" onClick={onRefresh}>
          <RefreshCcw className="h-3.5 w-3.5" />
          Refresh
        </Button>
        <Button size="sm" onClick={onCreateSession} disabled={isCreating || !projectName}>
          <Plus className="h-3.5 w-3.5" />
          {isCreating ? "Adding" : "Add Session Directory"}
        </Button>
      </div>
    </header>
  );
}
