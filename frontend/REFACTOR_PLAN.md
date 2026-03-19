# Frontend Refactoring Plan

This document outlines all issues found during a code audit and provides actionable steps to refactor the frontend to follow professional development practices.

## Project Context

- **Framework:** SvelteKit with Svelte 5
- **Styling:** Tailwind CSS v4 (installed but barely used)
- **State:** Mix of Svelte 5 runes and legacy Svelte 4 stores
- **UI Icons:** lucide-svelte

---

## Issue Categories

1. [Styling Inconsistencies](#1-styling-inconsistencies)
2. [Code Duplication](#2-code-duplication)
3. [Dead/Unused Code](#3-deadunused-code)
4. [Type Issues](#4-type-issues)
5. [Architecture Issues](#5-architecture-issues)
6. [Accessibility Issues](#6-accessibility-issues)

---

## 1. Styling Inconsistencies

### 1.1 Remove All `<style>` Blocks — Convert to Tailwind

**Problem:** Almost every component uses scoped `<style>` blocks with raw CSS instead of Tailwind classes, despite Tailwind v4 being installed.

**Affected Files:**

- `src/lib/components/Sidebar.svelte` (~100 lines CSS)
- `src/lib/components/FileList.svelte` (~180 lines CSS)
- `src/lib/components/Toolbar.svelte` (~100 lines CSS)
- `src/lib/components/DriveCard.svelte` (~80 lines CSS)
- `src/lib/components/FilePreview.svelte` (~100 lines CSS)
- `src/lib/components/StatusBar.svelte` (~60 lines CSS)
- `src/lib/components/FileBrowser.svelte` (~120 lines CSS)
- `src/lib/components/JobMonitor.svelte` (~180 lines CSS)
- `src/lib/components/UploadDropzone.svelte` (~100 lines CSS)
- `src/lib/components/UploadProgress.svelte` (~120 lines CSS)
- `src/lib/components/preview/CodePreview.svelte` (~50 lines CSS)
- `src/lib/components/preview/ImagePreview.svelte` (~60 lines CSS)
- `src/lib/components/preview/AudioPreview.svelte` (~40 lines CSS)
- `src/lib/components/preview/VideoPreview.svelte` (~30 lines CSS)
- `src/lib/components/preview/PdfPreview.svelte` (~25 lines CSS)
- `src/routes/+layout.svelte` (~80 lines CSS)
- `src/routes/+page.svelte` (~40 lines CSS)
- `src/routes/login/+page.svelte` (~120 lines CSS)
- `src/routes/settings/+page.svelte` (~200 lines CSS)
- `src/routes/browse/+page.svelte` (~50 lines CSS)

**Action:**

1. Convert all CSS to Tailwind utility classes
2. For complex/repeated patterns, use Tailwind's `@apply` in `layout.css` sparingly
3. Remove all `<style>` blocks from components

**Example Conversion:**

```svelte
<!-- BEFORE -->
<button class="nav-btn">Click</button>

<!-- AFTER -->
<button
	class="flex h-7 w-7 cursor-pointer items-center justify-center rounded border-none bg-transparent text-gray-500 transition-all duration-100 hover:enabled:bg-gray-700 hover:enabled:text-gray-300"
>
	Click
</button>

<style>
	.nav-btn {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: none;
		border-radius: 4px;
		color: #888;
		cursor: pointer;
		transition: all 0.1s ease;
	}
	.nav-btn:hover:not(:disabled) {
		background: #333;
		color: #ccc;
	}
</style>
```

### 1.2 Inconsistent Theme Approach

**Problem:** Three different theming strategies used simultaneously:

- Hardcoded dark colors (`#1e1e1e`, `#252525`)
- `@media (prefers-color-scheme: dark)` queries
- Light theme only (Breadcrumb, SearchBar)

**Action:**

1. Decide on ONE approach: dark mode only OR system preference
2. Use Tailwind's dark mode feature: `dark:` prefix
3. Configure in `tailwind.config.js`:

```js
export default {
	darkMode: 'class' // or 'media' for system preference
};
```

### 1.3 Create Design Tokens

**Problem:** Hardcoded colors with slight variations everywhere:

```css
/* Backgrounds: */ #1e1e1e, #1a1a1a, #141414, #111827, #252525
/* Borders: */ #2a2a2a, #333, #374151
/* Text: */ #ccc, #888, #666, #555, #aaa, #e0e0e0
```

**Action:**

1. Define custom colors in `layout.css` using Tailwind v4 syntax:

```css
@import 'tailwindcss';

@theme {
	--color-surface-primary: #1e1e1e;
	--color-surface-secondary: #252525;
	--color-surface-tertiary: #2a2a2a;
	--color-border-primary: #333;
	--color-border-secondary: #2a2a2a;
	--color-text-primary: #e0e0e0;
	--color-text-secondary: #888;
	--color-text-muted: #555;
	--color-accent: #4a9eff;
	--color-accent-hover: #345580;
	--color-danger: #dc3545;
	--color-success: #10b981;
	--color-warning: #f39c12;
}
```

### 1.4 Duplicated Spinner Animation

**Problem:** `@keyframes spin` defined in 7 different files.

**Affected Files:**

- `FileList.svelte`
- `+layout.svelte`
- `+page.svelte`
- `login/+page.svelte`

**Action:**

1. Remove all `@keyframes spin` from components
2. Use Tailwind's built-in `animate-spin` class instead:

```svelte
<div class="h-6 w-6 animate-spin rounded-full border-2 border-gray-700 border-t-blue-500"></div>
```

### 1.5 Inconsistent Button Styling

**Problem:** Every component defines its own button styles:

- `.nav-btn`, `.action-btn`, `.header-btn`, `.control-btn`, `.submit-btn`, `.logout-btn`, `.back-btn`

**Action:**

1. Create reusable button component at `src/lib/components/ui/Button.svelte`:

```svelte
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md' | 'lg' | 'icon';
		disabled?: boolean;
		children: Snippet;
		onclick?: () => void;
	}

	let { variant = 'primary', size = 'md', disabled = false, children, onclick }: Props = $props();

	const baseClasses =
		'inline-flex items-center justify-center font-medium rounded transition-all duration-150 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed';

	const variantClasses = {
		primary: 'bg-accent text-white hover:bg-accent-hover',
		secondary:
			'bg-surface-secondary border border-border-primary text-text-secondary hover:bg-surface-tertiary hover:text-text-primary',
		ghost: 'bg-transparent text-text-secondary hover:bg-surface-secondary hover:text-text-primary',
		danger: 'bg-danger text-white hover:bg-red-700'
	};

	const sizeClasses = {
		sm: 'px-2 py-1 text-xs gap-1',
		md: 'px-4 py-2 text-sm gap-2',
		lg: 'px-6 py-3 text-base gap-2',
		icon: 'w-7 h-7 p-0'
	};
</script>

<button
	type="button"
	class="{baseClasses} {variantClasses[variant]} {sizeClasses[size]}"
	{disabled}
	{onclick}
>
	{@render children()}
</button>
```

---

## 2. Code Duplication

### 2.1 File Type Detection Duplicated in 3 Places

**Problem:** Extension arrays and file type logic duplicated:

**Location 1:** `src/lib/utils/fileTypes.ts`

```typescript
const VIDEO_EXTENSIONS = ['mp4', 'webm', 'mkv', ...];
const IMAGE_EXTENSIONS = ['jpg', 'jpeg', 'png', ...];
```

**Location 2:** `src/lib/utils/format.ts`

```typescript
export function getFileTypeDescription(filename: string): string {
	// Has its own extension map
}
```

**Location 3:** `src/lib/components/FileList.svelte`

```typescript
function getFileIcon(item: FileInfo) {
  const imageExts = ['jpg', 'jpeg', 'png', ...]; // Duplicated!
  const videoExts = ['mp4', 'mkv', ...]; // Duplicated!
}
```

**Action:**

1. Consolidate ALL file type logic into `src/lib/utils/fileTypes.ts`
2. Export a single source of truth:

```typescript
// fileTypes.ts - SINGLE SOURCE OF TRUTH

export const FILE_EXTENSIONS = {
  video: ['mp4', 'webm', 'mkv', 'avi', 'mov', 'wmv', 'flv', 'm4v', 'ogv'],
  audio: ['mp3', 'wav', 'flac', 'aac', 'ogg', 'm4a', 'wma', 'opus'],
  image: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'bmp', 'ico', 'avif'],
  pdf: ['pdf'],
  code: ['js', 'ts', 'jsx', 'tsx', 'py', 'go', 'rs', 'java', ...],
  text: ['txt', 'log', 'csv', 'tsv', 'rtf'],
  archive: ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz'],
  spreadsheet: ['xls', 'xlsx', 'csv', 'ods'],
  document: ['pdf', 'doc', 'docx', 'txt', 'md', 'rtf', 'odt'],
  web: ['html', 'htm', 'xml'],
  style: ['css', 'scss', 'sass', 'less'],
  data: ['json', 'yaml', 'yml', 'toml']
} as const;

export function getExtension(filename: string): string { ... }
export function getFileCategory(filename: string): keyof typeof FILE_EXTENSIONS | 'unknown' { ... }
export function getPreviewType(filename: string): PreviewType { ... }
export function getFileTypeDescription(filename: string): string { ... }
export function getMonacoLanguage(filename: string): string { ... }
export function getFileIcon(filename: string, isDir: boolean): ComponentType { ... }
```

3. Remove `getFileTypeDescription` from `format.ts`
4. Remove `getFileIcon` logic from `FileList.svelte` — import from `fileTypes.ts`

### 2.2 `formatSize` Duplicated

**Problem:** Same function exists twice with different names:

- `src/lib/utils/format.ts` → `formatFileSize()`
- `src/lib/utils/fileTypes.ts` → `formatSize()`

**Action:**

1. Delete `formatSize` from `fileTypes.ts`
2. Use `formatFileSize` from `format.ts` everywhere

### 2.3 Duplicated API Export Patterns

**Problem:** API modules export both individual functions AND an object:

```typescript
// files.ts
export async function listRoots() { ... }
export const filesApi = { listRoots, ... }; // Redundant
```

**Action:**

1. Pick ONE pattern — recommend individual named exports only
2. Remove all `*Api` objects from:
   - `src/lib/api/files.ts`
   - `src/lib/api/jobs.ts`
   - `src/lib/api/auth.ts`

### 2.4 Type Duplication

**Problem:** `SortField` and `SortDir` defined in `types/files.ts` but also inline in stores.

**Location 1:** `src/lib/types/files.ts`

```typescript
export type SortField = 'name' | 'size' | 'modTime' | 'type';
export type SortDir = 'asc' | 'desc';
```

**Location 2:** `src/lib/stores/files.ts`

```typescript
sortBy: 'name' | 'size' | 'modTime' | 'type'; // Duplicated inline
sortDir: 'asc' | 'desc'; // Duplicated inline
```

**Action:**

1. Import types from `types/files.ts` in stores
2. Use the imported types instead of inline definitions

---

## 3. Dead/Unused Code

### 3.1 Unused Component

**Problem:** `FileBrowser.svelte` exists but is never used — `browse/+page.svelte` builds its own layout.

**Action:**

1. Either delete `FileBrowser.svelte` OR refactor `browse/+page.svelte` to use it
2. Recommended: Use `FileBrowser.svelte` as the main container, move logic there

### 3.2 Unused Imports

**Problem:** Components import things they don't use.

**File:** `src/lib/components/Sidebar.svelte`

```typescript
import { Folder } from 'lucide-svelte'; // Never used
```

**Action:**

1. Run `eslint` with `no-unused-vars` rule
2. Remove all unused imports

### 3.3 Unnecessary Files

**Problem:** `.gitkeep` files in non-empty directories.

**Files to delete:**

- `src/lib/components/.gitkeep`
- `src/lib/stores/.gitkeep`
- `src/lib/utils/.gitkeep`

### 3.4 Empty Index File

**Problem:** `src/lib/index.ts` is empty.

**Action:**

1. Either populate it with re-exports OR delete it

---

## 4. Type Issues

### 4.1 Migrate Stores to Svelte 5 Runes

**Problem:** Components use Svelte 5 runes (`$state`, `$derived`, `$props`) but stores use legacy Svelte 4 pattern.

**Affected Files:**

- `src/lib/stores/auth.ts`
- `src/lib/stores/files.ts`
- `src/lib/stores/jobs.ts`
- `src/lib/stores/settings.ts`
- `src/lib/stores/websocket.ts`

**Action:**

1. Convert stores to use Svelte 5's `$state` runes with `.svelte.ts` extension
2. Example migration:

```typescript
// BEFORE: auth.ts (Svelte 4)
import { writable, derived } from 'svelte/store';

const { subscribe, set, update } = writable<AuthState>(initialState);

export const authStore = { subscribe, login, logout, ... };
export const isAuthenticated = derived(authStore, $auth => $auth.isAuthenticated);

// AFTER: auth.svelte.ts (Svelte 5)
let state = $state<AuthState>(initialState);

export const authStore = {
  get isAuthenticated() { return state.isAuthenticated; },
  get isLoading() { return state.isLoading; },
  get error() { return state.error; },

  async login(username: string, password: string) {
    state.isLoading = true;
    // ...
  },

  logout() {
    state = initialState;
  }
};
```

---

## 5. Architecture Issues

### 5.1 Centralize localStorage Access

**Problem:** Multiple files access `localStorage` directly:

- `src/lib/api/client.ts` — tokens
- `src/lib/stores/settings.ts` — settings
- `src/lib/api/files.ts` — reads token in `getPreviewUrl()`

**Action:**

1. Create `src/lib/utils/storage.ts`:

```typescript
const KEYS = {
	ACCESS_TOKEN: 'accessToken',
	REFRESH_TOKEN: 'refreshToken',
	SETTINGS: 'filemanager_settings'
} as const;

export const storage = {
	get<T>(key: string): T | null {
		if (typeof window === 'undefined') return null;
		try {
			const item = localStorage.getItem(key);
			return item ? JSON.parse(item) : null;
		} catch {
			return null;
		}
	},

	set<T>(key: string, value: T): void {
		if (typeof window === 'undefined') return;
		localStorage.setItem(key, JSON.stringify(value));
	},

	remove(key: string): void {
		if (typeof window === 'undefined') return;
		localStorage.removeItem(key);
	},

	// Typed accessors
	getAccessToken: () => storage.get<string>(KEYS.ACCESS_TOKEN),
	setAccessToken: (token: string) => storage.set(KEYS.ACCESS_TOKEN, token)
	// ... etc
};
```

### 5.2 Centralize Configuration

**Problem:** Magic numbers scattered across files:

```typescript
const REFRESH_INTERVAL_MS = 14 * 60 * 1000; // auth.ts
const DEFAULT_CHUNK_SIZE = 10 * 1024 * 1024; // upload.ts
staleTime: 1000 * 60, // +layout.svelte
refetchInterval: 5000 // jobs.ts
```

**Action:**

1. Create `src/lib/config.ts`:

```typescript
export const CONFIG = {
	auth: {
		tokenRefreshIntervalMs: 14 * 60 * 1000, // 14 minutes
		accessTokenExpiryMs: 15 * 60 * 1000 // 15 minutes
	},
	upload: {
		defaultChunkSize: 10 * 1024 * 1024, // 10MB
		maxConcurrentUploads: 3
	},
	query: {
		staleTimeMs: 60 * 1000, // 1 minute
		jobsRefetchIntervalMs: 5000 // 5 seconds
	},
	websocket: {
		pingIntervalMs: 30 * 1000,
		maxReconnectAttempts: 10,
		initialReconnectDelayMs: 1000,
		maxReconnectDelayMs: 30 * 1000
	}
} as const;
```

### 5.3 Add Error Boundary

**Problem:** No error handling at layout level — component errors crash the app.

**Action:**

1. Create `src/lib/components/ErrorBoundary.svelte`:

```svelte
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
		fallback?: Snippet<[Error]>;
	}

	let { children, fallback }: Props = $props();
	let error = $state<Error | null>(null);

	// Svelte 5 doesn't have built-in error boundaries yet
	// This is a placeholder for when it's supported
</script>

{#if error && fallback}
	{@render fallback(error)}
{:else}
	{@render children()}
{/if}
```

### 5.4 Create UI Component Library

**Problem:** No reusable base components.

**Action:**

1. Create `src/lib/components/ui/` directory with:

```
ui/
├── Button.svelte
├── Input.svelte
├── Select.svelte
├── Toggle.svelte
├── Card.svelte
├── Modal.svelte
├── Spinner.svelte
├── Badge.svelte
├── ProgressBar.svelte
└── index.ts
```

2. Export all from `index.ts`:

```typescript
export { default as Button } from './Button.svelte';
export { default as Input } from './Input.svelte';
// ...
```

---

## 6. Accessibility Issues

### 6.1 Missing Keyboard Support

**Problem:** `FileList.svelte` uses `ondblclick` without keyboard equivalent.

**Action:**

```svelte
<!-- Add keyboard handler -->
<tr
  onclick={(e) => handleRowClick(item, e)}
  ondblclick={() => handleDoubleClick(item)}
  onkeydown={(e) => {
    if (e.key === 'Enter') handleDoubleClick(item);
    if (e.key === ' ') { e.preventDefault(); handleRowClick(item, e); }
  }}
  tabindex="0"
  role="row"
>
```

### 6.2 Color Contrast Issues

**Problem:** `#555` text on `#1e1e1e` background fails WCAG AA.

**Action:**

1. Use minimum `#888` for secondary text
2. Use `#aaa` or lighter for readable text
3. Test with browser accessibility tools

### 6.3 Missing ARIA Labels

**Problem:** Some interactive elements lack proper labels.

**Action:**

1. Audit all buttons, inputs, and interactive elements
2. Add `aria-label` where text content is not descriptive

---

## Refactoring Order

Execute in this order to minimize conflicts:

### Phase 1: Foundation (Do First)

1. Create `src/lib/config.ts` with centralized constants
2. Create `src/lib/utils/storage.ts` for localStorage
3. Consolidate `fileTypes.ts` — single source of truth
4. Remove duplicate `formatSize` from `fileTypes.ts`
5. Delete unused files (`.gitkeep`, empty `index.ts`)

### Phase 2: Design System

1. Add design tokens to `layout.css`
2. Create `src/lib/components/ui/` base components
3. Create `Button.svelte`, `Spinner.svelte`, `Card.svelte`

### Phase 3: Component Migration (One at a Time)

1. Start with smallest components:
   - `StatusBar.svelte`
   - `PdfPreview.svelte`
   - `AudioPreview.svelte`
   - `VideoPreview.svelte`
2. Then medium components:
   - `Toolbar.svelte`
   - `DriveCard.svelte`
   - `ImagePreview.svelte`
   - `CodePreview.svelte`
3. Then large components:
   - `Sidebar.svelte`
   - `FileList.svelte`
   - `FilePreview.svelte`
4. Finally routes:
   - `login/+page.svelte`
   - `settings/+page.svelte`
   - `browse/+page.svelte`
   - `+layout.svelte`

### Phase 4: Store Migration

1. Convert stores to Svelte 5 runes (`.svelte.ts`)
2. Update all imports

### Phase 5: Cleanup

1. Remove unused imports
2. Remove dead components (`FileBrowser.svelte` if not used)
3. Remove redundant API object exports
4. Final lint and type check

---

## Validation Checklist

After refactoring, verify:

- [ ] `npm run check` passes (no TypeScript errors)
- [ ] `npm run lint` passes (no ESLint errors)
- [ ] `npm run build` succeeds
- [ ] No `<style>` blocks remain in components (except `layout.css`)
- [ ] All colors use design tokens
- [ ] All buttons use `Button.svelte` component
- [ ] All spinners use `animate-spin` class
- [ ] File type logic only in `fileTypes.ts`
- [ ] No direct `localStorage` access outside `storage.ts`
- [ ] All magic numbers in `config.ts`
- [ ] Keyboard navigation works for all interactive elements
- [ ] Color contrast passes WCAG AA

---

## Files to Create

```
src/lib/
├── config.ts                    # Centralized configuration
├── components/
│   └── ui/
│       ├── Button.svelte        # Reusable button
│       ├── Input.svelte         # Reusable input
│       ├── Select.svelte        # Reusable select
│       ├── Toggle.svelte        # Reusable toggle switch
│       ├── Card.svelte          # Reusable card container
│       ├── Modal.svelte         # Reusable modal
│       ├── Spinner.svelte       # Reusable spinner
│       ├── Badge.svelte         # Reusable badge
│       ├── ProgressBar.svelte   # Reusable progress bar
│       └── index.ts             # Re-exports
└── utils/
    └── storage.ts               # Centralized localStorage access
```

## Files to Delete

```
src/lib/components/.gitkeep
src/lib/stores/.gitkeep
src/lib/utils/.gitkeep
src/lib/index.ts (if staying empty)
src/lib/components/FileBrowser.svelte (if not using)
```

## Files to Rename (Store Migration)

```
src/lib/stores/auth.ts → src/lib/stores/auth.svelte.ts
src/lib/stores/files.ts → src/lib/stores/files.svelte.ts
src/lib/stores/jobs.ts → src/lib/stores/jobs.svelte.ts
src/lib/stores/settings.ts → src/lib/stores/settings.svelte.ts
src/lib/stores/websocket.ts → src/lib/stores/websocket.svelte.ts
```
