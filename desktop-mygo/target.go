package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"
)

// launchTarget is the folder a window opens and the item it selects there,
// both as virtual paths under the root mount.
type launchTarget struct {
	Path   string `json:"path"`
	Select string `json:"select,omitempty"`
}

type requestKind int

const (
	requestActivate requestKind = iota // show the main window
	requestPortal                      // start for the file chooser, without a window
	requestLocal                       // open target
	requestRemote                      // open the saved deployment at remote
)

// launchRequest is what one launch asks the running app to do.
type launchRequest struct {
	kind   requestKind
	target launchTarget
	remote string
}

// launchOptions is a parsed command line.
type launchOptions struct {
	request launchRequest
	server  string // development: the server binary
	dataDir string
	chooser string // "enable" or "disable" the file chooser routing
}

// parseLaunch reads the command line:
//
//	boxbox [folder | file] [--select file] [--remote https://…]
//	boxbox --portal | --enable-chooser | --disable-chooser
//
// A file opens its folder and selects it. BOXBOX_DESKTOP_URL is the remote
// deployment when --remote is not given; a remote wins over a local target.
func parseLaunch(args []string, cwd, remoteEnv string) (launchOptions, error) {
	var opts launchOptions
	var paths []string
	selected, remote, portal := "", remoteEnv, false
	for i := 0; i < len(args); i++ {
		name, value, hasValue := strings.Cut(args[i], "=")
		next := func() (string, error) {
			if hasValue {
				return value, nil
			}
			if i+1 >= len(args) {
				return "", fmt.Errorf("%s needs a value", name)
			}
			i++
			return args[i], nil
		}
		var err error
		switch name {
		case "--select", "-select":
			selected, err = next()
		case "--remote", "-remote":
			remote, err = next()
		case "--server", "-server":
			opts.server, err = next()
		case "--data-dir", "-data-dir":
			opts.dataDir, err = next()
		case "--portal":
			portal = true
		case "--enable-chooser":
			opts.chooser = "enable"
		case "--disable-chooser":
			opts.chooser = "disable"
		case "--":
			paths = append(paths, args[i+1:]...)
			i = len(args)
		default:
			if strings.HasPrefix(args[i], "-") {
				return opts, fmt.Errorf("unknown option %s", args[i])
			}
			paths = append(paths, args[i])
		}
		if err != nil {
			return opts, err
		}
	}
	switch {
	case portal:
		opts.request = launchRequest{kind: requestPortal}
	case remote != "":
		u, err := remoteURL(remote)
		if err != nil {
			return opts, err
		}
		opts.request = launchRequest{kind: requestRemote, remote: u}
	default:
		// File managers may pass several selected items; the first opens.
		first := ""
		if len(paths) > 0 {
			first = paths[0]
		}
		target, err := parseLaunchTarget(first, selected, cwd)
		if err != nil {
			return opts, err
		}
		if target != nil {
			opts.request = launchRequest{kind: requestLocal, target: *target}
		}
	}
	return opts, nil
}

// remoteURL accepts an HTTPS deployment URL without credentials, query or fragment.
func remoteURL(value string) (string, error) {
	u, err := url.Parse(value)
	if err != nil || u.Scheme != "https" || u.Host == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" {
		return "", errors.New("use an HTTPS BoxBox deployment URL without credentials, a query or a fragment")
	}
	return u.String(), nil
}

func virtualPath(local string) string {
	if local == "/" {
		return "root"
	}
	return "root" + local
}

// localPath accepts a local path or a file URL. Other schemes are rejected, so
// a launch argument cannot name a remote resource.
func localPath(argument, cwd string) (string, error) {
	path := argument
	if scheme, _, found := strings.Cut(argument, ":"); found && !strings.Contains(scheme, "/") && scheme != "" {
		parsed, err := url.Parse(argument)
		if err != nil {
			return "", fmt.Errorf("invalid file URL %q", argument)
		}
		if parsed.Scheme != "file" || (parsed.Host != "" && !strings.EqualFold(parsed.Host, "localhost")) ||
			parsed.User != nil || parsed.RawQuery != "" || parsed.Fragment != "" {
			return "", errors.New("only local paths and file:// URLs can be opened")
		}
		path = parsed.Path
	}
	if strings.ContainsRune(path, 0) {
		return "", errors.New("launch paths cannot contain NUL")
	}
	if !filepath.IsAbs(path) {
		path = filepath.Join(cwd, path)
	}
	return filepath.Clean(path), nil
}

// parseLaunchTarget resolves a folder or file, and a file to select, to a target.
func parseLaunchTarget(folder, selected, cwd string) (*launchTarget, error) {
	if folder == "" && selected == "" {
		return nil, nil
	}
	target := &launchTarget{}
	if folder != "" {
		path, err := localPath(folder, cwd)
		if err != nil {
			return nil, err
		}
		info, err := os.Stat(path)
		if err != nil {
			return nil, err
		}
		if info.IsDir() {
			target.Path = virtualPath(path)
		} else {
			target.Path, target.Select = virtualPath(filepath.Dir(path)), virtualPath(path)
		}
	}
	if selected != "" {
		path, err := localPath(selected, cwd)
		if err != nil {
			return nil, err
		}
		if info, err := os.Stat(path); err != nil {
			return nil, err
		} else if info.IsDir() {
			return nil, errors.New("--select expects a file, not a folder")
		}
		directory := virtualPath(filepath.Dir(path))
		if target.Path != "" && target.Path != directory {
			return nil, errors.New("the selected file must be directly inside the requested folder")
		}
		target.Path, target.Select = directory, virtualPath(path)
	}
	return target, nil
}

// browsePath is the route the Svelte app reads for its first folder and selection.
func browsePath(target *launchTarget) string {
	if target == nil {
		return "/browse"
	}
	query := url.Values{"path": {target.Path}}
	if target.Select != "" {
		query.Set("select", target.Select)
	}
	return "/browse?" + query.Encode()
}

// openTargetScript hands a target to the running page, which opens it in a
// new tab and keeps its other tabs.
func openTargetScript(origin string, target launchTarget) string {
	originJSON, _ := json.Marshal(origin)
	targetJSON, _ := json.Marshal(target)
	return fmt.Sprintf(`if (location.origin === %s && document.querySelector('meta[name="boxbox-desktop"]')) {
	if (!Array.isArray(window.boxboxDesktopTargets)) window.boxboxDesktopTargets = [];
	window.boxboxDesktopTargets.push(%s);
	window.dispatchEvent(new Event('boxbox-desktop-open'));
}
return null;`, originJSON, targetJSON)
}
