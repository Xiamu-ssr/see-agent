/** Pixel office type definitions. */

export interface SeatConfig {
  x: number;
  y: number;
  deskW: number;
  deskH: number;
}

export interface OfficeLayout {
  name: string;
  width: number;
  height: number;
  seats: SeatConfig[];
}
