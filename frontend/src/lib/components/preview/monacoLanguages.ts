type LanguageLoader = () => Promise<unknown>;

const loadCpp = () => import('monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution.js');
const loadIni = () => import('monaco-editor/esm/vs/basic-languages/ini/ini.contribution.js');

const languageLoaders: Record<string, LanguageLoader> = {
	javascript: () =>
		import('monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution.js'),
	typescript: () =>
		import('monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution.js'),
	html: () => import('monaco-editor/esm/vs/basic-languages/html/html.contribution.js'),
	css: () => import('monaco-editor/esm/vs/basic-languages/css/css.contribution.js'),
	scss: () => import('monaco-editor/esm/vs/basic-languages/scss/scss.contribution.js'),
	less: () => import('monaco-editor/esm/vs/basic-languages/less/less.contribution.js'),
	json: () => import('monaco-editor/esm/vs/language/json/monaco.contribution.js'),
	yaml: () => import('monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution.js'),
	xml: () => import('monaco-editor/esm/vs/basic-languages/xml/xml.contribution.js'),
	python: () => import('monaco-editor/esm/vs/basic-languages/python/python.contribution.js'),
	go: () => import('monaco-editor/esm/vs/basic-languages/go/go.contribution.js'),
	rust: () => import('monaco-editor/esm/vs/basic-languages/rust/rust.contribution.js'),
	java: () => import('monaco-editor/esm/vs/basic-languages/java/java.contribution.js'),
	kotlin: () => import('monaco-editor/esm/vs/basic-languages/kotlin/kotlin.contribution.js'),
	scala: () => import('monaco-editor/esm/vs/basic-languages/scala/scala.contribution.js'),
	ruby: () => import('monaco-editor/esm/vs/basic-languages/ruby/ruby.contribution.js'),
	php: () => import('monaco-editor/esm/vs/basic-languages/php/php.contribution.js'),
	csharp: () => import('monaco-editor/esm/vs/basic-languages/csharp/csharp.contribution.js'),
	fsharp: () => import('monaco-editor/esm/vs/basic-languages/fsharp/fsharp.contribution.js'),
	c: loadCpp,
	cpp: loadCpp,
	shell: () => import('monaco-editor/esm/vs/basic-languages/shell/shell.contribution.js'),
	powershell: () =>
		import('monaco-editor/esm/vs/basic-languages/powershell/powershell.contribution.js'),
	bat: () => import('monaco-editor/esm/vs/basic-languages/bat/bat.contribution.js'),
	ini: loadIni,
	dotenv: loadIni,
	markdown: () => import('monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution.js'),
	restructuredtext: () =>
		import('monaco-editor/esm/vs/basic-languages/restructuredtext/restructuredtext.contribution.js'),
	sql: () => import('monaco-editor/esm/vs/basic-languages/sql/sql.contribution.js'),
	graphql: () => import('monaco-editor/esm/vs/basic-languages/graphql/graphql.contribution.js'),
	swift: () => import('monaco-editor/esm/vs/basic-languages/swift/swift.contribution.js'),
	r: () => import('monaco-editor/esm/vs/basic-languages/r/r.contribution.js'),
	lua: () => import('monaco-editor/esm/vs/basic-languages/lua/lua.contribution.js'),
	dockerfile: () =>
		import('monaco-editor/esm/vs/basic-languages/dockerfile/dockerfile.contribution.js')
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
