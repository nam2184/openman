import { useCallback, useEffect, useMemo } from "react";
import {
  Background,
  Controls,
  ReactFlow,
  useNodesState,
  type Connection,
  type Edge,
  type Node,
} from "reactflow";
import "reactflow/dist/style.css";
import { SessionNode } from "./SessionNode";
import { useSessionStore, type AgentSession } from "./sessionStore";

const nodeTypes = { sessionCard: SessionNode };

interface SessionCanvasProps {
  sessions: Map<string, AgentSession>;
}

export function SessionCanvas({ sessions }: SessionCanvasProps) {
  const { createGroup, addSessionToGroup, groups } = useSessionStore();
  const [nodes, setNodes, onNodesChange] = useNodesState([]);

  useEffect(() => {
    setNodes((currentNodes) => {
      const currentPositions = new Map(currentNodes.map((node) => [node.id, node.position]));

      return Array.from(sessions.keys()).map<Node>((id, index) => ({
        id,
        type: "sessionCard",
        position: currentPositions.get(id) ?? {
          x: 96 + (index % 4) * 240,
          y: 96 + Math.floor(index / 4) * 180,
        },
        data: { label: id },
      }));
    });
  }, [sessions, setNodes]);

  const edges = useMemo<Edge[]>(() => {
    return Array.from(groups.values()).flatMap((group) => {
      const sessionIds = group.session_ids.filter((id) => sessions.has(id));
      const groupEdges: Edge[] = [];

      for (let i = 0; i < sessionIds.length; i += 1) {
        for (let j = i + 1; j < sessionIds.length; j += 1) {
          groupEdges.push({
            id: `${group.id}:${sessionIds[i]}-${sessionIds[j]}`,
            source: sessionIds[i],
            target: sessionIds[j],
            animated: true,
            style: { stroke: "#89b4fa", strokeWidth: 2 },
          });
        }
      }

      return groupEdges;
    });
  }, [groups, sessions]);

  const onConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) return;

      const sourceGroupId = sessions.get(connection.source)?.group_id;
      const targetGroupId = sessions.get(connection.target)?.group_id;

      let action: Promise<unknown>;
      if (sourceGroupId && sourceGroupId === targetGroupId) {
        return;
      }

      if (sourceGroupId && !targetGroupId) {
        action = addSessionToGroup(connection.target, sourceGroupId);
      } else if (!sourceGroupId && targetGroupId) {
        action = addSessionToGroup(connection.source, targetGroupId);
      } else if (sourceGroupId && targetGroupId) {
        const sourceGroup = groups.get(sourceGroupId);
        const targetGroup = groups.get(targetGroupId);
        action = createGroup(
          Array.from(new Set([
            ...(sourceGroup?.session_ids ?? []),
            ...(targetGroup?.session_ids ?? []),
            connection.source,
            connection.target,
          ])),
        );
      } else {
        action = createGroup([connection.source, connection.target]);
      }

      action.catch((error) => {
        console.error("Failed to update session group:", error);
      });
    },
    [addSessionToGroup, createGroup, groups, sessions],
  );

  return (
    <div className="h-full bg-[#11111b]">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onConnect={onConnect}
        nodeTypes={nodeTypes}
        fitView
      >
        <Background color="#313244" gap={24} />
        <Controls />
      </ReactFlow>
    </div>
  );
}
