type LanguageLoader = () => Promise<unknown>;

const loadBasicLanguages = () =>
	import('monaco-editor/basic-languages/monaco.contribution.js');
const loadJsonLanguage = () => import('monaco-editor/language/json/monaco.contribution.js');

const languageLoaders: Record<string, LanguageLoader> = {
	javascript: loadBasicLanguages,
	typescript: loadBasicLanguages,
	html: loadBasicLanguages,
	css: loadBasicLanguages,
	scss: loadBasicLanguages,
	less: loadBasicLanguages,
	json: loadJsonLanguage,
	yaml: loadBasicLanguages,
	xml: loadBasicLanguages,
	python: loadBasicLanguages,
	go: loadBasicLanguages,
	rust: loadBasicLanguages,
	java: loadBasicLanguages,
	kotlin: loadBasicLanguages,
	scala: loadBasicLanguages,
	ruby: loadBasicLanguages,
	php: loadBasicLanguages,
	csharp: loadBasicLanguages,
	fsharp: loadBasicLanguages,
	c: loadBasicLanguages,
	cpp: loadBasicLanguages,
	shell: loadBasicLanguages,
	powershell: loadBasicLanguages,
	bat: loadBasicLanguages,
	ini: loadBasicLanguages,
	dotenv: loadBasicLanguages,
	markdown: loadBasicLanguages,
	restructuredtext: loadBasicLanguages,
	sql: loadBasicLanguages,
	graphql: loadBasicLanguages,
	swift: loadBasicLanguages,
	r: loadBasicLanguages,
	lua: loadBasicLanguages,
	dockerfile: loadBasicLanguages
};

const languagePromises = new Map<string, Promise<unknown>>();

export async function loadMonacoLanguage(language: string): Promise<void> {
	const loader = languageLoaders[language];
	if (!loader) return;

	let promise = languagePromises.get(language);
	if (!promise) {
		promise = loader();
		languagePromises.set(language, promise);
	}
	await promise;
}
