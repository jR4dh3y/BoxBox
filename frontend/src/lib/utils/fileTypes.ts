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

export type PreviewType = 'video' | 'audio' | 'image' | 'pdf' | 'code' | 'text' | 'unsupported';
export type FileCategory = keyof typeof FILE_EXTENSIONS | 'unknown';

/**
 * Centralized file extension definitions
 * This is the ONLY place where file extensions should be defined
 */
export const FILE_EXTENSIONS = {
	video: ['mp4', 'webm', 'mkv', 'avi', 'mov', 'wmv', 'flv', 'm4v', 'ogv'],
	audio: ['mp3', 'wav', 'flac', 'aac', 'ogg', 'm4a', 'wma', 'opus'],
	image: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'bmp', 'ico', 'avif', 'tiff', 'tif', 'raw'],
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
	text: ['txt', 'log', 'csv', 'tsv', 'rtf'],
	archive: ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz'],
	spreadsheet: ['xls', 'xlsx', 'ods'],
	document: ['doc', 'docx', 'odt', 'ppt', 'pptx', 'odp'],
	web: ['html', 'htm', 'xml'],
	style: ['css', 'scss', 'sass', 'less'],
	data: ['json', 'yaml', 'yml', 'toml']
} as const;

/**
 * Get the file extension from a filename
 */
export function getExtension(filename: string): string {
	const lastDot = filename.lastIndexOf('.');
	if (lastDot === -1) return '';
	return filename.slice(lastDot + 1).toLowerCase();
}

/**
 * Get the file category based on extension
 */
export function getFileCategory(filename: string): FileCategory {
	const ext = getExtension(filename);
	if (!ext) return 'unknown';

	for (const [category, extensions] of Object.entries(FILE_EXTENSIONS)) {
		if ((extensions as readonly string[]).includes(ext)) {
			return category as FileCategory;
		}
	}
	return 'unknown';
}

/**
 * Determine the preview type for a file
 */
export function getPreviewType(filename: string): PreviewType {
	const ext = getExtension(filename);

	// Handle special filenames without extensions
	const lowerName = filename.toLowerCase();
	if (['dockerfile', 'makefile', 'cmakelists.txt', 'gemfile', 'rakefile'].includes(lowerName)) {
		return 'code';
	}
	if (lowerName.startsWith('.') && !ext) {
		// Dotfiles like .gitignore, .env, etc.
		return 'code';
	}

	if (!ext) return 'unsupported';

	if (FILE_EXTENSIONS.video.includes(ext as (typeof FILE_EXTENSIONS.video)[number])) return 'video';
	if (FILE_EXTENSIONS.audio.includes(ext as (typeof FILE_EXTENSIONS.audio)[number])) return 'audio';
	if (FILE_EXTENSIONS.image.includes(ext as (typeof FILE_EXTENSIONS.image)[number])) return 'image';
	if (FILE_EXTENSIONS.pdf.includes(ext as (typeof FILE_EXTENSIONS.pdf)[number])) return 'pdf';
	if (FILE_EXTENSIONS.code.includes(ext as (typeof FILE_EXTENSIONS.code)[number])) return 'code';
	if (FILE_EXTENSIONS.text.includes(ext as (typeof FILE_EXTENSIONS.text)[number])) return 'text';
	if (FILE_EXTENSIONS.web.includes(ext as (typeof FILE_EXTENSIONS.web)[number])) return 'code';
	if (FILE_EXTENSIONS.style.includes(ext as (typeof FILE_EXTENSIONS.style)[number])) return 'code';
	if (FILE_EXTENSIONS.data.includes(ext as (typeof FILE_EXTENSIONS.data)[number])) return 'code';

	return 'unsupported';
}

/**
 * Get Monaco Editor language ID from file extension
 */
