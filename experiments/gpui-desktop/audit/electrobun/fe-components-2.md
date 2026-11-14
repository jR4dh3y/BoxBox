# Electrobun frontend components, part 2

## Coverage
| File | Lines | Read |
|---|---:|---|
| StatusBar.svelte | 68 | 1–68 |
| Toolbar.svelte | 177 | 1–177 |
| UploadPanel.svelte | 143 | 1–143 |
| UploadProgress.svelte | 94 | 1–94 |
| desktop/ApplicationIcon.svelte | 23 | 1–23 |
| desktop/BrowseWorkspace.svelte | 114 | 1–114 |
| desktop/ChooserLocations.svelte | 132 | 1–132 |
| desktop/DeploymentIcon.svelte | 15 | 1–15 |
| desktop/DesktopUndoShortcut.svelte | 30 | 1–30 |
| desktop/FileChooser.svelte | 240 | 1–240 |
| desktop/OpenWithDialog.svelte | 200 | 1–200 |
| desktop/RemoteCatalog.svelte | 11 | 1–11 |
| desktop/RemoteCatalogQueries.svelte | 62 | 1–62 |
| desktop/ServersPage.svelte | 272 | 1–272 |
| desktop/TransferConflictDialog.svelte | 120 | 1–120 |
| desktop/TransferJobs.svelte | 120 | 1–120 |
| desktop/TrashPage.svelte | 186 | 1–186 |
| preview/AudioPreview.svelte | 40 | 1–40 |
| preview/CodePreview.svelte | 237 | 1–237 |
| preview/ImagePreview.svelte | 105 | 1–105 |
| preview/NotebookPreview.svelte | 241 | 1–241 |
| preview/OfficePreview.svelte | 327 | 1–327 |
| preview/PdfPreview.svelte | 32 | 1–32 |
| preview/VideoPreview.svelte | 85 | 1–85 |
| preview/codeLanguage.ts | 97 | 1–97 |
| preview/monaco.d.ts | 2 | 1–2 |
| preview/monacoLanguages.ts | 55 | 1–55 |
| preview/registry.ts | 30 | 1–30 |
| settings/AccountSettings.svelte | 24 | 1–24 |
| settings/AppearanceSettings.svelte | 267 | 1–267 |
| settings/GeneralSettings.svelte | 96 | 1–96 |
| settings/PairingCodeSettings.svelte | 108 | 1–108 |
| settings/SettingsModal.svelte | 107 | 1–107 |
| settings/SettingsRow.svelte | 12 | 1–12 |
| settings/ShareLinksSettings.svelte | 51 | 1–51 |

