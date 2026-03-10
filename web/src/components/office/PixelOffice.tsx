import { useEffect, useRef } from "react";
import Phaser from "phaser";
import { OfficeScene } from "./OfficeScene";
import { pickLayout } from "./office-config";

interface PixelOfficeProps {
  members: string[];
  seating: Record<string, number>;
  onAgentClick: (agentId: string) => void;
}

export default function PixelOffice({
  members,
  seating,
  onAgentClick,
}: PixelOfficeProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const gameRef = useRef<Phaser.Game | null>(null);
  const sceneRef = useRef<OfficeScene | null>(null);

  // Create / destroy Phaser game.
  useEffect(() => {
    if (!containerRef.current) return;

    const layout = pickLayout(members.length);

    const game = new Phaser.Game({
      type: Phaser.AUTO,
      parent: containerRef.current,
      width: layout.width,
      height: layout.height,
      pixelArt: true,
      backgroundColor: "#2a2a3e",
      scale: {
        mode: Phaser.Scale.FIT,
        autoCenter: Phaser.Scale.CENTER_BOTH,
      },
      scene: [OfficeScene],
    });

    gameRef.current = game;

    // Wait for scene to be ready, then start with data.
    game.events.once("ready", () => {
      const scene = game.scene.getScene("OfficeScene") as OfficeScene;
      sceneRef.current = scene;
      scene.scene.restart({ members, seating, onAgentClick });
    });

    return () => {
      sceneRef.current = null;
      game.destroy(true);
      gameRef.current = null;
    };
    // Only recreate game if members count changes drastically (layout change).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [members.length]);

  // Update agents when members/seating change (without recreating game).
  useEffect(() => {
    const scene = sceneRef.current;
    if (scene && scene.scene.isActive()) {
      scene.updateAgents(members, seating);
    }
  }, [members, seating]);

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        maxWidth: 640,
        margin: "0 auto",
        borderRadius: 12,
        overflow: "hidden",
        border: "1px solid var(--border)",
      }}
    />
  );
}
