---
name: journal
description: "Personal journal and note-taking assistant. Write timestamped entries, capture ideas, log decisions, and maintain a searchable knowledge base in the workspace."
version: 1.0.0
author: starkbot
metadata: {"clawdbot":{"emoji":"📓"}}
requires_tools: [write_file, read_file, list_files, glob, grep]
tags: [journal, notes, secretary, productivity, memory, documentation]
arguments:
  action:
    description: "Action to perform: write, read, search, list, today, summary"
    required: false
    default: "write"
  entry_type:
    description: "Type of entry: log, idea, decision, note, reflection, todo"
    required: false
    default: "log"
  content:
    description: "The content to write (for write action)"
    required: false
  query:
    description: "Search query (for search/read actions)"
    required: false
  date:
    description: "Specific date in YYYY-MM-DD format (defaults to today)"
    required: false
---

# Journal - Personal Knowledge Base

You are maintaining a journal and knowledge base. All entries are stored in the `journal/` directory, which is a dedicated folder separate from workspace.

**IMPORTANT:** Use paths starting with `journal/` (e.g., `journal/2026/01/2026-01-31.md`) - the file tools automatically route these to the journal directory.

## Directory Structure

```
journal/
├── 2024/
│   ├── 01/
│   │   ├── 2024-01-15.md    # Daily journal file
│   │   ├── 2024-01-16.md
│   │   └── ...
│   └── 02/
│       └── ...
├── ideas/
│   └── ideas-index.md       # Running index of all ideas
├── decisions/
│   └── decisions-log.md     # Decision log with context
└── README.md                # Journal overview
```

## Entry Types

| Type | Symbol | Purpose |
|------|--------|---------|
| `log` | 📝 | General daily log entries, updates, progress |
| `idea` | 💡 | New ideas, concepts, inspirations |
| `decision` | ⚖️ | Decisions made with rationale |
| `note` | 📌 | Quick notes, reminders, information |
| `reflection` | 🔮 | Thoughts, learnings, retrospectives |
| `todo` | ☐ | Tasks and action items |

## Actions

### 1. WRITE - Add a Journal Entry

When user wants to write/add/log something:

**Step 1:** Get current date and format paths
```json
{"tool": "exec", "command": "date +%Y-%m-%d"}
```

**Step 2:** Create/append to daily journal file

For the entry, use this markdown format:
```markdown
### HH:MM - [TYPE_SYMBOL] Entry Title

[Content here]

---
```

Example write operation:
```json
{
  "tool": "write_file",
  "path": "journal/2024/01/2024-01-15.md",
  "content": "### 14:32 - 📝 Project Update\n\nCompleted the authentication module. Next up: testing.\n\n---\n",
  "append": true,
  "create_dirs": true
}
```

**Daily file header** (create if file doesn't exist):
```markdown
# Journal: YYYY-MM-DD

*Day of the week, Month Day, Year*

---

```

### 2. READ - View Journal Entries

**Read today's journal:**
```json
{"tool": "read_file", "path": "journal/2024/01/2024-01-15.md"}
```

**Read specific date:**
```json
{"tool": "read_file", "path": "journal/2024/01/2024-01-10.md"}
```

### 3. SEARCH - Find Past Entries

**Search all journals for a term:**
```json
{"tool": "grep", "pattern": "authentication", "path": "journal/", "recursive": true}
```

**Search by entry type:**
```json
{"tool": "grep", "pattern": "💡", "path": "journal/", "recursive": true}
```

### 4. LIST - Show Available Journals

**List all journal files:**
```json
{"tool": "glob", "pattern": "journal/**/*.md"}
```

**List journals for a month:**
```json
{"tool": "list_files", "path": "journal/2024/01"}
```

### 5. TODAY - Quick View Today's Entries

Read the current day's journal file. If it doesn't exist, offer to create it.

### 6. SUMMARY - Generate Summary

Read recent entries and provide a summary of:
- Key activities and progress
- Decisions made
- Ideas captured
- Outstanding todos

## Special Entry Handling

### Ideas (💡)

When logging an idea, ALSO append to the ideas index:

```json
{
  "tool": "write_file",
  "path": "journal/ideas/ideas-index.md",
  "content": "- **2024-01-15**: [Idea Title] - Brief description. See: journal/2024/01/2024-01-15.md\n",
  "append": true,
  "create_dirs": true
}
```

### Decisions (⚖️)

When logging a decision, ALSO append to decisions log:

```json
{
  "tool": "write_file",
  "path": "journal/decisions/decisions-log.md",
  "content": "\n## 2024-01-15: [Decision Title]\n\n**Context:** Why this decision was needed\n\n**Decision:** What was decided\n\n**Rationale:** Why this choice was made\n\n**Alternatives considered:** Other options that were rejected\n\n---\n",
  "append": true,
  "create_dirs": true
}
```

### Todos (☐)

Format todos with checkboxes:
```markdown
### 14:32 - ☐ Tasks for Today

- [ ] Complete authentication module
- [ ] Review PR #42
- [ ] Update documentation

---
```

## Initialization

If `journal/README.md` doesn't exist, create it:

```json
{
  "tool": "write_file",
  "path": "journal/README.md",
  "content": "# StarkBot Journal\n\nPersonal journal and knowledge base.\n\n## Structure\n\n- `YYYY/MM/` - Daily journal entries organized by year and month\n- `ideas/` - Index of all captured ideas\n- `decisions/` - Log of important decisions with context\n\n## Entry Types\n\n- 📝 Log - General updates and progress\n- 💡 Idea - New concepts and inspirations\n- ⚖️ Decision - Decisions with rationale\n- 📌 Note - Quick notes and reminders\n- 🔮 Reflection - Thoughts and learnings\n- ☐ Todo - Tasks and action items\n",
  "create_dirs": true
}
```

## User Interaction

When the user invokes this skill:

1. **Understand intent** - Are they writing, reading, or searching?
2. **Get current timestamp** - Use exec to get current date/time
3. **Perform action** - Use appropriate tools
4. **Confirm completion** - Tell user what was done and where

### Example Interactions

**User:** "Log that I finished the API integration"
→ Write a 📝 log entry to today's journal

**User:** "I have an idea for a new feature"
→ Write a 💡 idea entry + update ideas index

**User:** "What did I work on last week?"
→ Read journals from the past 7 days and summarize

**User:** "Search my notes for database"
→ Grep through journal/ for "database"

**User:** "Show me all my decisions this month"
→ Read decisions-log.md or grep for ⚖️
