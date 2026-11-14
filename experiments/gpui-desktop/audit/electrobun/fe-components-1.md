# Electrobun frontend components, part 1

## Coverage
| File | Lines | Read |
|---|---:|---|
| AddMenu.svelte | 34 | 1–34 |
| AnimatedLogo.svelte | 95 | 1–95 |
| BrowseBrowser.svelte | 439 | 1–439 |
| BrowseDialogs.svelte | 138 | 1–138 |
| DriveCard.svelte | 126 | 1–126 |
| DriveContextMenu.svelte | 25 | 1–25 |
| DriveList.svelte | 49 | 1–49 |
| DriveRow.svelte | 123 | 1–123 |
| DrivesView.svelte | 39 | 1–39 |
| EditablePathBar.svelte | 151 | 1–151 |
| FileContextMenu.svelte | 26 | 1–26 |
| FileGrid.svelte | 559 | 1–559 |
| FileList.svelte | 581 | 1–581 |
| FilePreview.svelte | 259 | 1–259 |
| FileThumbnail.svelte | 76 | 1–76 |
| KeyboardShortcutsDialog.svelte | 74 | 1–74 |
| LoadError.svelte | 23 | 1–23 |
| Notifications.svelte | 21 | 1–21 |
| PathEditor.svelte | 583 | 1–583 |
| PicoLogo.svelte | 37 | 1–37 |
| SearchBar.svelte | 117 | 1–117 |
| ShareLinkList.svelte | 144 | 1–144 |
| ShareModal.svelte | 166 | 1–166 |
| Sidebar.svelte | 279 | 1–279 |

