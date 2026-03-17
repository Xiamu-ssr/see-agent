/** Office layout configurations and colour palettes. */

import type { OfficeLayout } from "./types";

const SMALL_LAYOUT: OfficeLayout = {
  name: "small",
  width: 480,
  height: 320,
  seats: [
    { x: 80, y: 140, deskW: 80, deskH: 48 },
    { x: 200, y: 140, deskW: 80, deskH: 48 },
    { x: 320, y: 140, deskW: 80, deskH: 48 },
  ],
};

const MEDIUM_LAYOUT: OfficeLayout = {
  name: "medium",
  width: 480,
  height: 400,
  seats: [
    { x: 80, y: 120, deskW: 80, deskH: 48 },
    { x: 200, y: 120, deskW: 80, deskH: 48 },
    { x: 320, y: 120, deskW: 80, deskH: 48 },
    { x: 80, y: 260, deskW: 80, deskH: 48 },
    { x: 200, y: 260, deskW: 80, deskH: 48 },
    { x: 320, y: 260, deskW: 80, deskH: 48 },
  ],
};

const LARGE_LAYOUT: OfficeLayout = {
  name: "large",
  width: 640,
  height: 480,
  seats: [
    { x: 80, y: 100, deskW: 80, deskH: 48 },
    { x: 200, y: 100, deskW: 80, deskH: 48 },
    { x: 320, y: 100, deskW: 80, deskH: 48 },
    { x: 440, y: 100, deskW: 80, deskH: 48 },
    { x: 80, y: 230, deskW: 80, deskH: 48 },
    { x: 200, y: 230, deskW: 80, deskH: 48 },
    { x: 320, y: 230, deskW: 80, deskH: 48 },
    { x: 440, y: 230, deskW: 80, deskH: 48 },
    { x: 80, y: 360, deskW: 80, deskH: 48 },
    { x: 200, y: 360, deskW: 80, deskH: 48 },
    { x: 320, y: 360, deskW: 80, deskH: 48 },
    { x: 440, y: 360, deskW: 80, deskH: 48 },
  ],
};

export const OFFICE_LAYOUTS = [SMALL_LAYOUT, MEDIUM_LAYOUT, LARGE_LAYOUT];

/** Pick the smallest layout that fits *count* agents. */
export function pickLayout(count: number): OfficeLayout {
  for (const layout of OFFICE_LAYOUTS) {
    if (count <= layout.seats.length) return layout;
  }
  return LARGE_LAYOUT;
}

/** Six distinct tint colours for agent sprites (as 0xRRGGBB). */
export const AGENT_TINTS: number[] = [
  0x4a9eff, // blue
  0x4adf7a, // green
  0xff6b6b, // red
  0xffa94d, // orange
  0xcc5de8, // purple
  0x20c997, // teal
];
