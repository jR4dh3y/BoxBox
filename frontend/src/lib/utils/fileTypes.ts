/**
 * File type detection utilities - SINGLE SOURCE OF TRUTH
 * All file extension arrays and type detection logic should be imported from here
 */

import type { ComponentType } from 'svelte';
import {
	Folder,
	File,
	FileImage,
	FileVideo,
	FileAudio,
	FileText,
	FileCode,
	FileArchive,
	FileSpreadsheet,
	Globe,
	Palette,
	FileJson
} from 'lucide-svelte';

export type PreviewType =
	'video' | 'audio' | 'image' | 'pdf' | 'code' | 'text' | 'office' | 'notebook' | 'unsupported';

/**
 * Centralized file extension definitions
 * This is the ONLY place where file extensions should be defined
 */
const FILE_EXTENSIONS = {
	video: ['mp4', 'webm', 'mkv', 'avi', 'mov', 'wmv', 'flv', 'm4v', 'ogv'],
	audio: ['mp3', 'wav', 'flac', 'aac', 'ogg', 'm4a', 'wma', 'opus'],
	image: [
		'jpg',
		'jpeg',
		'png',
		'gif',
		'webp',
		'svg',
		'bmp',
		'ico',
		'avif',
		'tiff',
		'tif',
		'raw',
		'qoi'
	],
	pdf: ['pdf'],
	code: [
		// JavaScript/TypeScript
		'js',
		'jsx',
		'ts',
		'tsx',
		'mjs',
		'cjs',
		// Backend languages
		'py',
		'go',
		'rs',
		'java',
		'kt',
		'scala',
		'rb',
		'php',
		'cs',
		'fs',
		// Systems languages
		'c',
		'cpp',
		'cc',
		'cxx',
		'h',
		'hpp',
		'hxx',
		// Shell/Scripts
		'sh',
		'bash',
		'zsh',
		'fish',
		'ps1',
		'bat',
		'cmd',
		// Config files
		'ini',
		'conf',
		'cfg',
		'env',
		// Documentation
		'md',
		'mdx',
		'rst',
		'tex',
		// Database
		'sql',
		'graphql',
		'gql',
		// Other
		'dockerfile',
		'makefile',
		'cmake',
		'gradle',
		'swift',
		'r',
		'lua',
		'vim',
		'asm'
	],
	text: ['txt', 'log', 'csv', 'tsv'],
	archive: ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz'],
	spreadsheet: ['xls', 'xlsx', 'xlsm', 'xlsb', 'xlt', 'xltx', 'xltm', 'ods', 'ots'],
	document: ['doc', 'docx', 'docm', 'dot', 'dotx', 'dotm', 'odt', 'ott', 'rtf'],
	presentation: ['ppt', 'pptx', 'pptm', 'pot', 'potx', 'potm', 'pps', 'ppsx', 'ppsm', 'odp', 'otp'],
	notebook: ['ipynb'],
	web: ['html', 'htm', 'xml'],
	style: ['css', 'scss', 'sass', 'less'],
	data: ['json', 'yaml', 'yml', 'toml']
} as const;

const OFFICE_PREVIEW_EXTENSIONS = new Set<string>([
	...FILE_EXTENSIONS.spreadsheet,
	...FILE_EXTENSIONS.document,
	...FILE_EXTENSIONS.presentation
]);

const SPECIAL_CODE_FILENAMES = new Set<string>([
	'dockerfile',
	'makefile',
	'gnumakefile',
	'cmakelists.txt',
	'gemfile',
	'rakefile'
]);

export const SHELL_DOTFILES = new Set<string>([
	'.bashrc',
	'.bash_profile',
	'.bash_login',
	'.bash_logout',
	'.profile',
	'.zshrc',
	'.zprofile',
	'.zshenv',
	'.zlogin',
	'.zlogout',
	'.kshrc',
	'.cshrc',
	'.tcshrc'
]);

export const DOTFILE_LANGUAGE_MAP: Record<string, string> = {
	'.env': 'dotenv',
	'.envrc': 'shell',
	'.gitconfig': 'ini',
	'.gitignore': 'ignore',
	'.dockerignore': 'ignore',
	'.editorconfig': 'ini',
	'.npmrc': 'ini',
	'.yarnrc': 'ini',
	'.curlrc': 'shell',
	'.wgetrc': 'ini',
	'.vimrc': 'vim'
};

export function isSingleDotfile(filename: string): boolean {
	const baseName = filename.split('/').pop() ?? filename;
	return baseName.startsWith('.') && baseName.indexOf('.', 1) === -1 && baseName.length > 1;
}

