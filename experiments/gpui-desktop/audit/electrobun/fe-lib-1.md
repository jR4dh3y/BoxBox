# Electrobun frontend lib, part 1

## Coverage

| File | Lines | Read |
|---|---:|---|
| `/home/radhey/code/BoxBox/frontend/src/lib/api/auth.ts` | 85 | 1–85 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/client.ts` | 343 | 1–343 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/connect.ts` | 92 | 1–92 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/desktop.ts` | 29 | 1–29 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/desktopApps.ts` | 66 | 1–66 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/desktopClipboard.ts` | 62 | 1–62 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/desktopPreferences.ts` | 155 | 1–155 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/desktopServers.ts` | 67 | 1–67 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/desktopTrash.ts` | 51 | 1–51 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/desktopVolumes.ts` | 89 | 1–89 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/directory.ts` | 41 | 1–41 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/drive-names.ts` | 40 | 1–40 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/earlyStart.ts` | 97 | 1–97 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/files.ts` | 396 | 1–396 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/index.ts` | 90 | 1–90 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/jobs.ts` | 225 | 1–225 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/parse.ts` | 48 | 1–48 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/remotePaths.ts` | 145 | 1–145 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/shareManagement.ts` | 126 | 1–126 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/shares.ts` | 209 | 1–209 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/system.ts` | 47 | 1–47 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/wallpaper.ts` | 49 | 1–49 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/wallpaperImage.ts` | 40 | 1–40 |
| `/home/radhey/code/BoxBox/frontend/src/lib/api/websocket.ts` | 55 | 1–55 |
| `/home/radhey/code/BoxBox/frontend/src/lib/config.ts` | 67 | 1–67 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/chooser.svelte.ts` | 309 | 1–309 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/chooserServers.svelte.ts` | 25 | 1–25 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/connectionView.ts` | 66 | 1–66 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/deploymentIcon.ts` | 14 | 1–14 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/jobEvents.ts` | 257 | 1–257 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/openWith.ts` | 51 | 1–51 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/remote.svelte.ts` | 106 | 1–106 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/servers.svelte.ts` | 151 | 1–151 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/trash.svelte.ts` | 62 | 1–62 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/volumeCatalog.ts` | 24 | 1–24 |
| `/home/radhey/code/BoxBox/frontend/src/lib/desktop/volumes.svelte.ts` | 72 | 1–72 |
| `/home/radhey/code/BoxBox/frontend/src/lib/index.ts` | 15 | 1–15 |
| `/home/radhey/code/BoxBox/frontend/src/lib/settings/options.ts` | 26 | 1–26 |
| `/home/radhey/code/BoxBox/frontend/src/lib/settings/pairing.svelte.ts` | 128 | 1–128 |
| `/home/radhey/code/BoxBox/frontend/src/lib/settings/preferences.ts` | 163 | 1–163 |

## Behaviours

