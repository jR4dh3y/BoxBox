# Line-by-line feature audit

We are porting BoxBox's desktop file manager from Electrobun (a Svelte UI
plus a Go server) to a native GPUI app in Rust. The GPUI app is a native
Linux file manager that looks like BoxBox. Features have been lost in the
port because earlier passes skimmed. Your job is to find every behaviour in
your chunk of source and say whether the GPUI app has it.

## Inputs

- Your chunk: a list of files, one absolute path per line.
- GPUI app source (Rust): `/home/radhey/.t3/worktrees/BoxBox/gpui-native/experiments/gpui-desktop/src/`
- Current map of known gaps: `/home/radhey/.t3/worktrees/BoxBox/gpui-native/experiments/gpui-desktop/UI_MAP.md`
  (read the "Still to do" section first; still record those items, but tag them `known`).

## Rules

1. Read every line of every file in your chunk. Use a file viewer in
   ranges of at most 400 lines and keep going until the last line. Do not
   grep instead of reading, and do not skip "boilerplate".
2. Record every behaviour a user or the OS can observe. That includes:
   keys and modifiers, mouse buttons, double-click and middle-click,
   drag-and-drop, menu items and when they are disabled, dialogs and their
   wording, confirmations, defaults, limits and thresholds, sorting and
   naming rules, error messages and recovery, empty and loading states,
   timing (debounce, delays), persistence, accessibility, focus handling,
   freedesktop/D-Bus/portal/GIO contracts, MIME and desktop-file handling,
   thumbnails, trash spec details, and conflict handling.
   One row per behaviour. Small edge cases count; they are what got lost.
3. For each behaviour, search the GPUI source (rg) and open the matching
   code to decide its status:
   `✓` present · `◐` partial (say what is missing) · `✗` missing ·
   `—` not wanted in GPUI (web-only, server-only, or replaced by a native
   app; say why).
4. Do not edit any file except your own output file. Do not build, run,
   commit, or install anything.

## Output

Write `<output path given in your task>` as Markdown:

```
# <chunk name>

## Coverage
| File | Lines | Read |
|---|---|---|
| path | 812 | 1–812 |

## Behaviours
| # | Behaviour | Source | GPUI | GPUI evidence | Notes |
|---|---|---|---|---|---|
| 1 | Shift+Delete asks, then deletes permanently | FileList.svelte:212 | ✗ | none found | |

## Missing or partial, ranked
Short list of the `✗` and `◐` rows that matter most for a native Linux
file manager, most important first, each with one line on why.
```

Source is `file:line` (basename is fine when unique). GPUI evidence is
`file:line` or `none found`. The Coverage table must list every file in your
chunk with its full line count; a file you did not finish is a failed task.