## Behaviours
| # | Behaviour | Source | GPUI | GPUI evidence | Notes |
|---:|---|---|:---:|---|---|
| 1 | Toolbar plus menu offers new folder and new file only below root; actions are disabled while busy. | AddMenu.svelte:1-34; BrowseBrowser.svelte:51-55 | ◐ | ui/toolbar.rs:546-560 | No upload actions in native add menu. |
| 2 | BoxBox logo draws layered blue B mark and animated green status lights; busy changes blink animation, hidden document pauses lights, reduced-motion disables animation. | AnimatedLogo.svelte:1-95 | — | none found | Decorative web branding/visibility animation, not file-manager functionality. |
| 3 | Browser pane takes active focus on pointer/focus; root renders drives, directories render list/grid, Trash replaces content and hides toolbar/status bar. | BrowseBrowser.svelte:30-40, 150-185, 189-215, 238-388 | ◐ | ui/browser.rs:26-56, 88-141 | GPUI has pane focus, list/grid and Trash, but its workspace chrome differs; no one-pane sidebar inside each pane. |
| 4 | Pane supports Ctrl/Cmd+L to focus location and Ctrl/Cmd+V paste outside editable controls; paste requires create permission and no open dialog/menu. | BrowseBrowser.svelte:91-119 | ◐ | workspace.rs:143-188 | Ctrl+L and paste are bound; GPUI key context handles text fields, but no matching component-level open-overlay guard. |
| 5 | Native directory watch reloads the pane when a watched directory changes; pending hover prefetch is cancelled on navigation/teardown. | BrowseBrowser.svelte:61-65, 82-89 | ✓ | workspace.rs:367-368; workspace/watching.rs | Watchers start for workspace directories. |
| 6 | Folder hover uses delayed prefetch; drive, sidebar, list and grid folder hover all feed it. | BrowseBrowser.svelte:101-105, 205-212, 244-288 | ◐ | none found | `UI_MAP.md` still-to-do says folder prefetch is not ported; no matching GPUI hover handler found. **known** |
| 7 | Deployment-root failure offers Connections; empty drives show “No configured storage found”; drive loading is explicit. | BrowseBrowser.svelte:216-235; DrivesView.svelte:17-22 | ◐ | ui/browser.rs:30-48 | Loading/empty present; deployment-specific error/Connections action is absent from this native browser branch. |
| 8 | Empty directory error replaces content with error and Retry; refresh failure with stale entries keeps them and presents a retry notice. | BrowseBrowser.svelte:289-306, 419-435 | ◐ | ui/browser.rs:20-24, 58-80, 115-127 | Retry and stale-list notice exist; no delayed spinner in native views. **known** |
| 9 | Upload controls open file or folder picker and accept multiple files / directory selection; upload panel and transfer jobs appear in notifications. | BrowseBrowser.svelte:57-60, 390-439 | ◐ | ui/browser.rs:97-99; ui/toolbar.rs:546-560 | External paths can be dropped onto pane and imported; toolbar picker actions are absent. Transfer jobs are coarse status. |
| 10 | Trash page refreshes browser data after a change. | BrowseBrowser.svelte:189-193 | ✓ | ui/browser.rs:144-245 | Native Trash has restore and permanent-delete actions. |
| 11 | Preview, conflict, Open With, settings, and share overlays load on demand; dismissing overlays closes the corresponding state. | BrowseBrowser.svelte:307-388 | ◐ | ui/overlays.rs:181-211; ui/settings.rs:1421-1480; ui/conflicts.rs | Preview/settings exist. Conflict handling exists; native preview has no previous/next controls. **known** |
| 12 | Preview modal restores focus to opener; Escape closes or exits fullscreen, arrows step previewable items unless focus is interactive; backdrop click closes. | FilePreview.svelte:24-102 | ◐ | ui/overlays.rs:181-210; workspace/operations.rs:1431-1510 | Native read-only preview exists, but no fullscreen, backdrop-close, or previous/next stepping. **known** |
| 13 | Preview header shows filename/size and previous/next buttons; local file opens with default app, remote file downloads; unsupported file type offers open/download fallback. | FilePreview.svelte:126-235 | ◐ | ui/overlays.rs:181-210; workspace/operations.rs:1404-1428 | Native supports basic text/image preview; app opening works. No preview header action row or download fallback in the preview. |
| 14 | Preview content selects image, PDF, code/text, video/audio, office, notebook implementation; loading and unsupported states are shown. | FilePreview.svelte:104-117, 188-235 | ◐ | workspace/preview.rs:90-123; ui/overlays.rs:183-195 | Native handles text and raster/inline-thumbnail content; PDF/video/audio/office/notebook-specific previews are not present. |
| 15 | Preview text/code can save edited content back and notify browser. | FilePreview.svelte:88-90, 216-220 | — | workspace/preview.rs:91-95 | GPUI preview is explicitly read-only; editing belongs to the file's own app. |
| 16 | Create file/folder modal prefills defaults, selects basename/all on focus, Enter submits, Create confirms. | BrowseDialogs.svelte:1-62 | ◐ | workspace/operations.rs:238-250, 276-350 | Native create is a focused text-field dialog; no `untitled.txt`/`New folder` defaults or selection-mode contract. |
| 17 | Rename dialog preloads name, selects basename for files/all for folders, Enter confirms. | BrowseDialogs.svelte:64-77 | ◐ | workspace/operations.rs:238-250, 298-328 | Native rename is dialog-based, not in-place. **known** |
| 18 | Delete confirmation distinguishes local Trash from permanent remote deletion; singular names or plural count/list are shown. | BrowseDialogs.svelte:79-115 | ◐ | ui/overlays.rs:87-95; ui/confirm.rs:1-50 | Native local path says Move to Trash; confirmation is generic and does not enumerate multiple names/count. |
| 19 | Properties shows name, type, parent location, file size, modified date and permissions. | BrowseDialogs.svelte:117-137 | ◐ | ui/overlays.rs:147-178 | Native shows name/type/location/size/modified/provider; permissions are missing. |
| 20 | Drive cards show device/server, filesystem, capacity/free/used and read-only state; usage level controls progress-bar/text styling. | DriveCard.svelte:1-126 | ◐ | components/drive_card.rs:45-171 | Native drive cards show stats and usage; custom stored names/context menu are not wired on cards. |
| 21 | Drive card click opens drive; hover starts folder prefetch; right-click offers Rename and conditional Reset Name. | DriveCard.svelte:102-126; DriveContextMenu.svelte:1-25 | ◐ | ui/drives.rs:954-964 | Opens drive. Rename/reset-name menu is not present. **known** |
| 22 | Drive list exposes responsive Name/Device/Filesystem/Size/Free/Used columns; clicking a row opens drive, hover prefetches, context menu renames. | DriveList.svelte:14-49; DriveRow.svelte:1-123 | ◐ | ui/drives.rs:966-1128 | Native list adapts columns by width and opens rows; custom names/context menu are missing. **known** |
| 23 | Drives view displays loading, empty state, list or responsive card grid; cards cap at three columns. | DrivesView.svelte:17-39 | ◐ | ui/browser.rs:30-56; ui/drives.rs:918-1128 | Both list/cards and states exist; layout thresholds differ. |
| 24 | Path bar renders root/server/mount breadcrumbs, clickable ancestors, root button, page title, and root label. | EditablePathBar.svelte:18-54, 101-151 | ◐ | ui/toolbar.rs:325-477 | Breadcrumbs and root navigation exist. Native root/server crumb equivalence is not established for all remote path kinds. |
| 25 | Click blank path bar, Enter, or F2 opens location editor; Escape closes; home button goes to root; focus returns only when no other element took focus. | EditablePathBar.svelte:56-81, 83-99 | ◐ | workspace.rs:163-164; workspace/operations.rs:259-274, 370-380 | Native Ctrl+L location editor exists; click/Enter/F2 activation and matching focus restoration are absent. |
| 26 | Location editor starts with selected current path; supports local absolute-path and remote server-name normalization, root button, submit, Escape cancel, and invalid-remote toast. | PathEditor.svelte:72-110, 116-155, 306-356 | ◐ | workspace/operations.rs:259-274, 370-380 | Native text location edit exists; suggestion completion is absent. **known** |
| 27 | Location suggestions are delayed 120 ms, list only directories filtered case-insensitively, handle stale requests/errors, and honor hidden-file setting. | PathEditor.svelte:159-265, 286-303 | ✗ | none found | No suggestions in GPUI. **known** |
| 28 | Up/Down cycles suggestions; Tab/ArrowRight completes; Enter chooses active suggestion or path; pointer-down selects suggestion; blur cancels after 120 ms. | PathEditor.svelte:358-583 | ◐ | workspace/operations.rs:259-274 | Basic Enter/Escape location editing exists, but suggestion keyboard/pointer/blur behavior does not. **known** |
| 29 | File context menu adapts actions and disabled state to selection, writable/read-only state, paste availability, favorites and create permission. | FileContextMenu.svelte:1-26; FileGrid.svelte:144-226; FileList.svelte:206-286 | ◐ | ui/overlays.rs:25-145 | Native menu has create, paste, open, preview, copy/cut, favorite, rename, delete, properties, and remote download/share; some action eligibility differs. |
| 30 | Empty-space right-click clears selection and opens background menu; item right-click preserves multi-selection or selects only clicked row/tile; keyboard ContextMenu/Shift+F10 opens menu. | FileGrid.svelte:135-226; FileList.svelte:206-286 | ◐ | ui/file_list.rs:82-94; ui/overlays.rs:25-145 | Pointer right-click menus exist. Keyboard context-menu trigger was not found. |
| 31 | List/grid support single selection, Ctrl/Cmd toggle, Shift range, checkboxes, active focus, and selected/cut styling. | FileGrid.svelte:85-133, 415-458; FileList.svelte:119-179, 430-530 | ◐ | ui/file_list.rs:110-123, 126-149; pane.rs:421-483 | Native has selection, checkbox, active/cut visuals and modifier-based selection; GPUI row range behavior differs and is not a grid's web selection implementation. |
| 32 | Double-click opens folders/files; optional single-click preview applies only to unmodified files. | FileGrid.svelte:88-118, 420-444; FileList.svelte:139-177, 444-467 | ✓ | ui/file_list.rs:126-149; workspace/operations.rs:1397-1428 | Native behavior matches folder open and preference-driven file preview/open. |
| 33 | List view sorts Name/Type/Size/Modified; same header toggles asc/desc, new header starts ascending, sort state is announced. | FileList.svelte:75-83, 293-335 | ◐ | ui/file_list.rs:25-34; sort.rs | Sorting exists; keyboard header activation/ARIA announcement is not applicable to GPUI widget implementation. |
| 34 | List/grid keyboard navigation includes arrows, PageUp/Down, Home/End, Space toggle, Enter open, Delete, F2, Ctrl/Cmd+C/X/V, and keyboard context menu. | FileGrid.svelte:151-226; FileList.svelte:209-286 | ◐ | workspace.rs:143-188; pane.rs:421-483 | Native binds arrows, Enter, Space preview, Delete, F2 and clipboard shortcuts; PageUp/Down, Home/End, keyboard context menu and Ctrl+Space toggle are absent. **known** |
| 35 | Grid uses responsive column count and virtual rows; thumbnails load lazily, image errors fall back to file icon, and filenames can hide extensions. | FileGrid.svelte:52-82, 376-459, 465-486 | ◐ | ui/file_grid.rs:14-64; workspace/preview.rs:9-87 | Native has virtual rows, extension preference, requested inline thumbnails and bounded cache; image-error fallback is not visible as a distinct retry UI. |
| 36 | List virtualizes rows, keeps header sticky, and displays type/size/date with compact mode and optional selection checkbox. | FileList.svelte:70-83, 288-400, 401-581 | ✓ | ui/file_list.rs:15-59; components/file_list.rs:133-212 | Native list virtualizes rows and includes header, metadata, compact mode, checkbox. |
| 37 | Dragging list/grid selection starts file drag; dropping onto a folder moves/imports, self-drop is refused, drop overlay invites upload to pane. | FileGrid.svelte:447-466; FileList.svelte:483-510; BrowseBrowser.svelte:238-252 | ◐ | ui/browser.rs:97-99; drag_out/mod.rs | Native supports drag-out and external paths dropped onto pane; in-app folder/pane drop move/copy is absent. **known** |
| 38 | File name display optionally hides extension, preserves dotfiles, and leaves folders unchanged. | FileGrid.svelte:350-356; FileList.svelte:274-280 | ✓ | ui/file_list.rs:66; ui/file_grid.rs:75 | Uses shared `display_name` helper and setting. |
| 39 | FileThumbnail supports supported-type check, source-key cache invalidation, lazy async image loading, contain/cover fit, accessible alt, and icon fallback on error. | FileThumbnail.svelte:1-76 | ◐ | ui/file_grid.rs:68-85; workspace/preview.rs:9-87,117-123 | Native grid fetches/caches thumbnails; `FileThumbnail.svelte` is unused on Electrobun master and freedesktop shared-thumbnail integration remains open. |
| 40 | F1 dialog lists file, selection, navigation, desktop shortcuts; desktop group is omitted outside local desktop. | KeyboardShortcutsDialog.svelte:1-74 | ◐ | ui/shortcuts.rs:14-47 | Native F1 exists and lists shortcuts, but differs: it documents Space preview, Ctrl+B favorite, Ctrl+Shift+E create file and Ctrl+, settings; no Shift+Delete line. |
| 41 | Dynamic component load failure reports component label and offers Reload (plus optional Close). | LoadError.svelte:1-23 | — | none found | Web chunk/import recovery only; native is not loading Svelte modules. |
| 42 | Notifications stack toasts, transfer jobs, stale refresh errors, and upload panel in one fixed column. | Notifications.svelte:1-21; BrowseBrowser.svelte:419-439 | ◐ | ui/notifications.rs:1-80 | Native has toast/operation notices and jobs, but no same combined upload panel/stale-error stack. |
| 43 | Pico logo renders selectable-disabled ASCII art at three sizes. | PicoLogo.svelte:1-37 | — | none found | Branding component, no native file-manager behavior. |
| 44 | Search submits trimmed nonempty query; input callback debounces 300 ms; Escape or clear button clears and calls clear/input callbacks; focus selects all. | SearchBar.svelte:1-117 | ◐ | ui/toolbar.rs:179-244, 480-544 | Native search field and results exist; debounce/clear-on-Escape/trimmed submit semantics were not found in the native field. |
| 45 | Share list shows item or created time, access, expiry, copy link feedback for 1.5 s, revoke state/error, and never-expiring label. | ShareLinkList.svelte:1-144 | ◐ | ui/settings.rs:720-1038, 1255-1269 | Native shared-link management lists/copies/revokes with confirmation; not the per-item share-list table/feedback timing. |
| 46 | Share modal loads existing item links, defaults to view and never expiry, offers folder access levels and 1h/1d/7d/30d choices, validates whole-number MB upload cap, creates and highlights link. | ShareModal.svelte:1-166 | ◐ | ui/overlays.rs:110-119; ui/settings.rs:312-318, 1321-1355 | Native context share opens shared-link settings/create dialog; item-scoped existing-link list, MB limit and exact defaults/options differ. |
| 47 | Sidebar lists This PC/Server, mounted roots, Places, devices, Trash and favorites; active path is prefix-boundary aware. | Sidebar.svelte:1-68, 201-279 | ✓ | ui/sidebar.rs:29-35, 58-127 | Main navigation surfaces exist. |
| 48 | Sidebar sections collapse; favorites can be unpinned; device mount/eject buttons disable while busy or unavailable; Trash opens local Trash. | Sidebar.svelte:78-112, 130-229, 235-279 | ◐ | ui/sidebar.rs:37-48, 85-126, 130-166 | Collapse persistence/state and favorites/device actions exist. Native sidebar has fixed width and does not become a narrow-window drawer. |
| 49 | On narrow windows sidebar becomes a dismissible drawer; Escape closes it and navigation closes drawer. | Sidebar.svelte:29-36, 78-112, 116-125 | ✗ | none found | Native sidebar is fixed at `SIDEBAR_WIDTH`. **known** |
| 50 | Drive custom names persist and context action conditionally resets to detected name. | DriveCard.svelte:11-18, 25-33, 93-126; DriveContextMenu.svelte:18-23 | ✗ | none found | Drive rename/reset-name is a known gap. **known** |