export function getMonacoLanguage(filename: string): string {
	const ext = getExtension(filename);
	const lowerName = filename.toLowerCase();

	// Special filenames
	if (lowerName === 'dockerfile') return 'dockerfile';
	if (lowerName === 'makefile' || lowerName === 'gnumakefile') return 'makefile';
	if (lowerName.endsWith('.gitignore') || lowerName.endsWith('.dockerignore')) return 'ignore';

	const languageMap: Record<string, string> = {
		// JavaScript/TypeScript
		js: 'javascript',
		jsx: 'javascript',
		mjs: 'javascript',
		cjs: 'javascript',
		ts: 'typescript',
		tsx: 'typescript',
		// Web
		html: 'html',
		htm: 'html',
		css: 'css',
		scss: 'scss',
		sass: 'scss',
		less: 'less',
		// Data formats
		json: 'json',
		yaml: 'yaml',
		yml: 'yaml',
		toml: 'ini',
		xml: 'xml',
		// Backend languages
		py: 'python',
		go: 'go',
		rs: 'rust',
		java: 'java',
		kt: 'kotlin',
		scala: 'scala',
		rb: 'ruby',
		php: 'php',
		cs: 'csharp',
		fs: 'fsharp',
		// Systems languages
		c: 'c',
		cpp: 'cpp',
		cc: 'cpp',
		cxx: 'cpp',
		h: 'c',
		hpp: 'cpp',
		hxx: 'cpp',
		// Shell/Scripts
		sh: 'shell',
		bash: 'shell',
		zsh: 'shell',
		fish: 'shell',
		ps1: 'powershell',
		bat: 'bat',
		cmd: 'bat',
		// Config files
		ini: 'ini',
		conf: 'ini',
		cfg: 'ini',
		env: 'dotenv',
		// Documentation
		md: 'markdown',
		mdx: 'markdown',
		rst: 'restructuredtext',
		tex: 'latex',
		// Database
		sql: 'sql',
		graphql: 'graphql',
		gql: 'graphql',
		// Other
		swift: 'swift',
		r: 'r',
		lua: 'lua',
		vim: 'vim',
		// Plain text
		txt: 'plaintext',
		log: 'plaintext',
		csv: 'plaintext',
		tsv: 'plaintext'
	};

	return languageMap[ext] || 'plaintext';
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
	if (!ext) return 'File';

	const typeMap: Record<string, string> = {
		// Documents
		pdf: 'PDF Document',
		doc: 'Word Document',
		docx: 'Word Document',
		xls: 'Excel Spreadsheet',
		xlsx: 'Excel Spreadsheet',
		ppt: 'PowerPoint',
		pptx: 'PowerPoint',
		txt: 'Text File',
		rtf: 'Rich Text',
		odt: 'OpenDocument Text',
		ods: 'OpenDocument Spreadsheet',
		odp: 'OpenDocument Presentation',
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

/**
 * Get the appropriate icon component for a file
 */
export function getFileIcon(filename: string, isDir: boolean): ComponentType {
	if (isDir) return Folder;

	const ext = getExtension(filename);
	if (!ext) return File;

	// Check each category
	if (FILE_EXTENSIONS.image.includes(ext as (typeof FILE_EXTENSIONS.image)[number]))
		return FileImage;
	if (FILE_EXTENSIONS.video.includes(ext as (typeof FILE_EXTENSIONS.video)[number]))
		return FileVideo;
	if (FILE_EXTENSIONS.audio.includes(ext as (typeof FILE_EXTENSIONS.audio)[number]))
		return FileAudio;
	if (FILE_EXTENSIONS.code.includes(ext as (typeof FILE_EXTENSIONS.code)[number])) return FileCode;
	if (FILE_EXTENSIONS.archive.includes(ext as (typeof FILE_EXTENSIONS.archive)[number]))
		return FileArchive;
	if (FILE_EXTENSIONS.spreadsheet.includes(ext as (typeof FILE_EXTENSIONS.spreadsheet)[number]))
		return FileSpreadsheet;
	if (FILE_EXTENSIONS.document.includes(ext as (typeof FILE_EXTENSIONS.document)[number]))
		return FileText;
	if (FILE_EXTENSIONS.pdf.includes(ext as (typeof FILE_EXTENSIONS.pdf)[number])) return FileText;
	if (FILE_EXTENSIONS.text.includes(ext as (typeof FILE_EXTENSIONS.text)[number])) return FileText;
	if (FILE_EXTENSIONS.web.includes(ext as (typeof FILE_EXTENSIONS.web)[number])) return Globe;
	if (FILE_EXTENSIONS.style.includes(ext as (typeof FILE_EXTENSIONS.style)[number])) return Palette;
	if (FILE_EXTENSIONS.data.includes(ext as (typeof FILE_EXTENSIONS.data)[number])) return FileJson;

	return File;
}
