import { Plus, Sparkles } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Badge } from "../../../components/ui/badge";
import { Button } from "../../../components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Input } from "../../../components/ui/input";
import { ScrollArea } from "../../../components/ui/scroll-area";
import { Separator } from "../../../components/ui/separator";
import { cn } from "../../../lib/utils";
import { useProjectStore, type Project } from "../../../features/project/projectStore";

interface ProjectSidebarProps {
  project: Project | null;
}

export function ProjectSidebar({ project }: ProjectSidebarProps) {
  const { createProject, initializeProjects, projects, setCurrentProject } = useProjectStore();
  const [projectName, setProjectName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  useEffect(() => {
    initializeProjects().catch((initError) => {
      setError(formatError(initError));
      console.error("Failed to load projects:", initError);
    });
  }, [initializeProjects]);

  const sortedProjects = useMemo(
    () => [...projects].sort((a, b) => a.createdAt.localeCompare(b.createdAt)),
    [projects],
  );

  const submitProject = async () => {
    setError(null);
    setIsCreating(true);

    try {
      await createProject(projectName);
      setProjectName("");
    } catch (createError) {
      setError(formatError(createError));
      console.error("Failed to create project:", createError);
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <aside className="flex w-[320px] shrink-0 flex-col border-r border-[#313244] bg-[#181825]">
      <div className="space-y-4 p-4">
        <div className="flex items-center gap-2">
          <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#313244] text-[#89b4fa]">
            <Sparkles className="h-4 w-4" />
          </div>
          <div>
            <h1 className="text-base font-semibold text-[#cdd6f4]">OpenMan</h1>
            <p className="text-xs text-[#6c7086]">Project session canvases</p>
          </div>
        </div>

        <Card className="bg-[#1e1e2e]/40">
          <CardHeader>
            <CardTitle>New Project</CardTitle>
            <CardDescription>Projects are containers for session canvases.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-2">
            <Input
              value={projectName}
              placeholder="Project name"
              onChange={(event) => setProjectName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  submitProject();
                }
              }}
            />
            <Button className="w-full" onClick={submitProject} disabled={isCreating || !projectName.trim()}>
              <Plus className="h-4 w-4" />
              {isCreating ? "Creating" : "Create Project"}
            </Button>
            {error && <p className="text-xs text-[#f38ba8]">{error}</p>}
          </CardContent>
        </Card>
      </div>

      <Separator />

      <div className="min-h-0 flex-1 p-4">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-xs font-semibold uppercase tracking-wide text-[#6c7086]">Projects</h2>
          <Badge variant="secondary">{projects.length}</Badge>
        </div>
        <ScrollArea className="h-full">
          <div className="space-y-2 pr-2">
            {sortedProjects.length === 0 ? (
              <Card className="border-dashed bg-[#1e1e2e]/40">
                <CardHeader>
                  <CardTitle>No projects yet</CardTitle>
                  <CardDescription>Create a project before adding sessions.</CardDescription>
                </CardHeader>
              </Card>
            ) : (
              sortedProjects.map((item) => (
                <button
                  key={item.id}
                  className={cn(
                    "w-full rounded-xl border border-[#313244] bg-[#1e1e2e]/40 p-3 text-left transition-colors hover:border-[#45475a]",
                    project?.id === item.id && "border-[#89b4fa] bg-[#313244]",
                  )}
                  onClick={() => setCurrentProject(item)}
                >
                  <span className="truncate text-sm font-medium text-[#cdd6f4]">{item.name}</span>
                </button>
              ))
            )}
          </div>
        </ScrollArea>
      </div>
    </aside>
  );
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
