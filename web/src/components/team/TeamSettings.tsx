import React, { useState, useEffect, useCallback } from "react";

interface TeamSettingsProps {
  open: boolean;
  onClose: () => void;
  team: {
    id: string;
    name: string;
    members: string[];
    leader?: string | null;
    status: string;
  };
  onSave: (updates: { name?: string; members?: string[]; leader?: string }) => void;
}

const overlayStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  backgroundColor: "rgba(0, 0, 0, 0.5)",
  zIndex: 999,
};

const drawerStyle: React.CSSProperties = {
  position: "fixed",
  top: 0,
  right: 0,
  bottom: 0,
  width: 380,
  backgroundColor: "var(--bg-elevated)",
  borderLeft: "1px solid var(--border)",
  zIndex: 1000,
  display: "flex",
  flexDirection: "column",
  transition: "transform 0.25s ease-in-out",
};

export default function TeamSettings({ open, onClose, team, onSave }: TeamSettingsProps) {
  const [name, setName] = useState(team.name);
  const [members, setMembers] = useState<string[]>(team.members);
  const [leader, setLeader] = useState(team.leader ?? "");
  const [newMember, setNewMember] = useState("");

  useEffect(() => {
    setName(team.name);
    setMembers(team.members);
    setLeader(team.leader ?? "");
  }, [team]);

  const handleRemoveMember = useCallback((member: string) => {
    setMembers((prev) => prev.filter((m) => m !== member));
    setLeader((prev) => (prev === member ? "" : prev));
  }, []);

  const handleAddMember = useCallback(() => {
    const trimmed = newMember.trim();
    if (trimmed && !members.includes(trimmed)) {
      setMembers((prev) => [...prev, trimmed]);
      setNewMember("");
    }
  }, [newMember, members]);

  const handleSave = useCallback(() => {
    const updates: { name?: string; members?: string[]; leader?: string } = {};
    if (name !== team.name) updates.name = name;
    if (JSON.stringify(members) !== JSON.stringify(team.members)) updates.members = members;
    if (leader && leader !== (team.leader ?? "")) updates.leader = leader;
    onSave(updates);
  }, [name, members, leader, team, onSave]);

  if (!open) return null;

  return (
    <>
      {/* Overlay */}
      <div style={overlayStyle} onClick={onClose} />

      {/* Drawer */}
      <div
        style={{
          ...drawerStyle,
          transform: open ? "translateX(0)" : "translateX(100%)",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "16px 20px",
            borderBottom: "1px solid var(--border)",
          }}
        >
          <h2 style={{ margin: 0, fontSize: 18, color: "var(--text-strong)" }}>
            Team Settings
          </h2>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              cursor: "pointer",
              fontSize: 20,
              color: "var(--muted)",
              padding: 4,
              lineHeight: 1,
            }}
            aria-label="Close"
          >
            &times;
          </button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflowY: "auto", padding: "20px" }}>
          {/* Basic Info */}
          <section style={{ marginBottom: 24 }}>
            <label
              style={{
                display: "block",
                marginBottom: 6,
                fontSize: 13,
                fontWeight: 600,
                color: "var(--text-strong)",
              }}
            >
              Team Name
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              style={{
                width: "100%",
                padding: "8px 12px",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)",
                backgroundColor: "var(--bg-elevated)",
                color: "var(--text-strong)",
                fontSize: 14,
                boxSizing: "border-box",
              }}
            />
          </section>

          {/* Members */}
          <section style={{ marginBottom: 24 }}>
            <label
              style={{
                display: "block",
                marginBottom: 6,
                fontSize: 13,
                fontWeight: 600,
                color: "var(--text-strong)",
              }}
            >
              Members
            </label>
            <ul style={{ listStyle: "none", margin: 0, padding: 0, marginBottom: 8 }}>
              {members.map((member) => (
                <li
                  key={member}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "6px 10px",
                    marginBottom: 4,
                    borderRadius: "var(--radius-sm)",
                    border: "1px solid var(--border)",
                    fontSize: 14,
                    color: "var(--text-strong)",
                  }}
                >
                  <span>{member}</span>
                  <button
                    onClick={() => handleRemoveMember(member)}
                    style={{
                      background: "none",
                      border: "none",
                      cursor: "pointer",
                      color: "var(--danger)",
                      fontSize: 16,
                      lineHeight: 1,
                      padding: "0 2px",
                    }}
                    aria-label={`Remove ${member}`}
                  >
                    &times;
                  </button>
                </li>
              ))}
            </ul>
            <input
              type="text"
              placeholder="Add member..."
              value={newMember}
              onChange={(e) => setNewMember(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleAddMember();
                }
              }}
              style={{
                width: "100%",
                padding: "8px 12px",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)",
                backgroundColor: "var(--bg-elevated)",
                color: "var(--text-strong)",
                fontSize: 14,
                boxSizing: "border-box",
              }}
            />
          </section>

          {/* Leader */}
          <section style={{ marginBottom: 24 }}>
            <label
              style={{
                display: "block",
                marginBottom: 6,
                fontSize: 13,
                fontWeight: 600,
                color: "var(--text-strong)",
              }}
            >
              Leader
            </label>
            <select
              value={leader}
              onChange={(e) => setLeader(e.target.value)}
              style={{
                width: "100%",
                padding: "8px 12px",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)",
                backgroundColor: "var(--bg-elevated)",
                color: "var(--text-strong)",
                fontSize: 14,
                boxSizing: "border-box",
              }}
            >
              <option value="">Select a leader</option>
              {members.map((member) => (
                <option key={member} value={member}>
                  {member}
                </option>
              ))}
            </select>
          </section>
        </div>

        {/* Footer / Save */}
        <div
          style={{
            padding: "16px 20px",
            borderTop: "1px solid var(--border)",
          }}
        >
          <button
            onClick={handleSave}
            style={{
              width: "100%",
              padding: "10px 0",
              backgroundColor: "var(--accent)",
              color: "#fff",
              border: "none",
              borderRadius: "var(--radius-lg)",
              fontSize: 14,
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            Save
          </button>
        </div>
      </div>
    </>
  );
}
