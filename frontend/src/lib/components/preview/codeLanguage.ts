import {
	getExtension,
	SHELL_DOTFILES,
	DOTFILE_LANGUAGE_MAP,
	isSingleDotfile
} from '#lib/utils/fileTypes.js';

/**
 * Get Monaco Editor language ID from file extension
 */
export function getMonacoLanguage(filename: string): string {
	const ext = getExtension(filename);
	const lowerName = filename.toLowerCase();

	// Special filenames
	if (lowerName === 'dockerfile') return 'dockerfile';
	if (lowerName === 'makefile' || lowerName === 'gnumakefile') return 'makefile';
	if (SHELL_DOTFILES.has(lowerName)) return 'shell';
	if (DOTFILE_LANGUAGE_MAP[lowerName]) return DOTFILE_LANGUAGE_MAP[lowerName];
	if (isSingleDotfile(lowerName)) return 'plaintext';

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
