package main

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"

	"github.com/egoist/mygo"
)

const maxPreferencesBytes = 128 << 10

// hardwareAcceleration reads the saved Appearance choice. It applies before
// GTK starts, so the server cannot be asked for it.
func hardwareAcceleration(dataDir string) bool {
	file, err := os.Open(filepath.Join(dataDir, "desktop", "preferences.json"))
	if err != nil {
		return true
	}
	defer file.Close()
	var saved struct {
		HardwareAcceleration *bool `json:"hardwareAcceleration"`
	}
	if err := json.NewDecoder(io.LimitReader(file, maxPreferencesBytes)).Decode(&saved); err != nil {
		log.Print("Could not read the rendering preference; using hardware acceleration")
		return true
	}
	return saved.HardwareAcceleration == nil || *saved.HardwareAcceleration
}

// renderer maps the choice onto WebKitGTK. Without its DMA-BUF renderer pages
// draw on the CPU and backdrop-filter, which frosted glass needs, is not drawn.
func renderer(accelerated bool) mygo.DMABUFRenderer {
	if accelerated {
		return mygo.DMABUFRendererOn
	}
	return mygo.DMABUFRendererOff
}

// renderingScript tells Appearance settings which mode this launch runs in,
// so it can say when a changed choice needs a restart.
func renderingScript(origin string, accelerated bool) string {
	originJSON, _ := json.Marshal(origin)
	return fmt.Sprintf(`(() => {
	if (location.origin !== %s) return;
	const apply = () => { document.documentElement.dataset.hardwareAcceleration = '%t'; };
	if (document.documentElement) apply();
	else document.addEventListener('DOMContentLoaded', apply, { once: true });
})();`, originJSON, accelerated)
}