| # | Behaviour | Source | GPUI | GPUI evidence | Notes |
|---:|---|---|:---:|---|---|
| 1 | Username/password login, access token storage, refresh and logout; refresh is single-flight, serialized against logout, retries once on refresh-token rotation conflict, and a terminal 401 closes the session. | `auth.ts:26`; `client.ts:74–192` | — | none found | Authentication belongs to web/server API; native app uses local files and optional remote connection credentials separately. |
| 2 | Authenticated requests attach bearer tokens, keep same-origin credentials, serialize JSON and blobs, build query parameters, accept 204, and surface typed HTTP/JSON errors; retry once after 401 refresh. | `client.ts:207–343` | — | none found | Generic web API transport; no matching browser API client is part of the native UI. |
| 3 | Pairing code generation requires password confirmation, validates format and expiry, lists paired clients/status, and revokes a client; unsupported endpoints show a server-update message. | `connect.ts:10–92`; `pairing.svelte.ts:31–128` | ✗ | none found | Pairing is explicitly later work in `UI_MAP.md` Still to do. |
| 4 | Detect local desktop and distinguish local from remote paths; local desktop open-with, clipboard and server-transfer actions reject unsupported remote/local mixtures. | `desktop.ts:5–29`; `remotePaths.ts:10–145`; `desktopClipboard.ts:35–62` | ◐ | `api/http.rs:1–300`; `os_clipboard.rs:1`; `workspace/operations.rs:616–720` | Remote HTTP and local providers exist, but saved-server management is partial; clipboard and cross-provider operations do not establish complete parity. |
| 5 | List installed apps by MIME association, search recommended and other apps, choose default/first recommended, launch, set the default handler, and open a terminal at a path. | `desktopApps.ts:8–66`; `openWith.ts:7–51` | ◐ | `open_with.rs:325–349`; `ui/open_with.rs:1–175`; `workspace/open_with.rs:23–91` | Native dialog, filtering, app launch and default association are implemented. GPUI source has no equivalent for the desktop API's open-terminal action. |
| 6 | System clipboard can hold copy or cut of one or more local paths; empty selections and remote items error; a successful cut move clears only the unchanged cut selection. | `desktopClipboard.ts:8–62` | ◐ | `os_clipboard.rs:1`; `workspace/operations.rs:720–` | OS clipboard integration is present; UI_MAP records clipboard support but the cut-clear detail is not independently confirmed here. |
| 7 | Persist desktop preferences with typed/defaulted values: hidden files, extensions, delete confirmation, sort/view defaults, appearance, acceleration, single-click preview, selection checkboxes, compact mode, drive names and favorites. | `desktopPreferences.ts:8–62`; `preferences.ts:45–163` | ◐ | `workspace.rs:52–80`; `ui/settings.rs:100–135`; `settings_state.rs:1` | General settings confirm hidden files, extensions, delete confirmation, preview, checkboxes, sort/view; appearance and favorites exist. There is no native acceleration/compact preference or stored drive-name override UI; drive Rename/Reset Name is listed as still to do. |
| 8 | Load/save desktop settings; migrate legacy inline/local wallpaper into desktop storage, validate image hash, sequence saves, flush, and show load/save failure notices. | `desktopPreferences.ts:64–155` | ◐ | `workspace/appearance.rs:69–129`; `ui/settings.rs:330–390` | Wallpaper and appearance exist; UI_MAP says wallpaper picker chooser remains untested. Settings storage and migration behavior differ from web persistence. |
| 9 | Saved remote server create/edit/pair/remove; validate server metadata and HTTPS URL; probe connection state, show expiry/revocation/offline state, expose roots and icons, and isolate failed remote sources. | `desktopServers.ts:8–67`; `servers.svelte.ts:20–151`; `connectionView.ts:8–66`; `chooserServers.svelte.ts:1–25` | ◐ | `ui/settings.rs:1280–1318`; `api/http.rs:1`; `settings_state.rs:1` | Native has connection settings and remote HTTP; full Servers page/catalog and pairing are explicitly later work. Marked partial/known. |
| 10 | Show user Trash entries, deletion time, original path, directory and restorable status; restore without overwriting; permanently delete selected entries only after a second click. | `desktopTrash.ts:6–51`; `trash.svelte.ts:11–62` | ✓ | `local/trash.rs:34–170`; `ui/browser.rs:1`; `workspace/operations.rs:762–817` | Native Trash uses freedesktop Trash and has restore/permanent delete; its confirm interaction is not necessarily identical to the web controller’s second-click rule. |
| 11 | List mounted/mountable volumes with capabilities and capacity; mount/unmount/eject and refresh snapshot; report per-action success/failure; combine mounted volume drives into This PC. | `desktopVolumes.ts:8–89`; `volumes.svelte.ts:11–72`; `volumeCatalog.ts:1–24` | ◐ | `devices.rs:1`; `workspace/devices.rs:40–92` | UDisks2 mount/eject exists. Phone/network GVfs mounts and LUKS unlock are explicitly known gaps; local capacity is partial per Still to do. |
| 12 | Directory responses are validated at the boundary; entry names reject empty, dot, dot-dot, slash and NUL; child paths derive from the returned parent path. | `directory.ts:4–41`; `parse.ts:1–48` | ✓ | `local.rs:1`; `api.rs:1` | Native has direct filesystem providers and path validation; response parsing itself is web transport detail. |
| 13 | Custom drive names can be listed, assigned and removed per mount point. | `drive-names.ts:5–40` | ✗ | none found | Stored drive names and Rename/Reset Name in drive menu are listed in `UI_MAP.md` Still to do; known gap. |
| 14 | Early page shell starts auth refresh and directory fetch before app JS; app claims each once, avoids duplicate token rotation, rejects definite 401, and drops mismatched/stale (>30s) or failed listing. | `earlyStart.ts:1–97`; `config.ts:24–31` | — | none found | Web page-load optimization and browser shell handoff, not native-app behavior. |
| 15 | List roots and drive statistics; browse directories; inspect/create/rename/move/edit/delete files; recursively confirm delete via `confirm=true`; search names; create thumbnail, preview, download and folder-archive URLs. | `files.ts:15–396` | ◐ | `api.rs:1`; `api/http.rs:1`; `workspace/operations.rs:1` | Local browse and core file operations exist. Web URL streaming/archives are not equivalent to native preview/open; UI_MAP still identifies several UI gaps (load retry, type-ahead, etc.). |
| 16 | Search results, paths, roots, stats and jobs preserve source identity across local/remote servers; reject malformed remote paths, cross-source rename, remote root browsing without choosing a drive, and cross-server move. | `remotePaths.ts:10–145`; `files.ts:106–258`; `jobs.ts:104–160` | ◐ | `api/http.rs:1`; `workspace.rs:428–` | Remote browsing is available through connections, but saved server lifecycle and some cross-provider behaviors remain incomplete; remote management is marked later/partial. |
| 17 | File content preview fetch reports HTTP status text on failure; local files can expose token-bearing stream URLs; remote paths do not include local token query parameters. | `files.ts:274–396` | ◐ | `media.rs:1`; `workspace/preview.rs:1`; `workspace/operations.rs:1431–1508` | GPUI has text/image preview paths but no same URL/browser media transport; preview stepping and image zoom are known gaps. |
| 18 | Job model has copy/move/delete lifecycle, progress, optional byte total, error/timestamps; create, cancel, classify terminal/active; cross-server copy uses desktop transfer and cross-server move is rejected with guidance. | `jobs.ts:13–225` | ◐ | `progress.rs:1`; `workspace/jobs.rs:1`; `workspace/operations.rs:616–720`; `ui/jobs.rs:1` | Native runs in-process jobs with progress/cancellation; UI_MAP describes transfer status as coarse/partial. |
| 19 | Malformed JSON, unknown WebSocket event, invalid payload/state/progress (non-integer or outside 0–100) are ignored; job updates, completion, server error and pong are recognized. | `websocket.ts:1–55` | — | none found | WebSocket protocol parser is not used by the native local implementation. |
| 20 | Job events use SSE snapshots and per-job events, reconnect with exponential delay capped at 30s, fall back to active-job polling every 2s when unavailable, stop polling hidden documents, block unauthorized sources, retry on focus/online, and reconcile each remote independently. | `jobEvents.ts:10–257`; `config.ts:34–45` | ◐ | `workspace/jobs.rs:1`; `progress.rs:1` | Native job execution is local; remote/server SSE recovery contract has no matching native service-event stream confirmed. |
| 21 | Create/list/revoke owner share links; specify recipient permissions, upload byte cap and expiry; remote-created link URLs use paired server address. | `shareManagement.ts:1–126`; `shares.ts:10–76` | ✓ | `api/http.rs:236–287`; `ui/settings.rs:313–` | Native share-link management is present in settings; recipient web pages are outside native scope. |
| 22 | Public share recipient operations expose metadata, folder listing, upload/delete/create-folder, download/archive/preview/wallpaper URLs, and detect valid expiry. | `shares.ts:78–209` | — | none found | Public share page and upload panel are web-only per UI_MAP. |
| 23 | Read system drive and upload configuration from backend; upload chunk size is operator-configured. | `system.ts:8–47`; `config.ts:11–18` | — | none found | Server upload API is web/backend-specific; native transfers use filesystem operations. |
| 24 | Shared/server wallpaper metadata can be fetched, image downloaded/uploaded, display mode updated, and wallpaper deleted; 404 means no published wallpaper, invalid metadata/image errors. | `wallpaper.ts:1–49`; `wallpaperImage.ts:1–40` | — | none found | These are server-published wallpaper/share APIs; native appearance uses local wallpaper persistence and is a distinct implementation. |
| 25 | Query freshness/prefetch, concurrency, upload chunking and limits, spinner delay (150ms), search debounce (300ms), websocket reconnect/ping and jobs refetch intervals are configured centrally. | `config.ts:1–67` | ◐ | `workspace.rs:479`; `ui/settings.rs:1` | Exact web timings and upload controls are not evidenced as native parity. Native currently has no spinner per UI_MAP; mark known where UI timing matters. |
| 26 | Portal chooser parses open/save/save-files requests, title/button/filter/choices/default folder/name/multiple/directory; starts at request folder or first root and loads saved server locations after initial local paint. | `chooser.svelte.ts:1–179`; `chooserServers.svelte.ts:1–25` | ✗ | none found | System picker through xdg portal is explicitly Still to do; known gap. |
| 27 | Chooser filters by case-insensitive name query, hidden-file toggle, directory/file mode, MIME exact/wildcard and shell glob; sorts by configured field/direction. | `chooser.svelte.ts:89–122`; `chooser.svelte.ts:285–309` | ✗ | none found | No native portal chooser, so these picker-specific controls are absent. |
| 28 | Chooser navigation supports parent, local roots, remote root locations and volume mounting; remote selection is limited to open-file mode (not directories/save); unavailable mounted path shows an explicit error. | `chooser.svelte.ts:124–210` | ✗ | none found | Whole chooser surface is absent, even though ordinary native browsing and UDisks volume actions exist. |
| 29 | Chooser selection respects single/multiple and directory eligibility; save mode copies selected filename; validates save name; save-files returns current folder; open accepts selected path or current folder in directory mode. | `chooser.svelte.ts:212–257` | ✗ | none found | Portal chooser is known missing. |
| 30 | Chooser handles overwrite conflict with “Choose Replace” state bound to unchanged target path; cancel sends cancellation request; loading requests abort on navigation/dispose and stale responses are discarded. | `chooser.svelte.ts:175–284` | ✗ | none found | Portal chooser and overwrite recovery are known missing. |
| 31 | Chooser server/roots failures keep local picker usable, show separate server-location error, load each server’s roots independently, and ignore aborted/stale responses. | `chooser.svelte.ts:151–172`; `chooserServers.svelte.ts:1–25`; `remote.svelte.ts:1–56` | ✗ | none found | No native chooser/server catalog; remote settings support does not supply picker locations. |
| 32 | Remote snapshot panels retain last confirmed value on refresh failure, show refresh error beside it, reject stale results; one keyed mutation runs at a time and reports operation-specific failure. | `remote.svelte.ts:1–106` | ◐ | `ui/settings.rs:1`; `api/http.rs:1` | Similar async settings state exists, but no proof of the same stale-result and retained-snapshot contract across native panels. |
| 33 | Trash selection permits restore only when restorable; permanent deletion requires a second click; successful mutation reloads current list. | `trash.svelte.ts:11–62` | ◐ | `ui/browser.rs:1`; `local/trash.rs:132–170` | Restore/delete and list refresh exist; exact second-click confirmation is not established by inspected native evidence. |
| 34 | Volume actions are serialized against refresh/action busy state, report Mounted/Unmounted/Ejected success, retain server snapshot, and expose newest action/refresh error. | `volumes.svelte.ts:11–72` | ◐ | `workspace/devices.rs:40–92`; `ui/sidebar.rs:1` | Native mount/eject exists, but exact status/error and internal/network/LUKS coverage is partial. |
| 35 | Open With search matches app name or desktop ID across recommended and other groups; default handler or first recommended is initial choice; filtered-out app cannot be launched; confirmed default stays visible after launch failure. | `openWith.ts:7–51` | ✓ | `open_with.rs:325–349`; `workspace/open_with.rs:23–91`; `ui/open_with.rs:62–175` | Core listed behavior is present, including default selection/association. The separate open-terminal operation is the partial gap in #5. |
| 36 | Settings exposes allowed sort/view options and narrows values to declared choices without accepting arbitrary strings. | `options.ts:1–26` | ✓ | `sort.rs:1`; `ui/settings.rs:115–135` | Native sort/view settings are implemented. |
| 37 | Device pairing settings poll paired clients/status every 3 seconds only while visible; generate and copy code; revoke client; show errors and close a code when server status/expiry changes; abort poll and clear password on dispose. | `pairing.svelte.ts:1–128` | ✗ | none found | Pairing is listed as later work in UI_MAP; known gap. |
| 38 | Preference changes apply immediately; wallpaper operations are latest-wins/cancellable, local desktop wallpaper migrates to server storage, browser inline data is persisted locally, old local images are cleaned up; publish changes to share pages and report partial publication failure. | `preferences.ts:25–163` | ◐ | `workspace/appearance.rs:69–129`; `settings_state.rs:1` | Native preferences and local appearance exist, but web share-page publication/localStorage wallpaper migration are not the native contract. |

