package service

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io/fs"
	"mime"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

var (
	ErrDesktopRuntimeUnavailable     = errors.New("desktop application runtime unavailable")
	ErrDesktopApplicationUnavailable = errors.New("application is not available for this file")
	ErrDesktopTerminalUnavailable    = errors.New("no supported terminal is installed")
	ErrDesktopLaunchFailed           = errors.New("desktop application could not be launched")
	ErrDesktopDefaultFailed          = errors.New("default application could not be changed")
	ErrDesktopTerminalLaunchFailed   = errors.New("terminal could not be launched")
)

// DesktopApplication is an installed desktop entry that can open a file.
type DesktopApplication struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Icon      string `json:"icon,omitempty"`
	IsDefault bool   `json:"isDefault"`
}

// DesktopAppsResult is returned by GET /api/v1/desktop/apps. Recommended apps are
// associated with the file type; other apps are every visible app that accepts files.
type DesktopAppsResult struct {
	MIMEType    string               `json:"mimeType"`
	Recommended []DesktopApplication `json:"recommended"`
	Other       []DesktopApplication `json:"other"`
}

// DesktopApps exposes the local desktop actions supported by the private app server.
type DesktopApps interface {
	Open(ctx context.Context, path string) error
	ListApps(ctx context.Context, path string) (DesktopAppsResult, error)
	OpenWith(ctx context.Context, path, appID string) error
	SetDefault(ctx context.Context, path, appID string) error
	Icon(ctx context.Context, appID string) (DesktopAppIcon, error)
	OpenTerminal(ctx context.Context, path string) error
}

// DesktopCommandRunner keeps OS process execution replaceable at the process boundary.
type DesktopCommandRunner interface {
	Output(ctx context.Context, name string, args ...string) ([]byte, error)
	Run(ctx context.Context, name string, args ...string) ([]byte, error)
	Start(name string, args []string, workingDirectory string) error
	LookPath(name string) (string, error)
}

type desktopAppsService struct {
	fs        filesystem.FS
	mounts    *mounts
	commands  DesktopCommandRunner
	defaultMu sync.Mutex
}

// NewDesktopAppsService creates the GIO/XDG-backed desktop integration.
// Pass nil for commands to use the local OS process runner.
func NewDesktopAppsService(fsys filesystem.FS, mountPoints []model.MountPoint, commands DesktopCommandRunner) DesktopApps {
	if commands == nil {
		commands = osDesktopCommandRunner{}
	}
	return &desktopAppsService{
		fs:       fsys,
		mounts:   newMounts(fsys, fixedMounts(mountPoints)),
		commands: commands,
	}
}

// Open hands an existing file or folder to the user's default application.
func (s *desktopAppsService) Open(ctx context.Context, path string) error {
	resolved, err := s.resolveExistingPath(path)
	if err != nil {
		return err
	}
	if output, err := s.commands.Run(ctx, "xdg-open", resolved); err != nil {
		return launchFailed(output, err)
	}
	return nil
}

func (s *desktopAppsService) ListApps(ctx context.Context, path string) (DesktopAppsResult, error) {
	resolved, err := s.resolveFile(path)
	if err != nil {
		return DesktopAppsResult{}, err
	}
	mimeType, err := s.detectMIMEType(ctx, resolved)
	if err != nil {
		return DesktopAppsResult{}, err
	}
	recommended, other := s.applicationsForMIMEType(mimeType)
	return DesktopAppsResult{
		MIMEType:    mimeType,
		Recommended: desktopApplications(recommended),
		Other:       desktopApplications(other),
	}, nil
}

func (s *desktopAppsService) OpenWith(ctx context.Context, path, appID string) error {
	app, resolved, _, err := s.applicationForFile(ctx, path, appID)
	if err != nil {
		return err
	}
	if output, err := s.commands.Run(ctx, "gio", "launch", app.path, resolved); err != nil {
		return launchFailed(output, err)
	}
	return nil
}

// SetDefault changes the user's association for the inspected MIME type, never an extension guessed by the renderer.
func (s *desktopAppsService) SetDefault(ctx context.Context, path, appID string) error {
	_, _, mimeType, err := s.applicationForFile(ctx, path, appID)
	if err != nil {
		return err
	}
	s.defaultMu.Lock()
	defer s.defaultMu.Unlock()
	if _, err := s.commands.Run(ctx, "gio", "mime", mimeType, appID); err != nil {
		return fmt.Errorf("%w: %v", ErrDesktopDefaultFailed, err)
	}
	if err := updateDesktopMIMEOverride(mimeType, appID); err != nil {
		return fmt.Errorf("%w: %v", ErrDesktopDefaultFailed, err)
	}
	output, err := s.commands.Output(ctx, "gio", "mime", mimeType)
	firstLine, _, _ := strings.Cut(string(output), "\n")
	if err != nil || !strings.HasSuffix(firstLine, ": "+appID) {
		return fmt.Errorf("%w: the desktop did not confirm the selected default", ErrDesktopDefaultFailed)
	}
	return nil
}

