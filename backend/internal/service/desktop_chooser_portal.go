package service

import (
	"context"
	"fmt"
	"mime"
	"net/http"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"unicode/utf8"

	"github.com/godbus/dbus/v5"
	"github.com/godbus/dbus/v5/introspect"
	"github.com/godbus/dbus/v5/prop"
)

const chooserInterface = "org.freedesktop.impl.portal.FileChooser"
const chooserBus = "org.freedesktop.impl.portal.desktop.boxbox"
const chooserObject = dbus.ObjectPath("/org/freedesktop/portal/desktop")

type chooserPortal struct {
	conn    *dbus.Conn
	chooser *DesktopChooser
	ctx     context.Context
}
type chooserPortalRequest struct {
	portal *chooserPortal
	id     string
}

// StartPortal exports only FileChooser. The portal frontend remains responsible for sandbox grants.
func (c *DesktopChooser) StartPortal(ctx context.Context) (func(), error) {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return nil, err
	}
	p := &chooserPortal{conn: conn, chooser: c, ctx: ctx}
	if err = conn.Export(p, chooserObject, chooserInterface); err != nil {
		conn.Close()
		return nil, err
	}
	_, err = prop.Export(conn, chooserObject, prop.Map{chooserInterface: {"version": {Value: uint32(4), Writable: false, Emit: prop.EmitFalse}}})
	if err != nil {
		conn.Close()
		return nil, err
	}
	node := &introspect.Node{Interfaces: []introspect.Interface{{Name: chooserInterface, Methods: introspect.Methods(p)}, prop.IntrospectData}}
	if err = conn.Export(introspect.NewIntrospectable(node), chooserObject, "org.freedesktop.DBus.Introspectable"); err != nil {
		conn.Close()
		return nil, err
	}
	reply, err := conn.RequestName(chooserBus, dbus.NameFlagDoNotQueue)
	if err != nil || reply != dbus.RequestNameReplyPrimaryOwner {
		conn.Close()
		return nil, fmt.Errorf("BoxBox portal name unavailable: %v", err)
	}
	return func() { c.Close(); conn.Close() }, nil
}
func (p *chooserPortal) authorized(sender dbus.Sender) bool {
	var owner string
	err := p.conn.BusObject().Call("org.freedesktop.DBus.GetNameOwner", 0, "org.freedesktop.portal.Desktop").Store(&owner)
	return err == nil && owner == string(sender)
}
func (p *chooserPortal) OpenFile(sender dbus.Sender, handle dbus.ObjectPath, appID, parent, title string, options map[string]dbus.Variant) (uint32, map[string]dbus.Variant, *dbus.Error) {
	return p.run(sender, handle, appID, title, "open", options)
}
func (p *chooserPortal) SaveFile(sender dbus.Sender, handle dbus.ObjectPath, appID, parent, title string, options map[string]dbus.Variant) (uint32, map[string]dbus.Variant, *dbus.Error) {
	return p.run(sender, handle, appID, title, "save", options)
}
func (p *chooserPortal) SaveFiles(sender dbus.Sender, handle dbus.ObjectPath, appID, parent, title string, options map[string]dbus.Variant) (uint32, map[string]dbus.Variant, *dbus.Error) {
	return p.run(sender, handle, appID, title, "save-files", options)
}
func (r *chooserPortalRequest) Close(sender dbus.Sender) *dbus.Error {
	if !r.portal.authorized(sender) {
		return dbus.NewError("org.freedesktop.DBus.Error.AccessDenied", nil)
	}
	r.portal.chooser.Cancel(r.id)
	return nil
}
func (p *chooserPortal) run(sender dbus.Sender, handle dbus.ObjectPath, appID, title, kind string, options map[string]dbus.Variant) (uint32, map[string]dbus.Variant, *dbus.Error) {
	if !p.authorized(sender) {
		return 2, nil, dbus.NewError("org.freedesktop.DBus.Error.AccessDenied", nil)
	}
	if !handle.IsValid() || !strings.HasPrefix(string(handle), "/org/freedesktop/portal/desktop/request/") {
		return 2, nil, dbus.MakeFailedError(fmt.Errorf("invalid request handle"))
	}
	request, err := p.parse(kind, appID, title, options)
	if err != nil {
		return 2, nil, dbus.MakeFailedError(err)
	}
	list, _ := p.chooser.List()
	if len(list) >= 16 {
		return 2, nil, dbus.MakeFailedError(fmt.Errorf("too many file choosers"))
	}
	request, result := p.chooser.Begin(request)
	if err := p.conn.Export(&chooserPortalRequest{portal: p, id: request.ID}, handle, "org.freedesktop.impl.portal.Request"); err != nil {
		p.chooser.Cancel(request.ID)
		return 2, nil, dbus.MakeFailedError(err)
	}
	defer p.conn.Export(nil, handle, "org.freedesktop.impl.portal.Request")
	select {
	case value := <-result:
		results := map[string]dbus.Variant{}
		if value.Code == 0 {
			results["uris"] = dbus.MakeVariant(value.URIs)
			results["choices"] = dbus.MakeVariant(value.Choices)
			if value.Filter != nil {
				results["current_filter"] = dbus.MakeVariant(*value.Filter)
			}
		}
		return value.Code, results, nil
	case <-p.ctx.Done():
		p.chooser.Cancel(request.ID)
		return 1, map[string]dbus.Variant{}, nil
	}
}
func portalOption(options map[string]dbus.Variant, key string, destination interface{}) error {
	if value, ok := options[key]; ok {
		if err := dbus.Store([]interface{}{value.Value()}, destination); err != nil {
			return fmt.Errorf("invalid %s: %w", key, err)
		}
	}
	return nil
}
func portalPath(options map[string]dbus.Variant, key string) (string, error) {
	var value []byte
	if err := portalOption(options, key, &value); err != nil {
		return "", err
	}
	if len(value) == 0 {
		return "", nil
	}
	if value[len(value)-1] != 0 || strings.IndexByte(string(value[:len(value)-1]), 0) >= 0 || !utf8.Valid(value[:len(value)-1]) {
		return "", fmt.Errorf("%s must be a UTF-8 NUL-terminated local path", key)
	}
	return string(value[:len(value)-1]), nil
}
func chooserName(value string) bool {
	return value != "" && value != "." && value != ".." && !strings.ContainsAny(value, "/\x00") && utf8.ValidString(value)
}
func (p *chooserPortal) parse(kind, appID, title string, options map[string]dbus.Variant) (ChooserRequest, error) {
	r := ChooserRequest{Kind: kind, AppID: appID, Title: title, AcceptLabel: "Open", Files: []string{}, Filters: []ChooserFilter{}, Choices: []ChooserChoice{}}
	if kind != "open" {
		r.AcceptLabel = "Save"
	}
	for key, destination := range map[string]interface{}{"accept_label": &r.AcceptLabel, "multiple": &r.Multiple, "directory": &r.Directory, "filters": &r.Filters} {
		if err := portalOption(options, key, destination); err != nil {
			return r, err
		}
	}
	if kind != "open" {
		r.Multiple = false
		r.Directory = kind == "save-files"
	}
	if len(r.Filters) > 100 {
		return r, fmt.Errorf("too many filters")
	}
	for _, filter := range r.Filters {
		if len(filter.Rules) > 100 {
			return r, fmt.Errorf("too many filter rules")
		}
		for _, rule := range filter.Rules {
			if rule.Kind > 1 || len(rule.Value) > 1024 {
				return r, fmt.Errorf("invalid filter rule")
			}
		}
	}
	if current, ok := options["current_filter"]; ok {
		var filter ChooserFilter
		if err := dbus.Store([]interface{}{current.Value()}, &filter); err != nil {
			return r, err
		}
		found := false
		for i, f := range r.Filters {
			if reflect.DeepEqual(f, filter) {
				r.Filter = i
				found = true
				break
			}
		}
		if !found {
			r.Filters = append(r.Filters, filter)
			r.Filter = len(r.Filters) - 1
		}
	}
	folder, err := portalPath(options, "current_folder")
	if err != nil {
		return r, err
	}
	file, err := portalPath(options, "current_file")
	if err != nil {
		return r, err
	}
	if kind == "save" && file != "" {
		folder = filepath.Dir(file)
		r.Name = filepath.Base(file)
	}
	if err := portalOption(options, "current_name", &r.Name); err != nil {
		return r, err
	}
	if r.Name != "" && !chooserName(r.Name) {
		return r, fmt.Errorf("invalid suggested file name")
	}
	r.Folder = p.chooser.Folder(folder)
	if kind == "save-files" {
		var names [][]byte
		if err := portalOption(options, "files", &names); err != nil {
			return r, err
		}
		if len(names) == 0 || len(names) > 1024 {
			return r, fmt.Errorf("invalid save file count")
		}
		seen := map[string]bool{}
		for _, name := range names {
			if len(name) == 0 || name[len(name)-1] != 0 {
				return r, fmt.Errorf("invalid save filename")
			}
			text := string(name[:len(name)-1])
			if !chooserName(text) || seen[text] {
				return r, fmt.Errorf("invalid or duplicate save filename")
			}
			seen[text] = true
			r.Files = append(r.Files, text)
		}
	}
	var choices []struct {
		ID      string
		Label   string
		Options []ChooserOption
		Value   string
	}
	if err := portalOption(options, "choices", &choices); err != nil {
		return r, err
	}
	if len(choices) > 100 {
		return r, fmt.Errorf("too many choices")
	}
	for _, choice := range choices {
		if choice.Value == "" {
			if len(choice.Options) > 0 {
				choice.Value = choice.Options[0].ID
			} else {
				choice.Value = "false"
			}
		}
		r.Choices = append(r.Choices, ChooserChoice{ID: choice.ID, Label: choice.Label, Options: choice.Options, Value: choice.Value})
	}
	return r, nil
}
func chooserMatches(filter ChooserFilter, name, path string) bool {
	for _, rule := range filter.Rules {
		if rule.Kind == 0 {
			if match, _ := filepath.Match(rule.Value, name); match {
				return true
			}
		} else {
			content := mime.TypeByExtension(filepath.Ext(name))
			if content == "" {
				if f, err := os.Open(path); err == nil {
					data := make([]byte, 512)
					n, _ := f.Read(data)
					f.Close()
					content = http.DetectContentType(data[:n])
				}
			}
			content, _, _ = mime.ParseMediaType(content)
			if content == rule.Value || strings.HasSuffix(rule.Value, "/*") && strings.HasPrefix(content, strings.TrimSuffix(rule.Value, "*")) {
				return true
			}
		}
	}
	return len(filter.Rules) == 0
}
