/** Phaser container representing a single agent at a desk. */

import Phaser from "phaser";
import { AGENT_TINTS } from "./office-config";

export class AgentSprite extends Phaser.GameObjects.Container {
  private rect: Phaser.GameObjects.Rectangle;
  private label: Phaser.GameObjects.Text;
  private idleTween: Phaser.Tweens.Tween | null = null;
  public agentId: string;

  constructor(
    scene: Phaser.Scene,
    x: number,
    y: number,
    agentId: string,
    index: number,
    onAgentClick: (id: string) => void,
  ) {
    super(scene, x, y);
    this.agentId = agentId;

    const tint = AGENT_TINTS[index % AGENT_TINTS.length];

    // Coloured rectangle body (32 x 48).
    this.rect = scene.add.rectangle(0, 0, 32, 48, tint);
    this.add(this.rect);

    // Initial letter.
    const initial = agentId.charAt(0).toUpperCase();
    this.label = scene.add.text(0, 0, initial, {
      fontSize: "18px",
      fontFamily: "monospace",
      color: "#ffffff",
      fontStyle: "bold",
    });
    this.label.setOrigin(0.5, 0.5);
    this.add(this.label);

    // Interactive hit area.
    this.setSize(32, 48);
    this.setInteractive({ useHandCursor: true });
    this.on("pointerdown", () => onAgentClick(agentId));

    // Idle breathing tween.
    this.idleTween = scene.tweens.add({
      targets: this,
      y: y - 2,
      duration: 1200,
      yoyo: true,
      repeat: -1,
      ease: "Sine.easeInOut",
    });

    scene.add.existing(this as Phaser.GameObjects.GameObject);
  }

  destroy(fromScene?: boolean) {
    this.idleTween?.destroy();
    super.destroy(fromScene);
  }
}