func (s *desktopAppsService) applicationForFile(ctx context.Context, path, appID string) (installedDesktopApp, string, string, error) {
	if !validDesktopID(appID) {
		return installedDesktopApp{}, "", "", ErrDesktopApplicationUnavailable
	}
	resolved, err := s.resolveFile(path)
	if err != nil {
		return installedDesktopApp{}, "", "", err
	}
	mimeType, err := s.detectMIMEType(ctx, resolved)
	if err != nil {
		return installedDesktopApp{}, "", "", err
	}
	// Only an entry the list could have offered is launched; the renderer never supplies a command.
	recommended, other := s.applicationsForMIMEType(mimeType)
	for _, app := range append(recommended, other...) {
		if app.application.ID != appID {
			continue
		}
		return app, resolved, mimeType, nil
	}
	return installedDesktopApp{}, "", "", ErrDesktopApplicationUnavailable
}

func (s *desktopAppsService) OpenTerminal(ctx context.Context, path string) error {
	resolved, err := s.resolveExistingPath(path)
	if err != nil {
		return err
	}
	isDirectory, err := s.fs.IsDir(resolved)
	if err != nil {
		return fmt.Errorf("%w: %v", ErrPathNotFound, err)
	}
	if !isDirectory {
		resolved = filepath.Dir(resolved)
	}

	terminal, err := s.findTerminal()
	if err != nil {
		return err
	}
	args := terminalArguments(terminal.name, resolved)
	if err := s.commands.Start(terminal.path, args, resolved); err != nil {
		return fmt.Errorf("%w: %v", ErrDesktopTerminalLaunchFailed, err)
	}
	return nil
}

func launchFailed(output []byte, err error) error {
	detail := strings.TrimSpace(string(output))
	if detail == "" {
		detail = err.Error()
	}
	return fmt.Errorf("%w: %s", ErrDesktopLaunchFailed, detail)
}

func (s *desktopAppsService) resolveFile(path string) (string, error) {
	resolved, err := s.resolveExistingPath(path)
	if err != nil {
		return "", err
	}
	isDirectory, err := s.fs.IsDir(resolved)
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrPathNotFound, err)
	}
	if isDirectory {
		return "", ErrNotFile
	}
	return resolved, nil
}

func (s *desktopAppsService) resolveExistingPath(path string) (string, error) {
	_, resolved, err := s.mounts.resolve(path, readExisting)
	return resolved, err
}

func (s *desktopAppsService) detectMIMEType(ctx context.Context, path string) (string, error) {
	output, err := s.commands.Output(ctx, "gio", "info", "--attributes=standard::content-type", path)
	if err != nil {
		return "", fmt.Errorf("%w: GIO could not inspect the file: %v", ErrDesktopRuntimeUnavailable, err)
	}
	const contentTypePrefix = "standard::content-type:"
	for _, line := range strings.Split(string(output), "\n") {
		value, ok := strings.CutPrefix(strings.TrimSpace(line), contentTypePrefix)
		if !ok {
			continue
		}
		mediaType, _, parseErr := mime.ParseMediaType(strings.TrimSpace(value))
		if parseErr == nil {
			return mediaType, nil
		}
	}
	return "", fmt.Errorf("%w: GIO returned no usable content type", ErrDesktopRuntimeUnavailable)
}

func desktopApplications(installed []installedDesktopApp) []DesktopApplication {
	apps := make([]DesktopApplication, len(installed))
	for i, app := range installed {
		apps[i] = app.application
	}
	return apps
}

type installedDesktopApp struct {
	application DesktopApplication
	path        string
	mimeTypes   []string
	// NoDisplay entries are handler-only: offered when associated, never in the full list.
	noDisplay    bool
	acceptsFiles bool
}

