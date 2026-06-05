import { useProjectStore } from "../../features/project/projectStore";
import { SessionWorkspace } from "../../features/sessions/SessionWorkspace";
import { ProjectSidebar } from "./components/ProjectSidebar";

export function AppShell() {
  const currentProject = useProjectStore((state) => state.currentProject);

  return (
    <div className="flex h-screen overflow-hidden bg-[#1e1e2e] text-[#cdd6f4]">
      <ProjectSidebar project={currentProject} />
      <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
        <SessionWorkspace />
      </main>
    </div>
  );
}
