import React from "react";

interface PixelOfficeProps {
  members: string[];
  seating: Record<string, number>;
  onAgentClick: (agentId: string) => void;
}

function getLayout(count: number): { cols: number; rows: number; total: number } {
  if (count <= 3) return { cols: 3, rows: 1, total: 3 };
  if (count <= 6) return { cols: 3, rows: 2, total: 6 };
  return { cols: 4, rows: 3, total: 12 };
}

export default function PixelOffice({ members, seating, onAgentClick }: PixelOfficeProps) {
  const layout = getLayout(members.length);

  // Build seat array: index -> agentId | null
  const seats: (string | null)[] = Array.from({ length: layout.total }, () => null);
  for (const agentId of members) {
    const seatIdx = seating[agentId];
    if (seatIdx != null && seatIdx >= 0 && seatIdx < layout.total) {
      seats[seatIdx] = agentId;
    }
  }
  // Place any unassigned members into first available seats
  const unassigned = members.filter(
    (id) => seating[id] == null || seating[id] < 0 || seating[id] >= layout.total || seats[seating[id]] !== id
  );
  for (const agentId of unassigned) {
    const emptyIdx = seats.indexOf(null);
    if (emptyIdx !== -1) seats[emptyIdx] = agentId;
  }

  return (
    <div
      style={{
        background: "linear-gradient(135deg, var(--accent-subtle) 0%, var(--card) 60%, var(--bg-hover) 100%)",
        border: "1px solid var(--border)",
        borderRadius: 12,
        padding: 32,
        position: "relative",
        overflow: "hidden",
      }}
    >
      {/* Pixel grid floor pattern */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage:
            "linear-gradient(var(--border) 1px, transparent 1px), linear-gradient(90deg, var(--border) 1px, transparent 1px)",
          backgroundSize: "24px 24px",
          opacity: 0.15,
          pointerEvents: "none",
        }}
      />

      {/* Header */}
      <div style={{ position: "relative", marginBottom: 24, textAlign: "center" }}>
        <span
          style={{
            fontSize: 13,
            fontWeight: 600,
            letterSpacing: "0.05em",
            textTransform: "uppercase",
            color: "var(--muted)",
          }}
        >
          Office &mdash; {members.length} {members.length === 1 ? "agent" : "agents"}
        </span>
      </div>

      {/* Seat grid */}
      <div
        style={{
          position: "relative",
          display: "grid",
          gridTemplateColumns: `repeat(${layout.cols}, 1fr)`,
          gap: 20,
          maxWidth: layout.cols * 140,
          margin: "0 auto",
        }}
      >
        {seats.map((agentId, idx) => (
          <Seat key={idx} agentId={agentId} onClick={onAgentClick} />
        ))}
      </div>
    </div>
  );
}

function Seat({ agentId, onClick }: { agentId: string | null; onClick: (id: string) => void }) {
  const [hovered, setHovered] = React.useState(false);
  const occupied = agentId != null;
  const initial = occupied ? agentId.charAt(0).toUpperCase() : "";

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 8,
      }}
    >
      {/* Desk surface */}
      <div
        style={{
          width: "100%",
          background: occupied ? "var(--card)" : "var(--bg-hover)",
          border: `1px solid var(--border)`,
          borderRadius: 10,
          padding: "16px 8px 12px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 8,
          cursor: occupied ? "pointer" : "default",
          transition: "box-shadow 0.15s, transform 0.15s",
          transform: occupied && hovered ? "translateY(-2px)" : "none",
          boxShadow: occupied && hovered ? "0 4px 12px rgba(0,0,0,0.1)" : "none",
        }}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onClick={() => occupied && onClick(agentId)}
        role={occupied ? "button" : undefined}
        tabIndex={occupied ? 0 : undefined}
        onKeyDown={(e) => {
          if (occupied && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            onClick(agentId);
          }
        }}
      >
        {/* Avatar circle */}
        <div
          style={{
            width: 44,
            height: 44,
            borderRadius: "50%",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 18,
            fontWeight: 700,
            color: occupied ? "var(--text-strong)" : "var(--muted)",
            background: occupied ? "var(--accent-subtle)" : "var(--bg-hover)",
            border: occupied ? "2px solid var(--accent)" : "2px dashed var(--border)",
            transition: "border-color 0.15s",
          }}
        >
          {occupied ? initial : ""}
        </div>

        {/* Name label */}
        <span
          style={{
            fontSize: 12,
            fontWeight: 500,
            color: occupied ? "var(--text-strong)" : "var(--muted)",
            textAlign: "center",
            maxWidth: 90,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {occupied ? agentId : "Empty"}
        </span>
      </div>
    </div>
  );
}