// applicationsForMIMEType returns associated apps first, then every other visible app
// whose Exec line takes a file or URL, sorted by name.
func (s *desktopAppsService) applicationsForMIMEType(mimeType string) ([]installedDesktopApp, []installedDesktopApp) {
	apps := readInstalledDesktopApps(s.commands)
	recommended := recommendedDesktopApps(apps, mimeType)
	offered := make(map[string]struct{}, len(recommended))
	for _, app := range recommended {
		offered[app.application.ID] = struct{}{}
	}
	var other []string
	for id, app := range apps {
		if _, exists := offered[id]; !exists && !app.noDisplay && app.acceptsFiles {
			other = append(other, id)
		}
	}
	sortDesktopIDsByName(other, apps)
	others := make([]installedDesktopApp, len(other))
	for i, id := range other {
		others[i] = apps[id]
	}
	return recommended, others
}

func sortDesktopIDsByName(ids []string, apps map[string]installedDesktopApp) {
	sort.Slice(ids, func(i, j int) bool {
		left := strings.ToLower(apps[ids[i]].application.Name)
		right := strings.ToLower(apps[ids[j]].application.Name)
		if left == right {
			return ids[i] < ids[j]
		}
		return left < right
	})
}

func recommendedDesktopApps(apps map[string]installedDesktopApp, mimeType string) []installedDesktopApp {
	associations := readMIMEAssociations(mimeType, mimeAppsFiles())
	orderedIDs := make([]string, 0, len(apps))
	seen := make(map[string]struct{}, len(apps))
	appendID := func(id string) {
		if !validDesktopID(id) {
			return
		}
		if app, exists := apps[id]; !exists || !app.acceptsFiles {
			return
		}
		if _, exists := seen[id]; exists {
			return
		}
		seen[id] = struct{}{}
		orderedIDs = append(orderedIDs, id)
	}
	for _, id := range associations.defaults {
		appendID(id)
	}
	for _, id := range associations.added {
		if _, removed := associations.removed[id]; !removed {
			appendID(id)
		}
	}

	var matching []string
	for id, app := range apps {
		if _, removed := associations.removed[id]; removed {
			continue
		}
		if matchesMIMEType(app.mimeTypes, mimeType) {
			matching = append(matching, id)
		}
	}
	sortDesktopIDsByName(matching, apps)
	for _, id := range matching {
		appendID(id)
	}

	defaultID := ""
	for _, id := range associations.defaults {
		if app, exists := apps[id]; exists && app.acceptsFiles {
			defaultID = id
			break
		}
	}
	result := make([]installedDesktopApp, 0, len(orderedIDs))
	for _, id := range orderedIDs {
		app := apps[id]
		app.application.IsDefault = id == defaultID
		result = append(result, app)
	}
	return result
}

func readInstalledDesktopApps(commands DesktopCommandRunner) map[string]installedDesktopApp {
	applications := make(map[string]installedDesktopApp)
	seen := make(map[string]struct{})
	for _, directory := range xdgApplicationDirectories() {
		_ = filepath.WalkDir(directory, func(path string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return nil
			}
			if entry.IsDir() {
				return nil
			}
			if filepath.Ext(entry.Name()) != ".desktop" {
				return nil
			}
			info, err := os.Stat(path)
			if err != nil || !info.Mode().IsRegular() {
				return nil
			}
			id, ok := desktopIDForPath(directory, path)
			if !ok {
				return nil
			}
			if _, exists := seen[id]; exists {
				return nil
			}
			app, hidden, err := readDesktopApplication(path, id, commands)
			if err != nil {
				return nil
			}
			seen[id] = struct{}{}
			if !hidden && app.application.ID != "" {
				applications[id] = app
			}
			return nil
		})
	}
	return applications
}

func readDesktopApplication(path, id string, commands DesktopCommandRunner) (installedDesktopApp, bool, error) {
	file, err := os.Open(path)
	if err != nil {
		return installedDesktopApp{}, false, err
	}
	defer file.Close()

	values := make(map[string]string)
	inDesktopEntry := false
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 1024), 1024*1024)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			inDesktopEntry = line == "[Desktop Entry]"
			continue
		}
		if !inDesktopEntry || line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if ok {
			values[key] = unescapeDesktopValue(value)
		}
	}
	if err := scanner.Err(); err != nil {
		return installedDesktopApp{}, false, err
	}
	if values["Hidden"] == "true" {
		return installedDesktopApp{}, true, nil
	}
	if values["Type"] != "Application" {
		return installedDesktopApp{}, false, nil
	}
	exec := values["Exec"]
	if exec == "" && values["DBusActivatable"] != "true" {
		return installedDesktopApp{}, false, nil
	}
	// GIO refuses an Exec line it cannot split, so the entry could never launch.
	if exec != "" && !validDesktopExec(exec) {
		return installedDesktopApp{}, false, nil
	}
	if tryExec := values["TryExec"]; tryExec != "" {
		if _, err := commands.LookPath(tryExec); err != nil {
			return installedDesktopApp{}, false, nil
		}
	}
	name := values["Name"]
	if name == "" {
		name = strings.TrimSuffix(id, ".desktop")
	}
	app := DesktopApplication{ID: id, Name: name, Icon: values["Icon"]}
	return installedDesktopApp{
		application:  app,
		path:         path,
		mimeTypes:    strings.Split(values["MimeType"], ";"),
		noDisplay:    values["NoDisplay"] == "true",
		acceptsFiles: values["DBusActivatable"] == "true" || execAcceptsFiles(exec),
	}, false, nil
}

