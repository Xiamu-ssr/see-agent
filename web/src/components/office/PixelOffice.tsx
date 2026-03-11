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

    // OfficeScene.create() receives { members, seating, onAgentClick } via
    // scene data automatically (passed in the Scene constructor config).
    // We wait for the scene to finish its create() then grab the reference.
    game.scene.start("OfficeScene", { members, seating, onAgentClick });
    const checkScene = () => {
      const scene = game.scene.getScene("OfficeScene") as OfficeScene;
      if (scene && scene.scene.isActive()) {
        sceneRef.current = scene;
      } else {
        requestAnimationFrame(checkScene);
      }
    };
    requestAnimationFrame(checkScene);

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
