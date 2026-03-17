import Editor from "@monaco-editor/react"

const EXT_LANG_MAP: Record<string, string> = {
  ".py": "python",
  ".json": "json",
  ".md": "markdown",
  ".yaml": "yaml",
  ".yml": "yaml",
  ".toml": "toml",
  ".ts": "typescript",
  ".tsx": "typescriptreact",
  ".js": "javascript",
  ".jsx": "javascript",
  ".sh": "shell",
  ".bash": "shell",
  ".css": "css",
  ".html": "html",
  ".xml": "xml",
  ".sql": "sql",
  ".rs": "rust",
  ".go": "go",
  ".rb": "ruby",
  ".txt": "plaintext",
}

function detectLanguage(filename?: string): string {
  if (!filename) return "plaintext"
  const dot = filename.lastIndexOf(".")
  if (dot < 0) return "plaintext"
  const ext = filename.slice(dot).toLowerCase()
  return EXT_LANG_MAP[ext] || "plaintext"
}

interface CodeEditorProps {
  value: string
  onChange?: (value: string) => void
  language?: string
  readOnly?: boolean
  filename?: string
  wordWrap?: boolean
}

export default function CodeEditor({
  value,
  onChange,
  language,
  readOnly = false,
  filename,
  wordWrap = true,
}: CodeEditorProps) {
  const lang = language || detectLanguage(filename)

  return (
    <Editor
      height="100%"
      language={lang}
      value={value}
      theme="vs-dark"
      onChange={(v) => onChange?.(v ?? "")}
      options={{
        readOnly,
        minimap: { enabled: false },
        fontSize: 13,
        lineNumbers: "on",
        scrollBeyondLastLine: false,
        wordWrap: wordWrap ? "on" : "off",
        tabSize: 2,
        renderWhitespace: "none",
        overviewRulerLanes: 0,
        hideCursorInOverviewRuler: true,
        scrollbar: {
          verticalScrollbarSize: 6,
          horizontalScrollbarSize: 6,
        },
        padding: { top: 12, bottom: 12 },
      }}
    />
  )
}