// validDesktopExec rejects the Exec lines GIO cannot split into arguments: an unterminated
// quote or a trailing escape. Such an entry could never launch.
func validDesktopExec(exec string) bool {
	var quote byte
	for i := 0; i < len(exec); i++ {
		switch character := exec[i]; {
		case quote == '\'':
			if character == '\'' {
				quote = 0
			}
		case character == '\\':
			if i+1 >= len(exec) {
				return false
			}
			i++
		case character == '"' || character == '\'':
			if quote == character {
				quote = 0
			} else if quote == 0 {
				quote = character
			}
		}
	}
	return quote == 0 && strings.TrimSpace(exec) != ""
}

// execAcceptsFiles reports whether GIO will pass the opened file to the command.
func execAcceptsFiles(exec string) bool {
	exec = strings.ReplaceAll(exec, "%%", "")
	for _, code := range []string{"%f", "%F", "%u", "%U"} {
		if strings.Contains(exec, code) {
			return true
		}
	}
	return false
}

func desktopIDForPath(root, path string) (string, bool) {
	relative, err := filepath.Rel(root, path)
	if err != nil || filepath.IsAbs(relative) || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", false
	}
	id := strings.ReplaceAll(relative, string(filepath.Separator), "-")
	return id, validDesktopID(id)
}

func validDesktopID(id string) bool {
	return id != "" && filepath.Base(id) == id && !strings.ContainsAny(id, "/\\\x00\r\n;") && strings.HasSuffix(id, ".desktop")
}

