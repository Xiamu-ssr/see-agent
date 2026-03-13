import { useState, useEffect, useCallback } from 'react'
import { getWorkspaceFiles, getWorkspaceFile, updateWorkspaceFile } from '@/api/agents'
import type { WorkspaceFileItem } from '@/types'
import { Save, ChevronRight, ChevronDown, File, Folder, FolderOpen, Copy, WrapText, Check, ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import CodeEditor from '@/components/ui/CodeEditor'

interface Props {
  agentId: string
}

interface TreeNode {
  name: string
  path: string
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
          className="flex items-center gap-1 w-full text-left py-0.5 px-1 rounded text-sm hover:bg-[var(--bg-hover)] transition-colors text-[var(--text-strong)]"
          style={{ paddingLeft: `${depth * 12 + 4}px` }}
        >
          {isOpen ? <ChevronDown size={12} className="text-[var(--muted)]" /> : <ChevronRight size={12} className="text-[var(--muted)]" />}
          {isOpen ? <FolderOpen size={13} className="text-[var(--warn)]" /> : <Folder size={13} className="text-[var(--warn)]" />}
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
        color: isSelected ? 'var(--accent)' : 'var(--text)',
      }}
    >
      <File size={13} className="text-[var(--muted)] shrink-0" />
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
  const [wordWrap, setWordWrap] = useState(true)
  const [copied, setCopied] = useState(false)

  const loadFiles = useCallback(async () => {
    const f = await getWorkspaceFiles(agentId)
    setFiles(f)
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

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault()
        handleSave()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  })

  const toggleDir = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const tree = buildTree(files)

  // Detect extension for language label
  const ext = selected?.split('.').pop() ?? ''

  return (
    <div className="flex gap-0 h-full min-h-0">
      {/* File tree — full width on mobile when no file selected, hidden when editing */}
      <ScrollArea className={`${selected ? 'hidden md:block' : 'w-full'} md:w-[220px] shrink-0 border-r border-[var(--border)] py-2 px-1`}>
        <p className="text-[10px] font-medium uppercase tracking-wide mb-1 px-1 text-[var(--muted)]">
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
          <p className="text-xs px-1 mt-2 text-[var(--muted)]">No files</p>
        )}
      </ScrollArea>

      {/* Editor — full width on mobile when file selected */}
      <div className={`${!selected ? 'hidden md:flex' : 'flex'} flex-1 flex-col min-h-0 min-w-0`}>
        {selected ? (
          <>
            <div className="flex items-center justify-between px-3 md:px-4 py-2 border-b border-[var(--border)] bg-[var(--bg)] shrink-0">
              <div className="flex items-center gap-2 min-w-0">
                {/* Mobile back to tree */}
                <button
                  onClick={() => setSelected(null)}
                  className="md:hidden p-1 -ml-1 rounded hover:bg-[var(--bg-hover)] transition-colors"
                >
                  <ArrowLeft size={14} className="text-[var(--muted)]" />
                </button>
                <span className="text-sm font-mono truncate text-[var(--text-strong)]">{selected}</span>
                {ext && (
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg-hover)] text-[var(--muted)] shrink-0 hidden md:inline">
                    {ext}
                  </span>
                )}
                <button
                  onClick={() => {
                    const fullPath = `~/.see-agent/agents/${agentId}/${selected}`
                    navigator.clipboard.writeText(fullPath)
                    setCopied(true)
                    setTimeout(() => setCopied(false), 1500)
                  }}
                  className="shrink-0 p-1 rounded hover:bg-[var(--accent-subtle)] transition-colors"
                  title="Copy full path"
                >
                  {copied ? <Check size={12} className="text-[var(--ok)]" /> : <Copy size={12} className="text-[var(--muted)]" />}
                </button>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <button
                  onClick={() => setWordWrap(!wordWrap)}
                  className={`p-1 rounded transition-colors ${wordWrap ? 'bg-[var(--accent-subtle)]' : ''}`}
                  title={wordWrap ? 'Disable word wrap' : 'Enable word wrap'}
                >
                  <WrapText size={14} style={{ color: wordWrap ? 'var(--accent)' : 'var(--muted)' }} />
                </button>
                {msg && (
                  <span className={`text-xs ${msg === 'Saved' ? 'text-[var(--ok)]' : 'text-[var(--danger)]'}`}>
                    {msg}
                  </span>
                )}
                <Button onClick={handleSave} disabled={!dirty || saving} size="sm">
                  <Save size={12} /> Save
                </Button>
              </div>
            </div>
            <div className="flex-1 min-h-0">
              <CodeEditor
                value={content}
                onChange={(v) => { setContent(v); setDirty(true); setMsg('') }}
                filename={selected}
                wordWrap={wordWrap}
              />
            </div>
          </>
        ) : (
          <div className="flex items-center justify-center flex-1">
            <p className="text-sm text-[var(--muted)]">Select a file to edit</p>
          </div>
        )}
      </div>
    </div>
  )
}
