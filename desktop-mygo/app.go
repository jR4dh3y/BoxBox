package main

import (
	"context"
	"log"

	"github.com/egoist/mygo"
)

// desktopApp owns the windows and the local server. Its methods run on the
// main thread. Closing the last window quits.
type desktopApp struct {
	server      *desktopServer
	drag        *gtkDrag
	accelerated bool
	ctx         context.Context

	main      *mygo.Window
	mainReady bool
	pending   []launchTarget // targets for the main page once it has loaded
	choosers  map[string]*mygo.Window
}

func newDesktopApp(ctx context.Context, server *desktopServer, drag *gtkDrag, accelerated bool) *desktopApp {
	return &desktopApp{server: server, drag: drag, accelerated: accelerated, ctx: ctx, choosers: map[string]*mygo.Window{}}
}

// handle carries out a launch request, this launch's or a later one's.
func (a *desktopApp) handle(request launchRequest) {
	switch request.kind {
	case requestPortal:
		// The server already serves the file chooser portal.
	case requestActivate:
		a.openMain(nil)
	case requestLocal:
		a.openMain(&request.target)
	case requestRemote:
		go func() {
			target, err := a.openRemote(request.remote)
			if err != nil {
				log.Printf("Could not open %s: %v", request.remote, err)
				return
			}
			mygo.RunOnMain(func() { a.openMain(&target) })
		}()
	}
}

// openMain shows the main window, opening target in a new tab of a page
// that is already there.
func (a *desktopApp) openMain(target *launchTarget) {
	if a.main != nil && !a.main.IsDestroyed() {
		a.main.Show()
		a.main.Focus()
		if target != nil {
			a.pending = append(a.pending, *target)
			if a.mainReady {
				a.deliverTargets()
			}
		}
		return
	}
	win := a.newWindow("BoxBox", a.server.bootstrapURL(browsePath(target)), "main")
	a.main, a.mainReady, a.pending = win, false, nil
	win.Page().OnDidFinishLoad(func() {
		if a.main != win {
			return
		}
		a.mainReady = true
		a.deliverTargets()
	})
	win.OnClosed(func() {
		if a.main == win {
			a.main, a.mainReady, a.pending = nil, false, nil
		}
	})
}

func (a *desktopApp) deliverTargets() {
	win := a.main
	for _, target := range a.pending {
		script := openTargetScript(a.server.origin, target)
		go func() {
			if _, err := win.Page().Eval(script); err != nil {
				log.Printf("Could not open a launch target: %v", err)
			}
		}()
	}
	a.pending = nil
}

func (a *desktopApp) newWindow(title, url, stateKey string) *mygo.Window {
	win := mygo.NewWindow(mygo.WindowOptions{
		Title:           title,
		URL:             url,
		Width:           1280,
		Height:          800,
		MinWidth:        640,
		MinHeight:       420,
		StateKey:        stateKey,
		BackgroundColor: "light-dark(#ffffff, #181818)",
		Page: mygo.PageOptions{
			PreloadScript:  renderingScript(a.server.origin, a.accelerated) + dragScript(a.server.origin) + paintProbeScript(),
			TrustedOrigins: []string{a.server.origin},
		},
	})
	win.Page().OnDidFailLoad(func(err *mygo.LoadError) {
		log.Printf("BoxBox page failed to load: %v", err)
	})
	attachDrag(win, a.drag)
	return win
}