func unescapeDesktopValue(value string) string {
	return strings.NewReplacer(`\s`, " ", `\n`, "\n", `\t`, "\t", `\r`, "\r", `\\`, `\`).Replace(value)
}

func matchesMIMEType(registered []string, mimeType string) bool {
	for _, candidate := range registered {
		candidate = strings.TrimSpace(candidate)
		if candidate == mimeType {
			return true
		}
		if strings.HasSuffix(candidate, "/*") && strings.HasPrefix(mimeType, strings.TrimSuffix(candidate, "*")) {
			return true
		}
	}
	return false
}

type mimeTypeAssociations struct {
	defaults []string
	added    []string
	removed  map[string]struct{}
}

func mimeAppsFiles() []string {
	configDirectories := []string{xdgHomeDirectory("XDG_CONFIG_HOME", ".config")}
	configDirectories = append(configDirectories, xdgSearchDirectories("XDG_CONFIG_DIRS", "/etc/xdg")...)
	dataDirectories := xdgApplicationDirectories()
	files := make([]string, 0, len(configDirectories)+len(dataDirectories))
	seen := make(map[string]struct{})
	for _, directory := range append(configDirectories, dataDirectories...) {
		for _, desktop := range strings.Split(os.Getenv("XDG_CURRENT_DESKTOP"), ":") {
			desktop = strings.ToLower(desktop)
			if desktop != "" && !strings.ContainsAny(desktop, "/\\\x00") {
				appendUniquePath(&files, seen, filepath.Join(directory, desktop+"-mimeapps.list"))
			}
		}
		appendUniquePath(&files, seen, filepath.Join(directory, "mimeapps.list"))
	}
	return files
}

func readMIMEAssociations(mimeType string, files []string) mimeTypeAssociations {
	associations := mimeTypeAssociations{removed: make(map[string]struct{})}
	hasDefault := false
	for _, path := range files {
		file, err := os.Open(path)
		if err != nil {
			continue
		}
		section := ""
		scanner := bufio.NewScanner(file)
		scanner.Buffer(make([]byte, 1024), 1024*1024)
		for scanner.Scan() {
			line := strings.TrimSpace(scanner.Text())
			if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
				section = strings.TrimSuffix(strings.TrimPrefix(line, "["), "]")
				continue
			}
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			key, value, ok := strings.Cut(line, "=")
			if !ok || key != mimeType {
				continue
			}
			ids := desktopIDs(value)
			switch section {
			case "Default Applications":
				if !hasDefault && len(ids) > 0 {
					associations.defaults = ids
					hasDefault = true
				}
			case "Added Associations":
				associations.added = append(associations.added, ids...)
			case "Removed Associations":
				for _, id := range ids {
					associations.removed[id] = struct{}{}
				}
			}
		}
		_ = file.Close()
	}
	return associations
}

func desktopIDs(value string) []string {
	parts := strings.Split(value, ";")
	ids := make([]string, 0, len(parts))
	for _, part := range parts {
		id := strings.TrimSpace(part)
		if validDesktopID(id) {
			ids = append(ids, id)
		}
	}
	return ids
}

func xdgApplicationDirectories() []string {
	dataHome := xdgHomeDirectory("XDG_DATA_HOME", ".local/share")
	directories := []string{filepath.Join(dataHome, "applications")}
	for _, directory := range xdgSearchDirectories("XDG_DATA_DIRS", "/usr/local/share:/usr/share") {
		directories = append(directories, filepath.Join(directory, "applications"))
	}
	unique := make([]string, 0, len(directories))
	seen := make(map[string]struct{})
	for _, directory := range directories {
		appendUniquePath(&unique, seen, directory)
	}
	return unique
}

func xdgHomeDirectory(variable, fallbackSuffix string) string {
	if configured := os.Getenv(variable); filepath.IsAbs(configured) {
		return filepath.Clean(configured)
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(string(filepath.Separator), fallbackSuffix)
	}
	return filepath.Join(home, fallbackSuffix)
}

func xdgSearchDirectories(variable, fallback string) []string {
	value, exists := os.LookupEnv(variable)
	if !exists {
		value = fallback
	}
	var directories []string
	for _, directory := range filepath.SplitList(value) {
		if filepath.IsAbs(directory) {
			directories = append(directories, filepath.Clean(directory))
		}
	}
	return directories
}

func appendUniquePath(paths *[]string, seen map[string]struct{}, path string) {
	path = filepath.Clean(path)
	if !filepath.IsAbs(path) {
		return
	}
	if _, exists := seen[path]; exists {
		return
	}
	seen[path] = struct{}{}
	*paths = append(*paths, path)
}

type desktopTerminal struct {
	name string
	path string
}

func (s *desktopAppsService) findTerminal() (desktopTerminal, error) {
	if preferred := strings.TrimSpace(os.Getenv("TERMINAL")); preferred != "" && !strings.ContainsAny(preferred, " \t\r\n") {
		if path, err := s.commands.LookPath(preferred); err == nil {
			return desktopTerminal{name: filepath.Base(preferred), path: path}, nil
		}
	}
	for _, name := range []string{
		"x-terminal-emulator", "kgx", "gnome-terminal", "konsole", "xfce4-terminal", "mate-terminal",
		"tilix", "kitty", "alacritty", "wezterm", "foot", "xterm",
	} {
		if path, err := s.commands.LookPath(name); err == nil {
			return desktopTerminal{name: name, path: path}, nil
		}
	}
	return desktopTerminal{}, ErrDesktopTerminalUnavailable
}

func terminalArguments(name, directory string) []string {
	switch filepath.Base(name) {
	case "kgx", "gnome-terminal", "xfce4-terminal", "mate-terminal", "tilix":
		return []string{"--working-directory=" + directory}
	case "konsole":
		return []string{"--workdir", directory}
	case "kitty":
		return []string{"--directory", directory}
	case "alacritty":
		return []string{"--working-directory", directory}
	case "wezterm":
		return []string{"start", "--cwd", directory}
	case "foot":
		return []string{"--working-directory=" + directory}
	default:
		return nil
	}
}

type osDesktopCommandRunner struct{}

func (osDesktopCommandRunner) Output(ctx context.Context, name string, args ...string) ([]byte, error) {
	command := exec.CommandContext(ctx, name, args...)
	// GIO inspection output is parsed by the private desktop API.
	command.Env = append(os.Environ(), "LC_ALL=C")
	return command.Output()
}

func (osDesktopCommandRunner) Run(ctx context.Context, name string, args ...string) ([]byte, error) {
	return exec.CommandContext(ctx, name, args...).CombinedOutput()
}

func (osDesktopCommandRunner) Start(name string, args []string, workingDirectory string) error {
	command := exec.Command(name, args...)
	command.Dir = workingDirectory
	if err := command.Start(); err != nil {
		return err
	}
	go func() { _ = command.Wait() }()
	return nil
}

func (osDesktopCommandRunner) LookPath(name string) (string, error) {
	return exec.LookPath(name)
}