## Missing or partial, ranked

1. **✗ Portal file chooser and its complete open/save behavior** (#26–31): the xdg portal picker is explicitly missing; system applications cannot use BoxBox’s custom chooser.
2. **✗ Pairing flow** (#3, #37): users cannot pair/revoke devices from the native app; this blocks onboarding remote servers.
3. **◐ Saved remote servers** (#9, #16): remote browsing exists, while full server lifecycle/catalog and pairing remain incomplete.
4. **◐ Native desktop action coverage** (#5): Open With and default-handler association are implemented, while the separate open-terminal action is not evidenced.
5. **◐ Removable and external volumes** (#11, #34): mount/eject works for supported UDisks devices; MTP, network mounts and encrypted-volume unlock remain missing.
6. **✗ Custom drive names** (#13): Rename/Reset Name and persisted drive labels are listed as still to do.
7. **◐ Desktop preference parity** (#7–8, #38): base settings and wallpaper exist, while several display preferences and web wallpaper publication/migration behavior lack native equivalents.
8. **◐ Job progress and event recovery** (#18–20): local in-process jobs exist, but progress is coarse and server SSE recovery behavior is not present as native parity.
9. **◐ Stream/media behavior** (#15, #17): native preview is intentionally limited/read-only; image zoom and preview stepping are known gaps, and browser URL streaming is not equivalent.
