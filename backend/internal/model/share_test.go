package model

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestSharePermissionsAcceptLegacyWriteWithoutGrantingDelete(t *testing.T) {
	var permissions SharePermissions
	if err := json.Unmarshal([]byte(`{"view":true,"download":true,"write":true}`), &permissions); err != nil {
		t.Fatal(err)
	}
	if !permissions.Upload || permissions.Delete || !permissions.LegacyReplace {
		t.Fatalf("legacy permissions = %+v", permissions)
	}

	data, err := json.Marshal(permissions)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(data), `"write"`) || strings.Contains(string(data), `legacyReplace`) {
		t.Fatalf("legacy permission leaked in API JSON: %s", data)
	}
	if !strings.Contains(string(data), `"upload":true`) || !strings.Contains(string(data), `"delete":false`) {
		t.Fatalf("split permission fields missing from API JSON: %s", data)
	}
}

func TestSharePermissionsPreferExplicitUploadOverLegacyWrite(t *testing.T) {
	var permissions SharePermissions
	if err := json.Unmarshal([]byte(`{"upload":false,"write":true,"delete":false}`), &permissions); err != nil {
		t.Fatal(err)
	}
	if permissions.Upload || permissions.Delete || permissions.LegacyReplace {
		t.Fatalf("explicit upload=false was overridden: %+v", permissions)
	}
}

func TestSharePermissionsResponseReportsLegacyWriteAccess(t *testing.T) {
	tests := []struct {
		name            string
		permissions     SharePermissions
		wantCanReplace  bool
		wantLegacyWrite bool
	}{
		{name: "view only", permissions: SharePermissions{View: true}, wantCanReplace: false, wantLegacyWrite: false},
		{name: "legacy write", permissions: SharePermissions{Upload: true, LegacyReplace: true}, wantCanReplace: true, wantLegacyWrite: true},
		{name: "upload only", permissions: SharePermissions{Upload: true}, wantCanReplace: false, wantLegacyWrite: true},
		{name: "upload and delete", permissions: SharePermissions{Upload: true, Delete: true}, wantCanReplace: true, wantLegacyWrite: true},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := test.permissions.ToResponse()
			if response.CanReplace != test.wantCanReplace {
				t.Fatalf("canReplace = %t, want %t", response.CanReplace, test.wantCanReplace)
			}
			if response.Write != test.wantLegacyWrite {
				t.Fatalf("legacy write alias = %t, want upload=%t", response.Write, test.wantLegacyWrite)
			}
			data, err := json.Marshal(response)
			if err != nil {
				t.Fatal(err)
			}
			var fields map[string]any
			if err := json.Unmarshal(data, &fields); err != nil {
				t.Fatal(err)
			}
			if got, ok := fields["write"].(bool); !ok || got != test.wantLegacyWrite {
				t.Fatalf("serialized write alias = %v, want upload=%t", fields["write"], test.wantLegacyWrite)
			}
		})
	}
}

func TestSharePermissionsIgnoreComputedReplacementInput(t *testing.T) {
	var permissions SharePermissions
	if err := json.Unmarshal([]byte(`{"upload":true,"canReplace":true}`), &permissions); err != nil {
		t.Fatal(err)
	}
	if permissions.LegacyReplace {
		t.Fatal("client-supplied canReplace granted legacy replacement access")
	}
}
