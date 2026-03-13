import { useState, useEffect, useCallback } from 'react'
import { getWorkspaceFiles, getWorkspaceFile, updateWorkspaceFile } from '@/api/agents'
import type { WorkspaceFileItem } from '@/types'
import { Save, ChevronRight, ChevronDown, File, Folder, FolderOpen } from 'lucide-react'

interface Props {
  agentId: string
}

// Build a tree structure from flat file list.
interface TreeNode {
  name: string       // display name (just the segment)
  path: string       // full relative path
  isDir: boolean
  children: TreeNode[]
}

function buildTree(items: WorkspaceFileItem[]): TreeNode[] {
  const root: TreeNode = { name: '', path: '', isDir: true, children: [] }

  for (const item of items) {
    const parts = item.name.split('/')
    let current = root

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      const isLast = i === parts.length - 1
      const fullPath = parts.slice(0, i + 1).join('/')

      let child = current.children.find((c) => c.name === part)
      if (!child) {
        child = {
          name: part,
          path: fullPath,
          isDir: isLast ? item.is_dir : true,
          children: [],
        }
        current.children.push(child)
      }
      current = child
    }
  }

  // Sort: dirs first, then alphabetical.
  const sortNodes = (nodes: TreeNode[]) => {
    nodes.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
      return a.name.localeCompare(b.name)
    })
    nodes.forEach((n) => sortNodes(n.children))
  }
  sortNodes(root.children)
  return root.children
}

function FileTreeNode({
  node,
  depth,
  selected,
  onSelect,
  expanded,
  onToggle,
}: {
  node: TreeNode
  depth: number
  selected: string | null
  onSelect: (path: string) => void
  expanded: Set<string>
  onToggle: (path: string) => void
}) {
  const isOpen = expanded.has(node.path)
  const isSelected = selected === node.path

  if (node.isDir) {
    return (
      <div>
        <button
          onClick={() => onToggle(node.path)}
          className="flex items-center gap-1 w-full text-left py-0.5 px-1 rounded text-sm hover:bg-[rgba(255,255,255,0.04)] transition-colors"
          style={{ paddingLeft: `${depth * 12 + 4}px`, color: '#e6edf3' }}
        >
          {isOpen ? <ChevronDown size={12} style={{ color: '#7d8590' }} /> : <ChevronRight size={12} style={{ color: '#7d8590' }} />}
          {isOpen ? <FolderOpen size={13} style={{ color: '#ff8c5c' }} /> : <Folder size={13} style={{ color: '#ff8c5c' }} />}
          <span className="truncate">{node.name}</span>
        </button>
        {isOpen && node.children.map((child) => (
          <FileTreeNode
            key={child.path}
            node={child}
            depth={depth + 1}
            selected={selected}
            onSelect={onSelect}
            expanded={expanded}
            onToggle={onToggle}
          />
        ))}
      </div>
    )
  }

  return (
    <button
      onClick={() => onSelect(node.path)}
      className="flex items-center gap-1 w-full text-left py-0.5 px-1 rounded text-sm transition-colors"
      style={{
        paddingLeft: `${depth * 12 + 4}px`,
        background: isSelected ? 'var(--accent-subtle)' : 'transparent',
        color: isSelected ? 'var(--accent)' : '#c9d1d9',
      }}
    >
      <File size={13} style={{ color: '#7d8590', flexShrink: 0 }} />
      <span className="truncate">{node.name}</span>
    </button>
  )
}

export default function AgentFiles({ agentId }: Props) {
  const [files, setFiles] = useState<WorkspaceFileItem[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [content, setContent] = useState('')
  const [dirty, setDirty] = useState(false)
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState('')
  const [expanded, setExpanded] = useState<Set<string>>(new Set())

  const loadFiles = useCallback(async () => {
    const f = await getWorkspaceFiles(agentId)
    setFiles(f)
    // Auto-expand top-level directories.
    const dirs = f.filter((item) => item.is_dir && !item.name.includes('/')).map((d) => d.name)
    setExpanded((prev) => {
      const next = new Set(prev)
      dirs.forEach((d) => next.add(d))
      return next
    })
  }, [agentId])

  useEffect(() => { loadFiles() }, [loadFiles])

  const loadFile = async (path: string) => {
    try {
      const f = await getWorkspaceFile(agentId, path)
      setSelected(path)
      setContent(f.content)
      setDirty(false)
      setMsg('')
    } catch {
      setMsg('Cannot read file')
    }
  }

  const handleSave = async () => {
    if (!selected) return
    setSaving(true)
    try {
      await updateWorkspaceFile(agentId, selected, content)
      setMsg('Saved')
      setDirty(false)
    } catch {
      setMsg('Error saving')
    } finally {
      setSaving(false)
    }
  }

  const toggleDir = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const tree = buildTree(files)

  return (
    <div className="flex gap-0 h-full min-h-0">
      {/* File tree */}
      <div
        className="w-[200px] shrink-0 border-r overflow-y-auto py-2 px-1"
        style={{ borderColor: 'var(--border)' }}
      >
        <p className="text-[10px] font-medium uppercase tracking-wide mb-1 px-1" style={{ color: '#7d8590' }}>
          ~/.see-agent/agents/{agentId}
        </p>
        {tree.map((node) => (
          <FileTreeNode
            key={node.path}
            node={node}
            depth={0}
            selected={selected}
            onSelect={loadFile}
            expanded={expanded}
            onToggle={toggleDir}
          />
        ))}
        {files.length === 0 && (
          <p className="text-xs px-1 mt-2" style={{ color: 'var(--muted)' }}>No files</p>
        )}
      </div>

      {/* Editor area */}
      <div className="flex-1 flex flex-col min-h-0 min-w-0">
        {selected ? (
          <>
            {/* File header */}
            <div
              className="flex items-center justify-between px-4 py-2 border-b shrink-0"
              style={{ borderColor: 'var(--border)', background: '#0d1117' }}
            >
              <span className="text-sm font-mono truncate" style={{ color: '#e6edf3' }}>{selected}</span>
              <div className="flex items-center gap-2 shrink-0">
                {msg && (
                  <span className="text-xs" style={{ color: msg === 'Saved' ? 'var(--ok)' : 'var(--danger)' }}>
                    {msg}
                  </span>
                )}
                <button
                  onClick={handleSave}
                  disabled={!dirty || saving}
                  className="flex items-center gap-1 rounded-[var(--radius-sm)] px-3 py-1 text-xs font-medium text-white transition-opacity"
                  style={{ background: 'var(--accent)', opacity: !dirty || saving ? 0.4 : 1 }}
                >
                  <Save size={12} /> Save
                </button>
              </div>
            </div>
            {/* Textarea fills remaining space */}
            <textarea
              value={content}
              onChange={(e) => { setContent(e.target.value); setDirty(true); setMsg('') }}
              onKeyDown={(e) => { if ((e.metaKey || e.ctrlKey) && e.key === 's') { e.preventDefault(); handleSave() } }}
              className="flex-1 min-h-0 w-full text-sm p-4 outline-none resize-none"
              style={{
                background: '#0d1117',
                color: '#e6edf3',
                fontFamily: 'var(--mono, ui-monospace, monospace)',
                lineHeight: '1.6',
                tabSize: 2,
              }}
              spellCheck={false}
            />
          </>
        ) : (
          <div className="flex items-center justify-center flex-1">
            <p className="text-sm" style={{ color: 'var(--muted)' }}>Select a file to edit</p>
          </div>
        )}
      </div>
    </div>
  )
}