## Missing or partial, ranked

1. **In-app drag and drop** (rows 37): native pane only accepts external paths; moving selected items onto folders/other pane and Ctrl+drag copy are not implemented. This is core file-manager behavior. **known**
2. **Drive names/context menu** (rows 21-22, 50): no Rename/Reset Name or stored custom names. **known**
3. **Keyboard coverage** (rows 30, 34, 40): Home/End/PageUp/PageDown, Ctrl+Arrow focus movement, Ctrl+Space toggle, Shift+Left/Right grid extension, type-ahead, and Shift+Delete are missing or absent from the native shortcut list. **known**
4. **Location suggestions** (rows 26-28): basic location editing exists, but directory suggestions, completion, and related key/pointer behavior do not. **known**
5. **Preview navigation and zoom** (rows 12-14): no previous/next stepping, fullscreen controls, or image zoom; native supports read-only text/image preview only. **known**
6. **File and selection details** (rows 18-19, 31): confirmations omit affected-name/count detail and properties omit permissions; Alt+Enter aggregation is not established by the component audit. **known**
7. **Delayed load feedback and recovery** (rows 8, 42): native has retry/stale-error messaging but no delayed spinner and has coarser transfer/upload notices. **known**
8. **Responsive sidebar** (row 49): no narrow-window drawer/collapse behavior. **known**
9. **Per-item sharing** (rows 45-46): sharing opens native shared-link settings rather than the item-scoped link list/create modal; upload-limit units and displayed feedback differ.
10. **Freedesktop thumbnail contract** (row 39): native requests thumbnails and caches up to 96 images in memory, but the shared `~/.cache/thumbnails` integration remains open. **known**