/**
 * Get the file extension from a filename
 */
export function getExtension(filename: string): string {
	if (isSingleDotfile(filename)) return '';

	const lastDot = filename.lastIndexOf('.');
	if (lastDot === -1) return '';
	return filename.slice(lastDot + 1).toLowerCase();
}

/**
 * Determine the preview type for a file
 */
export function getPreviewType(filename: string): PreviewType {
	const ext = getExtension(filename);

	// Handle special filenames without extensions
	const lowerName = filename.toLowerCase();
	if (SPECIAL_CODE_FILENAMES.has(lowerName)) {
		return 'code';
	}
	if (isSingleDotfile(lowerName)) {
		// Dotfiles like .bashrc, .gitignore, .env, etc.
		return 'code';
	}

	if (OFFICE_PREVIEW_EXTENSIONS.has(ext)) return 'office';
	for (const type of ['notebook', 'video', 'audio', 'image', 'pdf', 'code', 'text'] as const) {
		const extensions: readonly string[] = FILE_EXTENSIONS[type];
		if (extensions.includes(ext)) return type;
	}
	for (const type of ['web', 'style', 'data'] as const) {
		const extensions: readonly string[] = FILE_EXTENSIONS[type];
		if (extensions.includes(ext)) return 'code';
	}

	return 'unsupported';
}

/**
 * Check if a file can be previewed
 */
export function canPreview(filename: string): boolean {
	return getPreviewType(filename) !== 'unsupported';
}

/**
 * Get a file type description based on extension
 */
export function getFileTypeDescription(filename: string): string {
	const ext = getExtension(filename);
	const lowerName = filename.toLowerCase();

	if (SHELL_DOTFILES.has(lowerName)) return 'Shell Config';
	if (DOTFILE_LANGUAGE_MAP[lowerName]) return 'Config File';
	if (isSingleDotfile(lowerName)) return 'Dotfile';
	if (!ext) return SPECIAL_CODE_FILENAMES.has(lowerName) ? 'Code File' : 'File';

	const typeMap: Record<string, string> = {
		// Documents
		pdf: 'PDF Document',
		doc: 'Word Document',
		docx: 'Word Document',
		docm: 'Word Document',
		dot: 'Word Template',
		dotx: 'Word Template',
		dotm: 'Word Template',
		xls: 'Excel Spreadsheet',
		xlsx: 'Excel Spreadsheet',
		xlsm: 'Excel Spreadsheet',
		xlsb: 'Excel Spreadsheet',
		xlt: 'Excel Template',
		xltx: 'Excel Template',
		xltm: 'Excel Template',
		ppt: 'PowerPoint',
		pptx: 'PowerPoint',
		pptm: 'PowerPoint',
		pot: 'PowerPoint Template',
		potx: 'PowerPoint Template',
		potm: 'PowerPoint Template',
		pps: 'PowerPoint Slideshow',
		ppsx: 'PowerPoint Slideshow',
		ppsm: 'PowerPoint Slideshow',
		txt: 'Text File',
		rtf: 'Rich Text',
		odt: 'OpenDocument Text',
		ott: 'OpenDocument Template',
		ods: 'OpenDocument Spreadsheet',
		ots: 'OpenDocument Spreadsheet Template',
		odp: 'OpenDocument Presentation',
		otp: 'OpenDocument Presentation Template',
		ipynb: 'Jupyter Notebook',
		// Images
		jpg: 'JPEG Image',
		jpeg: 'JPEG Image',
		png: 'PNG Image',
		gif: 'GIF Image',
		bmp: 'Bitmap Image',
		svg: 'SVG Image',
		webp: 'WebP Image',
		ico: 'Icon File',
		tiff: 'TIFF Image',
		tif: 'TIFF Image',
		raw: 'RAW Image',
		avif: 'AVIF Image',
		// Video
		mp4: 'MP4 Video',
		mkv: 'MKV Video',
		avi: 'AVI Video',
		mov: 'QuickTime Video',
		wmv: 'WMV Video',
		flv: 'Flash Video',
		webm: 'WebM Video',
		m4v: 'M4V Video',
		ogv: 'OGV Video',
		// Audio
		mp3: 'MP3 Audio',
		wav: 'WAV Audio',
		flac: 'FLAC Audio',
		aac: 'AAC Audio',
		ogg: 'OGG Audio',
		wma: 'WMA Audio',
		m4a: 'M4A Audio',
		opus: 'Opus Audio',
		// Archives
		zip: 'ZIP Archive',
		rar: 'RAR Archive',
		'7z': '7-Zip Archive',
		tar: 'TAR Archive',
		gz: 'GZip Archive',
		bz2: 'BZip2 Archive',
		xz: 'XZ Archive',
		// Code
		js: 'JavaScript',
		ts: 'TypeScript',
		jsx: 'React JSX',
		tsx: 'React TSX',
		py: 'Python',
		java: 'Java',
		c: 'C Source',
		cpp: 'C++ Source',
		h: 'C Header',
		hpp: 'C++ Header',
		cs: 'C#',
		go: 'Go',
		rs: 'Rust',
		rb: 'Ruby',
		php: 'PHP',
		swift: 'Swift',
		kt: 'Kotlin',
		scala: 'Scala',
		// Web
		html: 'HTML',
		htm: 'HTML',
		css: 'CSS',
		scss: 'SCSS',
		sass: 'Sass',
		less: 'Less',
		// Data
		json: 'JSON',
		xml: 'XML',
		yaml: 'YAML',
		yml: 'YAML',
		csv: 'CSV',
		sql: 'SQL',
		toml: 'TOML',
		// Config
		ini: 'Config File',
		conf: 'Config File',
		cfg: 'Config File',
		env: 'Environment File',
		// Executables
		exe: 'Windows Executable',
		msi: 'Windows Installer',
		dmg: 'macOS Disk Image',
		deb: 'Debian Package',
		rpm: 'RPM Package',
		sh: 'Shell Script',
		bat: 'Batch File',
		ps1: 'PowerShell',
		// Other
		iso: 'Disk Image',
		img: 'Disk Image',
		log: 'Log File',
		md: 'Markdown',
		lock: 'Lock File'
	};

	return typeMap[ext] || `${ext.toUpperCase()} File`;
}

