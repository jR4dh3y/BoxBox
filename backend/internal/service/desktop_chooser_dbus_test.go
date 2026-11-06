package service

import (
	"context"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"

	"github.com/godbus/dbus/v5"
)

// Re-exec under a fresh bus so the test never claims a name on the user's session.
func TestChooserDBusProtocol(t *testing.T) {
	if os.Getenv("BOXBOX_CHOOSER_DBUS_TEST") != "1" {
		command, err := exec.LookPath("dbus-run-session")
		if err != nil {
			t.Skip("dbus-run-session unavailable")
		}
		binary, err := os.Executable()
		if err != nil {
			t.Fatal(err)
		}
		child := exec.Command(command, "--", binary, "-test.run=^TestChooserDBusProtocol$", "-test.v")
		child.Env = append(os.Environ(), "BOXBOX_CHOOSER_DBUS_TEST=1")
		output, err := child.CombinedOutput()
		if err != nil {
			t.Fatalf("isolated D-Bus: %v\n%s", err, output)
		}
		return
	}
	c, root := chooserFixture(t)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	close, err := c.StartPortal(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer close()
	frontend, err := dbus.ConnectSessionBus()
	if err != nil {
		t.Fatal(err)
	}
	defer frontend.Close()
	if _, err := frontend.RequestName("org.freedesktop.portal.Desktop", dbus.NameFlagDoNotQueue); err != nil {
		t.Fatal(err)
	}
	attacker, err := dbus.ConnectSessionBus()
	if err != nil {
		t.Fatal(err)
	}
	defer attacker.Close()
	handle := dbus.ObjectPath("/org/freedesktop/portal/desktop/request/test/chooser")
	options := map[string]dbus.Variant{"current_folder": dbus.MakeVariant(append([]byte(root), 0)), "choices": dbus.MakeVariant([]struct {
		ID      string
		Label   string
		Options []ChooserOption
		Value   string
	}{{"readonly", "Read only", []ChooserOption{}, "true"}})}
	if err := attacker.Object(chooserBus, chooserObject).Call(chooserInterface+".OpenFile", 0, handle, "test.app", "", "Open", options).Err; err == nil {
		t.Fatal("direct untrusted peer accepted")
	}
	call := frontend.Object(chooserBus, chooserObject).Go(chooserInterface+".OpenFile", 0, make(chan *dbus.Call, 1), handle, "test.app", "", "Open", options)
	request := waitChooser(t, c)
	if err := c.Select(ctx, request.ID, ChooserSelection{Paths: []string{"fixture/Report.txt"}, Choices: map[string]string{"readonly": "true"}}); err != nil {
		t.Fatal(err)
	}
	var code uint32
	var results map[string]dbus.Variant
	select {
	case done := <-call.Done:
		if err := done.Store(&code, &results); err != nil {
			t.Fatal(err)
		}
		raw, ok := done.Body[1].(map[string]dbus.Variant)
		if !ok {
			t.Fatal("invalid result body")
		}
		results = raw
	case <-time.After(3 * time.Second):
		t.Fatal("selection reply timeout")
	}
	if code != 0 || results["uris"].Signature().String() != "as" || results["choices"].Signature().String() != "a(ss)" {
		t.Fatalf("incorrect result signature: %d %#v", code, results)
	}
	call = frontend.Object(chooserBus, chooserObject).Go(chooserInterface+".OpenFile", 0, make(chan *dbus.Call, 1), handle, "test.app", "", "Open", map[string]dbus.Variant{})
	waitChooser(t, c)
	// Begin wakes the renderer before Export completes; wait for the real Request object.
	deadline := time.Now().Add(3 * time.Second)
	for {
		err := frontend.Object(chooserBus, handle).Call("org.freedesktop.impl.portal.Request.Close", 0).Err
		if err == nil {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal(err)
		}
		time.Sleep(time.Millisecond)
	}
	select {
	case done := <-call.Done:
		if err := done.Store(&code, &results); err != nil {
			t.Fatal(err)
		}
		if code != 1 {
			t.Fatal(code)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("cancel reply timeout")
	}
	var xml string
	if err := frontend.Object(chooserBus, chooserObject).Call("org.freedesktop.DBus.Introspectable.Introspect", 0).Store(&xml); err != nil || !strings.Contains(xml, "SaveFiles") {
		t.Fatalf("introspection: %v %s", err, xml)
	}
}
func waitChooser(t *testing.T, c *DesktopChooser) ChooserRequest {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		list, changed := c.List()
		if len(list) > 0 {
			return list[0]
		}
		select {
		case <-changed:
		case <-time.After(time.Second):
		}
	}
	t.Fatal("request did not reach chooser")
	return ChooserRequest{}
}
