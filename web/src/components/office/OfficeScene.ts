/** Phaser Scene for the pixel office. */

import Phaser from "phaser";
import { AgentSprite } from "./AgentSprite";
import { pickLayout } from "./office-config";
import type { OfficeLayout } from "./types";

export interface OfficeSceneData {
  members: string[];
  seating: Record<string, number>;
  onAgentClick: (id: string) => void;
}

export class OfficeScene extends Phaser.Scene {
  private layout!: OfficeLayout;
  private sprites: AgentSprite[] = [];
  private onAgentClick!: (id: string) => void;
  private isDragging = false;
  private dragStart = { x: 0, y: 0 };

  constructor() {
    super({ key: "OfficeScene" });
  }

  init(data: OfficeSceneData) {
    this.onAgentClick = data.onAgentClick;
  }

  create(data: OfficeSceneData) {
    const { members, seating } = data;
    this.layout = pickLayout(members.length);

    // Draw procedural background.
    this.drawBackground();

    // Draw desks.
    this.drawDesks();

    // Place agents.
    this.placeAgents(members, seating);

    // Camera bounds.
    this.cameras.main.setBounds(0, 0, this.layout.width, this.layout.height);

    // Mouse drag panning.
    this.input.on("pointerdown", (pointer: Phaser.Input.Pointer) => {
      this.isDragging = true;
      this.dragStart.x = pointer.x;
      this.dragStart.y = pointer.y;
    });

    this.input.on("pointermove", (pointer: Phaser.Input.Pointer) => {
      if (!this.isDragging || !pointer.isDown) return;
      const dx = pointer.x - this.dragStart.x;
      const dy = pointer.y - this.dragStart.y;
      // Only pan if moved significantly (avoid conflict with clicks).
      if (Math.abs(dx) > 4 || Math.abs(dy) > 4) {
        this.cameras.main.scrollX -= dx;
        this.cameras.main.scrollY -= dy;
      }
      this.dragStart.x = pointer.x;
      this.dragStart.y = pointer.y;
    });

    this.input.on("pointerup", () => {
      this.isDragging = false;
    });
  }

  private drawBackground() {
    const g = this.add.graphics();
    const { width, height } = this.layout;

    // Floor fill.
    g.fillStyle(0x2a2a3e, 1);
    g.fillRect(0, 0, width, height);

    // Grid lines.
    g.lineStyle(1, 0x3a3a50, 0.3);
    const gridSize = 24;
    for (let x = 0; x <= width; x += gridSize) {
      g.lineBetween(x, 0, x, height);
    }
    for (let y = 0; y <= height; y += gridSize) {
      g.lineBetween(0, y, width, y);
    }
  }

  private drawDesks() {
    const g = this.add.graphics();
    for (const seat of this.layout.seats) {
      // Desk surface.
      g.fillStyle(0x4a4a60, 1);
      g.fillRect(
        seat.x - seat.deskW / 2,
        seat.y - seat.deskH / 2,
        seat.deskW,
        seat.deskH,
      );
      // Desk border.
      g.lineStyle(1, 0x6a6a80, 0.8);
      g.strokeRect(
        seat.x - seat.deskW / 2,
        seat.y - seat.deskH / 2,
        seat.deskW,
        seat.deskH,
      );
    }
  }

  private placeAgents(
    members: string[],
    seating: Record<string, number>,
  ) {
    // Clear existing sprites.
    for (const s of this.sprites) s.destroy();
    this.sprites = [];

    // Assign seats.
    const seats = this.layout.seats;
    const assigned: (string | null)[] = Array.from(
      { length: seats.length },
      () => null,
    );

    for (const id of members) {
      const idx = seating[id];
      if (idx != null && idx >= 0 && idx < seats.length) {
        assigned[idx] = id;
      }
    }

    // Place unassigned into first available.
    const unassigned = members.filter(
      (id) =>
        seating[id] == null ||
        seating[id] < 0 ||
        seating[id] >= seats.length ||
        assigned[seating[id]] !== id,
    );
    for (const id of unassigned) {
      const emptyIdx = assigned.indexOf(null);
      if (emptyIdx !== -1) assigned[emptyIdx] = id;
    }

    // Create sprites.
    for (let i = 0; i < assigned.length; i++) {
      const agentId = assigned[i];
      if (agentId == null) continue;
      const seat = seats[i];
      // Agent sprite sits above the desk.
      const sprite = new AgentSprite(
        this,
        seat.x,
        seat.y - seat.deskH / 2 - 28,
        agentId,
        i,
        this.onAgentClick,
      );
      this.sprites.push(sprite);
    }
  }

  /** Called from React to update agent positions. */
  updateAgents(members: string[], seating: Record<string, number>) {
    this.layout = pickLayout(members.length);
    this.placeAgents(members, seating);
  }
}