const CODE_TINT = '#60a5fa';

/**
 * Each kind of file: its extensions, its icon, and its colour in the tinted set, as the desktop
 * app's `Kind::tint` gives it (null keeps the plain grey). The first match wins.
 */
const KINDS: [readonly string[], ComponentType, string | null][] = [
	[FILE_EXTENSIONS.pdf, FileText, '#f87171'],
	[['ini', 'conf', 'cfg', 'env', 'cmake', 'gradle'], FileCode, null],
	[FILE_EXTENSIONS.image, FileImage, '#2dd4bf'],
	[FILE_EXTENSIONS.video, FileVideo, '#a78bfa'],
	[FILE_EXTENSIONS.audio, FileAudio, '#fb923c'],
	[FILE_EXTENSIONS.code, FileCode, CODE_TINT],
	[FILE_EXTENSIONS.archive, FileArchive, '#fbbf24'],
	[['deb', 'rpm', 'appimage', 'flatpak', 'snap', 'apk', 'msi', 'jar'], File, '#fbbf24'],
	[FILE_EXTENSIONS.spreadsheet, FileSpreadsheet, '#4ade80'],
	[FILE_EXTENSIONS.presentation, FileText, '#fb7185'],
	[
		[...FILE_EXTENSIONS.document, ...FILE_EXTENSIONS.notebook, ...FILE_EXTENSIONS.text],
		FileText,
		null
	],
	[FILE_EXTENSIONS.web, Globe, CODE_TINT],
	[FILE_EXTENSIONS.style, Palette, CODE_TINT],
	[FILE_EXTENSIONS.data, FileJson, CODE_TINT],
	[['db', 'sqlite', 'sqlite3'], File, CODE_TINT]
];

/** A file's kind, or null for a name with no extension; undefined for an unknown extension. */
function fileKind(filename: string) {
	const ext = getExtension(filename);
	return ext ? KINDS.find(([extensions]) => extensions.includes(ext)) : null;
}

/** The colour of a file's kind in the tinted icon set; null keeps the plain grey. */
export function getFileTint(filename: string, isDir: boolean): string | null {
	if (isDir) return null;
	const kind = fileKind(filename);
	if (kind === null) return getPreviewType(filename) === 'code' ? CODE_TINT : null;
	return kind?.[2] ?? null;
}

/**
 * Get the appropriate icon component for a file
 */
export function getFileIcon(filename: string, isDir: boolean): ComponentType {
	if (isDir) return Folder;
	const kind = fileKind(filename);
	if (kind === null) return getPreviewType(filename) === 'code' ? FileCode : File;
	return kind?.[1] ?? File;
}
