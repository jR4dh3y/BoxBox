<script lang="ts">
	/**
	 * Component Testing Page
	 * Test all UI components without backend
	 */
	import {
		Button,
		Input,
		Select,
		Toggle,
		Card,
		Modal,
		Spinner,
		Badge,
		ProgressBar
	} from '$lib/components/ui';
	import Toolbar from '$lib/components/Toolbar.svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import SystemDriveCard from '$lib/components/SystemDriveCard.svelte';
	import FileList from '$lib/components/FileList.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import SearchBar from '$lib/components/SearchBar.svelte';
	import UploadDropzone from '$lib/components/UploadDropzone.svelte';
	import UploadProgress from '$lib/components/UploadProgress.svelte';
	import JobMonitor from '$lib/components/JobMonitor.svelte';
	import PicoLogo from '$lib/components/PicoLogo.svelte';
	import type { ContextMenuItem } from '$lib/components/ui/ContextMenu.svelte';
	import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
	import {
		FlaskConical,
		Bell,
		Copy,
		Scissors,
		ClipboardPaste,
		Trash2,
		Pencil
	} from 'lucide-svelte';
	import type { FileInfo, MountPoint } from '$lib/api/files';
	import type { SystemDrive } from '$lib/api/system';
	import type { Job } from '$lib/api/jobs';
	import type { UploadProgress as UploadProgressType } from '$lib/utils/upload';

	// UI Component States
	let inputValue = $state('');
	let selectValue = $state('option1');
	let toggleChecked = $state(false);
	let modalOpen = $state(false);
	let progressValue = $state(65);

	// Context Menu State
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);
	let contextMenuItems: ContextMenuItem[] = $state([
		{ id: 'copy', label: 'Copy', icon: Copy, shortcut: 'Ctrl+C' },
		{ id: 'cut', label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X' },
		{ id: 'paste', label: 'Paste', icon: ClipboardPaste, shortcut: 'Ctrl+V', disabled: true },
		{ id: 'sep1', label: '', separator: true },
		{ id: 'delete', label: 'Delete', icon: Trash2, shortcut: 'Del' },
		{ id: 'rename', label: 'Rename', icon: Pencil, shortcut: 'F2' }
	]);

	// Toolbar state
	let toolbarPath = $state(['Documents', 'Projects', 'Frontend']);
	let viewMode = $state<'list' | 'grid'>('list');

	// Mock data for components
	const mockRoots: MountPoint[] = [
		{ name: 'Home', readOnly: false },
		{ name: 'Media', readOnly: true },
		{ name: 'Downloads', readOnly: false }
	];

	const mockSystemDrives: SystemDrive[] = [
		{
			device: '/dev/sda1',
			mountPoint: '/home',
			fsType: 'ext4',
			totalBytes: 500000000000,
			usedBytes: 350000000000,
			freeBytes: 150000000000,
			usedPct: 70
		},
		{
			device: '/dev/sdb1',
			mountPoint: '/media/storage',
			fsType: 'ntfs',
			totalBytes: 2000000000000,
			usedBytes: 1800000000000,
			freeBytes: 200000000000,
			usedPct: 90
		},
		{
			device: '/dev/sdc1',
			mountPoint: '/mnt/downloads',
			fsType: 'ext4',
			totalBytes: 100000000000,
			usedBytes: 25000000000,
			freeBytes: 75000000000,
			usedPct: 25
		}
	];

	const mockFiles: FileInfo[] = [
		{
			name: 'Documents',
			path: '/Documents',
			isDir: true,
			size: 0,
			modTime: '2025-01-01T10:00:00Z',
			permissions: 'drwxr-xr-x'
		},
		{
			name: 'Photos',
			path: '/Photos',
			isDir: true,
			size: 0,
			modTime: '2025-01-01T09:00:00Z',
			permissions: 'drwxr-xr-x'
		},
		{
			name: 'report.pdf',
			path: '/report.pdf',
			isDir: false,
			size: 1024000,
			modTime: '2025-01-01T08:00:00Z',
			permissions: '-rw-r--r--'
		},
		{
			name: 'image.png',
			path: '/image.png',
			isDir: false,
			size: 2048000,
			modTime: '2024-12-25T12:00:00Z',
			permissions: '-rw-r--r--'
		},
		{
			name: 'video.mp4',
			path: '/video.mp4',
			isDir: false,
			size: 150000000,
			modTime: '2024-12-20T15:30:00Z',
			permissions: '-rw-r--r--'
		},
		{
			name: 'music.mp3',
			path: '/music.mp3',
			isDir: false,
			size: 5000000,
			modTime: '2024-12-15T18:00:00Z',
			permissions: '-rw-r--r--'
		},
		{
			name: 'script.js',
			path: '/script.js',
			isDir: false,
			size: 15000,
			modTime: '2024-12-10T09:00:00Z',
			permissions: '-rw-r--r--'
		},
		{
			name: 'data.json',
			path: '/data.json',
			isDir: false,
			size: 8500,
			modTime: '2024-12-05T14:00:00Z',
			permissions: '-rw-r--r--'
		}
	];

	const mockJobs: Job[] = [
		{
			id: '1',
			type: 'copy',
			state: 'running',
			sourcePath: '/home/file1.zip',
			destPath: '/backup/file1.zip',
			progress: 45,
			createdAt: '2025-01-01T10:00:00Z'
		},
		{
			id: '2',
			type: 'move',
			state: 'pending',
			sourcePath: '/downloads/movie.mp4',
			destPath: '/media/movie.mp4',
			progress: 0,
			createdAt: '2025-01-01T10:01:00Z'
		},
		{
			id: '3',
			type: 'delete',
			state: 'completed',
			sourcePath: '/tmp/old-file.txt',
			progress: 100,
			createdAt: '2025-01-01T09:00:00Z',
			completedAt: '2025-01-01T09:01:00Z'
		},
		{
			id: '4',
			type: 'copy',
			state: 'failed',
			sourcePath: '/home/broken.zip',
			destPath: '/backup/broken.zip',
			progress: 30,
			error: 'Disk full',
			createdAt: '2025-01-01T08:00:00Z'
		}
	];

	const mockUploads: UploadProgressType[] = [
		{
			uploadId: 'u1',
			fileName: 'large-video.mp4',
			totalSize: 500000000,
			uploadedSize: 250000000,
			percentage: 50,
			currentChunk: 5,
			totalChunks: 10,
			status: 'uploading'
		},
		{
			uploadId: 'u2',
			fileName: 'document.pdf',
			totalSize: 1024000,
			uploadedSize: 1024000,
			percentage: 100,
			currentChunk: 1,
			totalChunks: 1,
			status: 'complete'
		},
		{
			uploadId: 'u3',
			fileName: 'image.png',
			totalSize: 2048000,
			uploadedSize: 0,
			percentage: 0,
			currentChunk: 0,
			totalChunks: 1,
			status: 'pending'
		}
	];

	let selectedPaths = $state(new Set<string>());
	let sortBy = $state<'name' | 'size' | 'modTime' | 'type'>('name');
	let sortDir = $state<'asc' | 'desc'>('asc');
	let searchQuery = $state('');
	let searchLoading = $state(false);

	function handleNavigate(path: string) {
		console.log('Navigate to:', path);
	}

	function handleFileClick(file: FileInfo) {
		console.log('File clicked:', file);
	}

	function handleFilesSelected(files: File[]) {
		console.log(
			'Files selected:',
			files.map((f) => f.name)
		);
	}
</script>

<svelte:head>
	<title>Component Test Page</title>
</svelte:head>

<div class="min-h-screen bg-surface-primary p-8 text-text-primary">
	<h1 class="mb-8 flex items-center gap-3 text-3xl font-bold">
		<FlaskConical size={32} /> Component Test Page
	</h1>
	<p class="mb-8 text-text-secondary">Test all UI components without backend connection</p>

	<!-- Section: Branding -->
	<section class="mb-12">
		<h2 class="mb-6 border-b border-border-primary pb-2 text-2xl font-semibold">Branding</h2>

		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">PicoLogo</h3>
			<div class="flex flex-wrap items-end gap-8">
				<div class="flex flex-col items-center gap-2">
					<PicoLogo size="sm" />
					<span class="text-xs text-text-muted">Small</span>
				</div>
				<div class="flex flex-col items-center gap-2">
					<PicoLogo size="md" />
					<span class="text-xs text-text-muted">Medium</span>
				</div>
				<div class="flex flex-col items-center gap-2">
					<PicoLogo size="lg" />
					<span class="text-xs text-text-muted">Large</span>
				</div>
			</div>
		</div>
	</section>

	<!-- Section: Base UI Components -->
	<section class="mb-12">
		<h2 class="mb-6 border-b border-border-primary pb-2 text-2xl font-semibold">
			Base UI Components
		</h2>

		<!-- Buttons -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Buttons</h3>
			<div class="flex flex-wrap items-center gap-4">
				<Button variant="primary">Primary</Button>
				<Button variant="secondary">Secondary</Button>
				<Button variant="ghost">Ghost</Button>
				<Button variant="danger">Danger</Button>
				<Button variant="primary" disabled>Disabled</Button>
				<Button variant="primary" size="sm">Small</Button>
				<Button variant="primary" size="lg">Large</Button>
				<Button variant="secondary" size="icon" title="Icon Button"><Bell size={16} /></Button>
			</div>
		</div>

		<!-- Inputs -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Inputs</h3>
			<div class="flex max-w-md flex-wrap items-start gap-4">
				<div class="w-full">
					<Input placeholder="Type something..." bind:value={inputValue} />
					<p class="mt-1 text-xs text-text-muted">Value: {inputValue || '(empty)'}</p>
				</div>
				<div class="w-full">
					<Input type="password" placeholder="Password input" />
				</div>
				<div class="w-full">
					<Input placeholder="Disabled input" disabled />
				</div>
			</div>
		</div>

		<!-- Select -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Select</h3>
			<div class="max-w-xs">
				<Select
					bind:value={selectValue}
					options={[
						{ value: 'option1', label: 'Option 1' },
						{ value: 'option2', label: 'Option 2' },
						{ value: 'option3', label: 'Option 3' }
					]}
				/>
				<p class="mt-1 text-xs text-text-muted">Selected: {selectValue}</p>
			</div>
		</div>

		<!-- Toggle -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Toggle</h3>
			<div class="flex flex-col gap-4">
				<Toggle bind:checked={toggleChecked} label="Enable feature" />
				<p class="text-xs text-text-muted">State: {toggleChecked ? 'ON' : 'OFF'}</p>
				<Toggle checked={true} disabled label="Disabled toggle" />
			</div>
		</div>

		<!-- Badges -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Badges</h3>
			<div class="flex flex-wrap gap-3">
				<Badge>Default</Badge>
				<Badge variant="success">Success</Badge>
				<Badge variant="warning">Warning</Badge>
				<Badge variant="danger">Danger</Badge>
				<Badge variant="info">Info</Badge>
			</div>
		</div>

		<!-- Spinner -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Spinner</h3>
			<div class="flex items-center gap-6">
				<div class="flex flex-col items-center gap-2">
					<Spinner size="sm" />
					<span class="text-xs text-text-muted">Small</span>
				</div>
				<div class="flex flex-col items-center gap-2">
					<Spinner size="md" />
					<span class="text-xs text-text-muted">Medium</span>
				</div>
				<div class="flex flex-col items-center gap-2">
					<Spinner size="lg" />
					<span class="text-xs text-text-muted">Large</span>
				</div>
			</div>
		</div>

		<!-- Progress Bar -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Progress Bar</h3>
			<div class="flex max-w-md flex-col gap-4">
				<div>
					<ProgressBar value={progressValue} showLabel />
					<input type="range" min="0" max="100" bind:value={progressValue} class="mt-2 w-full" />
				</div>
				<ProgressBar value={100} variant="success" />
				<ProgressBar value={75} variant="warning" />
				<ProgressBar value={30} variant="danger" />
				<ProgressBar value={50} size="sm" />
			</div>
		</div>

		<!-- Cards -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Cards</h3>
			<div class="grid grid-cols-1 gap-4 md:grid-cols-3">
				<Card>
					<h4 class="mb-2 font-medium">Default Card</h4>
					<p class="text-sm text-text-secondary">This is a basic card component.</p>
				</Card>
				<Card variant="interactive" onclick={() => console.log('Card clicked!')}>
					<h4 class="mb-2 font-medium">Interactive Card</h4>
					<p class="text-sm text-text-secondary">Click me! I'm interactive.</p>
				</Card>
				<Card padding="lg">
					<h4 class="mb-2 font-medium">Large Padding</h4>
					<p class="text-sm text-text-secondary">This card has more padding.</p>
				</Card>
			</div>
		</div>

		<!-- Modal -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Modal</h3>
			<Button onclick={() => (modalOpen = true)}>Open Modal</Button>
			<Modal open={modalOpen} title="Test Modal" onclose={() => (modalOpen = false)}>
				<p class="text-text-secondary">This is the modal content. You can put anything here.</p>
				<div class="mt-4">
					<Input placeholder="Example input in modal" />
				</div>
				{#snippet footer()}
					<Button variant="secondary" onclick={() => (modalOpen = false)}>Cancel</Button>
					<Button variant="primary" onclick={() => (modalOpen = false)}>Confirm</Button>
				{/snippet}
			</Modal>
		</div>
	</section>

	<!-- Section: Context Menu -->
	<section class="mb-12">
		<h2 class="mb-6 border-b border-border-primary pb-2 text-2xl font-semibold">Context Menu</h2>

		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">
				Right-click anywhere to open context menu
			</h3>
			<div
				class="flex h-48 w-full items-center justify-center rounded-lg border-2 border-dashed border-border-secondary text-text-muted"
				role="region"
				oncontextmenu={(e) => {
					e.preventDefault();
					contextMenuX = e.clientX;
					contextMenuY = e.clientY;
					contextMenuOpen = true;
				}}
			>
				Right-click Area
			</div>

			{#if contextMenuOpen}
				<ContextMenu
					x={contextMenuX}
					y={contextMenuY}
					items={contextMenuItems}
					onSelect={(id) => {
						console.log('Context menu item selected:', id);
						contextMenuOpen = false;
					}}
					onClose={() => (contextMenuOpen = false)}
				/>
			{/if}
		</div>
	</section>

	<!-- Section: File Manager Components -->
	<section class="mb-12">
		<h2 class="mb-6 border-b border-border-primary pb-2 text-2xl font-semibold">
			File Manager Components
		</h2>

		<!-- Breadcrumb -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Breadcrumb</h3>
			<Breadcrumb segments={toolbarPath} onNavigate={handleNavigate} />
		</div>

		<!-- SearchBar -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">SearchBar</h3>
			<div class="max-w-md">
				<SearchBar
					value={searchQuery}
					isLoading={searchLoading}
					onSearch={(q) => console.log('Search:', q)}
					onInput={(q) => {
						searchQuery = q;
						console.log('Input:', q);
					}}
				/>
				<div class="mt-2">
					<Toggle bind:checked={searchLoading} label="Simulate loading" />
				</div>
			</div>
		</div>

		<!-- Toolbar -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Toolbar</h3>
			<div class="overflow-hidden rounded border border-border-primary">
				<Toolbar
					pathSegments={toolbarPath}
					canGoBack={true}
					canGoForward={false}
					canGoUp={true}
					onBack={() => console.log('Back')}
					onForward={() => console.log('Forward')}
					onUp={() => console.log('Up')}
					onNavigate={handleNavigate}
					onRefresh={() => console.log('Refresh')}
					onSettings={() => console.log('Settings')}
				/>
			</div>
		</div>

		<!-- StatusBar -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">StatusBar</h3>
			<div class="overflow-hidden rounded border border-border-primary">
				<StatusBar
					itemCount={mockFiles.length}
					selectedCount={selectedPaths.size}
					{viewMode}
					onViewModeChange={(mode) => (viewMode = mode)}
				/>
			</div>
		</div>

		<!-- SystemDriveCard -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">SystemDriveCard</h3>
			<div class="grid grid-cols-1 gap-4 md:grid-cols-3">
				{#each mockSystemDrives as drive (drive.mountPoint)}
					<SystemDriveCard
						{drive}
						onClick={() => console.log('Drive clicked:', drive.mountPoint)}
					/>
				{/each}
			</div>
		</div>

		<!-- Sidebar -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">Sidebar</h3>
			<div class="h-[400px] overflow-hidden rounded border border-border-primary">
				<Sidebar roots={mockRoots} currentPath="Home" onNavigate={handleNavigate} />
			</div>
		</div>

		<!-- FileList -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">FileList</h3>
			<div class="h-[400px] overflow-hidden rounded border border-border-primary">
				<FileList
					items={mockFiles}
					{sortBy}
					{sortDir}
					{selectedPaths}
					isLoading={false}
					compactMode={false}
					onItemClick={handleFileClick}
					onSortChange={(field, dir) => {
						sortBy = field;
						sortDir = dir;
					}}
					onSelectionChange={(paths) => (selectedPaths = paths)}
				/>
			</div>
			<p class="mt-2 text-xs text-text-muted">Selected: {selectedPaths.size} items</p>
		</div>
	</section>

	<!-- Section: Upload & Job Components -->
	<section class="mb-12">
		<h2 class="mb-6 border-b border-border-primary pb-2 text-2xl font-semibold">
			Upload & Job Components
		</h2>

		<!-- UploadDropzone -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">UploadDropzone</h3>
			<div class="max-w-md">
				<UploadDropzone onFilesSelected={handleFilesSelected} />
			</div>
		</div>

		<!-- UploadProgress -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">UploadProgress</h3>
			<div class="max-w-md">
				<UploadProgress
					uploads={mockUploads}
					onCancel={(id) => console.log('Cancel upload:', id)}
					onRemove={(id) => console.log('Remove upload:', id)}
					onClearCompleted={() => console.log('Clear completed uploads')}
				/>
			</div>
		</div>

		<!-- JobMonitor -->
		<div class="mb-8">
			<h3 class="mb-4 text-lg font-medium text-text-secondary">JobMonitor</h3>
			<div class="max-w-md">
				<JobMonitor
					jobs={mockJobs}
					onCancel={(id) => console.log('Cancel job:', id)}
					onRemove={(id) => console.log('Remove job:', id)}
					onClearCompleted={() => console.log('Clear completed jobs')}
				/>
			</div>
		</div>
	</section>

	<!-- Section: Color Palette -->
	<section class="mb-12">
		<h2 class="mb-6 border-b border-border-primary pb-2 text-2xl font-semibold">
			Design Tokens (Color Palette)
		</h2>

		<div class="grid grid-cols-2 gap-4 md:grid-cols-4">
			<!-- Surface colors -->
			<div class="flex flex-col gap-2">
				<h4 class="text-sm font-medium text-text-secondary">Surfaces</h4>
				<div
					class="flex h-12 items-center justify-center rounded border border-border-primary bg-surface-primary text-xs"
				>
					surface-primary
				</div>
				<div
					class="flex h-12 items-center justify-center rounded border border-border-primary bg-surface-secondary text-xs"
				>
					surface-secondary
				</div>
				<div
					class="flex h-12 items-center justify-center rounded border border-border-primary bg-surface-tertiary text-xs"
				>
					surface-tertiary
				</div>
				<div
					class="flex h-12 items-center justify-center rounded border border-border-primary bg-surface-elevated text-xs"
				>
					surface-elevated
				</div>
			</div>

			<!-- Text colors -->
			<div class="flex flex-col gap-2">
				<h4 class="text-sm font-medium text-text-secondary">Text</h4>
				<div
					class="flex h-12 items-center justify-center rounded bg-surface-secondary text-xs text-text-primary"
				>
					text-primary
				</div>
				<div
					class="flex h-12 items-center justify-center rounded bg-surface-secondary text-xs text-text-secondary"
				>
					text-secondary
				</div>
				<div
					class="flex h-12 items-center justify-center rounded bg-surface-secondary text-xs text-text-muted"
				>
					text-muted
				</div>
				<div
					class="flex h-12 items-center justify-center rounded bg-surface-secondary text-xs text-text-disabled"
				>
					text-disabled
				</div>
			</div>

			<!-- Accent colors -->
			<div class="flex flex-col gap-2">
				<h4 class="text-sm font-medium text-text-secondary">Accent</h4>
				<div class="flex h-12 items-center justify-center rounded bg-accent text-xs text-white">
					accent
				</div>
				<div
					class="flex h-12 items-center justify-center rounded bg-accent-hover text-xs text-white"
				>
					accent-hover
				</div>
				<div
					class="flex h-12 items-center justify-center rounded bg-accent-muted text-xs text-white"
				>
					accent-muted
				</div>
				<div class="flex h-12 items-center justify-center rounded bg-selection text-xs text-white">
					selection
				</div>
			</div>

			<!-- Semantic colors -->
			<div class="flex flex-col gap-2">
				<h4 class="text-sm font-medium text-text-secondary">Semantic</h4>
				<div class="flex h-12 items-center justify-center rounded bg-success text-xs text-white">
					success
				</div>
				<div class="flex h-12 items-center justify-center rounded bg-warning text-xs text-white">
					warning
				</div>
				<div class="flex h-12 items-center justify-center rounded bg-danger text-xs text-white">
					danger
				</div>
				<div class="flex h-12 items-center justify-center rounded bg-folder text-xs text-black">
					folder
				</div>
			</div>
		</div>
	</section>

	<footer class="border-t border-border-primary py-8 text-center text-sm text-text-muted">
		<p>Component Test Page - No backend required</p>
	</footer>
</div>