## Behaviours
| # | Behaviour | Source | GPUI | GPUI evidence | Notes |
|---:|---|---|:---:|---|---|
| 1 | Status line announces selected count of total and selected byte size when positive; otherwise singular/plural item count | StatusBar.svelte:22 | ◐ | ui/status_bar.rs:13 | GPUI status bar exists; compare selected-size/folder-only wording in its renderer. |
| 2 | List/grid view buttons show active state and notify view change | StatusBar.svelte:28 | ✓ | ui/status_bar.rs:40 | Both modes selectable in GPUI. |
| 3 | Back/Forward disabled without history; sidebar toggle is shown on narrow screens | Toolbar.svelte:58 | ◐ | ui/toolbar.rs:280 | Navigation exists; responsive sidebar affordance is not established in native UI. |
| 4 | Editable path bar supports navigation and hidden-path suggestions; optional page title and server name | Toolbar.svelte:24 | ◐ | ui/toolbar.rs:280 | Path entry exists; suggestion behavior is a known gap (UI_MAP Still to do). |
| 5 | Optional search field reports loading, clear and query input; responsive width and placeholder | Toolbar.svelte:92 | ◐ | ui/toolbar.rs:280 | Search support exists; full parity with component behavior not established. |
| 6 | Add menu disabled when neither create nor upload is possible; otherwise opens at button position and lazy-loads; import failure shows dismissible LoadError | Toolbar.svelte:48 | ◐ | ui/toolbar.rs:280 | GPUI has context menus; web AddMenu/new/upload options are not all represented (UI_MAP). |
| 7 | Refresh respects disabled state; Settings action opens settings | Toolbar.svelte:113 | ✓ | ui/toolbar.rs:280; ui/settings.rs:1424 | Native toolbar and settings are present. |
| 8 | Upload panel appears only when visible and uploads exist; newly added uploads reopen and expand a closed panel | UploadPanel.svelte:16 | — | none found | Browser upload queue is web-only; native copy-in uses transfer jobs. |
| 9 | Upload panel can minimize, expand or close; transition is 200 ms; live announcement and header reflect active/total/completed uploads | UploadPanel.svelte:49 | — | none found | Browser upload UI. |
| 10 | Clear finished is available when any upload is complete/error/cancelled; cancel active uploads and remove terminal rows | UploadPanel.svelte:77 | — | none found | Browser upload queue; distinct from native transfer job handling. |
| 11 | Upload row shows filename, status/error, percentage, uploaded/total bytes; row action changes from Cancel to Remove at terminal status | UploadProgress.svelte:16 | — | none found | Browser upload UI. |
| 12 | Workspace renders tabs, closes a tab only when multiple tabs exist, offers New tab (Ctrl+T) and Split pane (F3) | desktop/BrowseWorkspace.svelte:24 | ✓ | ui/tabs.rs; workspace.rs:1218 | Native tabs/split are wired. |
| 13 | Tab title reflects remote server/path, Trash, or local path/root; tab activation and pane focus are separate | desktop/BrowseWorkspace.svelte:31 | ✓ | ui/tabs.rs; ui/browser.rs | Native tabs/panes exist. |
| 14 | Workspace sidebar routes navigation, trash, volume actions and closes/toggles for narrow view; connections opens settings at connections section | desktop/BrowseWorkspace.svelte:15 | ◐ | ui/sidebar.rs; ui/settings.rs:1424 | Sidebar/volume support exists; GVfs MTP/network and pairing/server pages remain later/incomplete per UI_MAP. |
| 15 | Chooser unavailable outside Desktop; desktop chooser initializes from request id, disposes on unmount, title/app id reflect request | desktop/FileChooser.svelte:9 | ✗ | none found | Portal/system picker is an explicit known gap (UI_MAP Still to do). |
| 16 | Chooser Ctrl+F focuses/selects search; location accepts Enter; parent button disabled at root; search filters current folder and Escape/clear empties it | desktop/FileChooser.svelte:16 | ✗ | none found | No chooser surface in GPUI. |
| 17 | Chooser supports roots, Other locations, read-only sorted/selectable file list, hidden files, loading/error/empty states and sort controls | desktop/FileChooser.svelte:28 | ✗ | none found | Includes local/remote roots and location view; no portal chooser in GPUI. |
| 18 | Chooser request modes include open, save, save-files; filter/choice fields, filename, cancellation and disabled accept conditions; remote open downloads local copy while save/folder selection is disallowed | desktop/FileChooser.svelte:164 | ✗ | none found | Native system picker (xdg portal) remains explicitly wanted and missing. |
| 19 | Other locations refreshes drives and servers; volume list polls every 5 seconds while visible and idle; disabled when busy/unavailable | desktop/ChooserLocations.svelte:7 | ✗ | none found | Part of missing chooser. |
| 20 | Locations show retryable server/root load failures, no-drive/no-server/empty-root states, and mount/open action availability | desktop/ChooserLocations.svelte:68 | ✗ | none found | Part of missing chooser. |
| 21 | Application icon lazy-loads desktop icon and falls back to generic AppWindow on absent icon or image error | desktop/ApplicationIcon.svelte:7 | ✓ | ui/open_with.rs:14 | Native Open With renders app rows/icons via its Linux app integration. |
| 22 | Open With loads app list on open, aborts stale requests, delays spinner, supports reload on error and empty-installed-app state | desktop/OpenWithDialog.svelte:31 | ✓ | ui/open_with.rs:62; workspace/open_with.rs | Native Open With flow exists; exact spinner/reload wording may differ. |
| 23 | Open With filters recommended/other apps, chooses initial choice, search Enter/double-click opens, optional set-default, disables while launching and reports errors | desktop/OpenWithDialog.svelte:82 | ◐ | ui/open_with.rs:62; workspace/open_with.rs:75 | Open With is present; verify default-app persistence and all web interaction states before calling full parity. |
| 24 | Remote catalog refreshes server/root/stats every 15 seconds, refetches on focus, loads drive stats only at a remote root and publishes typed loading/error/ready snapshot | desktop/RemoteCatalogQueries.svelte:16 | ◐ | workspace.rs:25; api/http.rs | Remote catalog exists in native scope only partially; saved server catalog remains later work per UI_MAP. |
| 25 | Deployment icon maps server/cloud/home/building/hard-drive choices to matching glyph | desktop/DeploymentIcon.svelte:3 | ◐ | none found | Server page not in current GPUI scope. |
| 26 | Servers page refreshes immediately and every 30 seconds; delayed spinner; edit form focuses name or pairing code; Escape closes unless busy | desktop/ServersPage.svelte:16 | ✗ | none found | Servers page is explicitly later work (UI_MAP). |
| 27 | Servers add/edit/pair-again with icon, name, address, pairing code; cancel/save and busy labels | desktop/ServersPage.svelte:47 | ✗ | none found | Saved remote server management is absent in GPUI. |
| 28 | Server list shows connection state/details; retry offline, pair again if pairing expired, edit name/icon; remove requires confirmation and can cancel | desktop/ServersPage.svelte:143 | ✗ | none found | Saved remote server management is absent in GPUI. |
| 29 | Conflict dialog defaults each collision to Skip; permits Keep both, previews generated destination name; cancel batch or continue/skip all with item count | desktop/TransferConflictDialog.svelte:17 | ✓ | ui/conflicts.rs:40 | GPUI conflict modal implements Skip/Keep both choices. |
| 30 | Transfer jobs display active/recent operations, summary, source/destination, progress/status; cancel active with error toast, dismiss finished, clear finished | desktop/TransferJobs.svelte:12 | ◐ | progress.rs; ui/notifications.rs:24 | Native transfer progress is coarse; detailed web job panel is a known partial. |
| 31 | Trash loads on mount, offers refresh; delayed spinner, retry on load failure and empty state | desktop/TrashPage.svelte:31 | ◐ | workspace.rs:479; ui/browser.rs:26 | Trash exists; compare loading/error/retry UI details. |
| 32 | Trash rows select by click/Enter/Space; Delete on selected row begins permanent-delete confirmation; checkbox selection can be disabled | desktop/TrashPage.svelte:50 | ◐ | ui/browser.rs:200 | Native Trash exists; keyboard and checkbox parity not verified in renderer evidence. |
| 33 | Trash restore disabled unless restorable; permanent delete asks confirmation, labels running action and warns it cannot be undone | desktop/TrashPage.svelte:36 | ✓ | workspace.rs:767; workspace.rs:801; ui/browser.rs:216 | Native restore/delete operations are present. |
| 34 | Trash table shows original location and deleted date; selected row styling and disabled row focus during action | desktop/TrashPage.svelte:115 | ◐ | ui/browser.rs:26 | UI details may differ; native Trash is present. |
| 35 | Audio preview autoplay with metadata preload and native controls; reports unsupported/load error | preview/AudioPreview.svelte:12 | — | none found | Audio is intentionally opened in native media app per UI_MAP. |
| 36 | Code preview loads content by URL, read-only unless save callback supplied; Monaco lazy-load, worker, minimap, line numbers, wrapping, language selection | preview/CodePreview.svelte:15 | ◐ | workspace/preview.rs; ui/overlays.rs:181 | Native text preview exists; editor features/editing are beyond the read-only native preview contract. |
| 37 | Editable code preview tracks dirty state, saves on button or Ctrl/Cmd+S, reports saving/saved/error; load failure and Monaco fallback text | preview/CodePreview.svelte:57 | — | none found | Editing is not part of native read-only preview scope. |
| 38 | Image preview starts at 100%, zooms in/out by 25% within 25–500%, wheel zoom, rotates by 90°, reset to original, reports loading/error | preview/ImagePreview.svelte:16 | ◐ | workspace/preview.rs; ui/overlays.rs:181 | Image zoom is a known GPUI gap (UI_MAP Still to do). |
| 39 | Notebook preview parses notebook JSON, validates cells, displays cell types/source/execution count, stream/error/plain outputs and PNG/JPEG/GIF/WebP inline images | preview/NotebookPreview.svelte:15 | — | workspace/preview.rs | Native preview is quick-look only; notebook documents open in their app per UI_MAP. |
| 40 | Office preview supports docx/docm/dotx/dotm; spreadsheet formats with sheet tabs and first 250 rows; presentation formats; download fallback | preview/OfficePreview.svelte:9 | — | workspace/preview.rs | Office documents open in native app by design. |
| 41 | Office loading aborts on navigation, times out DOCX/PPTX render at 30 seconds, distinguishes unsupported formats and fetch/render errors | preview/OfficePreview.svelte:53 | — | workspace/preview.rs | Browser-specific renderer states are out of native preview scope. |
| 42 | PDF iframe preview is sandboxed and reports load failure with download guidance | preview/PdfPreview.svelte:8 | ◐ | workspace/preview.rs; ui/overlays.rs:181 | GPUI PDF support is thumbnail/first-page only (UI_MAP). |
| 43 | Video preview uses native browser controls, metadata preload, inline playback; codec/format-specific errors and optional download | preview/VideoPreview.svelte:8 | — | none found | Video intentionally opens in native media player. |
| 44 | Preview registry lazily loads and caches component modules for video/audio/image/PDF/office/notebook/code/text | preview/registry.ts:1 | — | workspace/preview.rs | Native dispatch replaces web component registry. |
| 45 | Filename extension/dotfile mappings choose syntax highlighting languages; unrecognized names use plaintext | preview/codeLanguage.ts:8 | ◐ | workspace/preview.rs | Native displays text; Monaco syntax language coverage is absent from read-only text preview. |
| 46 | General settings toggle hidden files, extensions, delete confirmation, single-click preview and checkboxes; choose default sort/direction/view | settings/GeneralSettings.svelte:15 | ◐ | ui/settings.rs:94; ui/settings.rs:106; ui/settings.rs:116 | GPUI preferences include hidden files, confirm-delete and sorting; extensions/single-click/checkbox/default-view parity is partial. |
| 47 | Appearance settings choose wallpaper/custom accent, fit, tint, frosted glass, compact rows, hardware acceleration; invalid typed accent is not applied; errors toast | settings/AppearanceSettings.svelte:15 | ◐ | ui/settings.rs:138; workspace/appearance.rs:69 | Most wallpaper/accent settings exist; hardware acceleration, frosted glass and compact rows need parity confirmation. Wallpaper picker is a known untested area. |
| 48 | Wallpaper chooser can use desktop native file picker; change/remove, apply/clear, show wallpaper name and fit/tint options | settings/AppearanceSettings.svelte:63 | ◐ | ui/settings.rs:214; ui/settings.rs:364 | Native chooser is implemented; UI_MAP calls picker behavior untested. |
| 49 | Settings modal offers sections, responsive tabs, reset confirmation and clears favorites/saved wallpaper; settings changes apply immediately | settings/SettingsModal.svelte:2 | ◐ | ui/settings.rs:44; ui/settings.rs:1424 | Core settings exists; account and pairing sections are not equivalent native surfaces. |
| 50 | Account page shows signed-in identity/sign-out; development mode explains sign-in is off; pairing management is embedded | settings/AccountSettings.svelte:8 | — | none found | Sign-in is web-only in this desktop app; pairing is separately later. |
| 51 | Pairing settings list/revoke devices, refresh on window focus, generate code after password confirmation, copy code, show expiry/used state and errors | settings/PairingCodeSettings.svelte:9 | ✗ | none found | Pairing is explicitly later work (UI_MAP). |
| 52 | Share links load with retry/empty states, list active links and remove revoked link locally | settings/ShareLinksSettings.svelte:14 | — | ui/settings.rs | Server share management is out of native scope per UI_MAP. |
| 53 | SettingsRow lays out labeled controls with separators and flexible wrapping | settings/SettingsRow.svelte:3 | ✓ | ui/settings.rs | Native SettingsRow primitive exists. |
| 54 | Ctrl/Cmd+Z invokes desktop undo unless text input, textarea, contenteditable or dialog has focus; Shift/Alt variants ignored; refreshes listing after undo | desktop/DesktopUndoShortcut.svelte:5 | ✓ | workspace.rs:185; workspace.rs:1218; ui/shortcuts.rs:45 | Native undo action and shortcut present. |

## Missing or partial, ranked

1. **System file chooser and its request modes** (rows 15–20, ✗): open/save/save-files, filter/hidden-file handling, remote download semantics and location selection have no GPUI portal implementation; this is a core desktop integration surface.
2. **Saved servers and pairing** (rows 26–28, 51, ✗): the Svelte UI manages remote locations, reconnection and device credentials; GPUI currently has no equivalent server/pairing pages.
3. **Image zoom/rotation and PDF preview parity** (rows 38, 42, ◐): native quick-look omits image zoom and PDF rendering is limited to thumbnail/first page.
4. **Transfer job detail** (row 30, ◐): native jobs lack the web panel's per-item progress, cancellation and dismissal detail.
5. **Preferences and settings parity** (rows 46–49, ◐): settings exist, but several browse and appearance controls remain absent or not proven in the native renderer.
6. **Status bar and responsive toolbar behavior** (rows 1, 3–6, ◐): core native chrome exists, but selection-size wording, path suggestions, narrow sidebar and AddMenu/search parity need follow-up.
