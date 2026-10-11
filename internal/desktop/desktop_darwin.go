//go:build darwin

package desktop

/*
#cgo CFLAGS: -x objective-c -fobjc-arc -mmacosx-version-min=13.0
#cgo LDFLAGS: -framework Cocoa -framework WebKit -framework UniformTypeIdentifiers -mmacosx-version-min=13.0
#include <stdlib.h>
char* ForceConfigure(void);
char* ForceGeometry(void);
char* ForceInspectWindow(void);
void ForceExpand(int expanded);
void ForceShutdown(void);
void ForceHide(void);
void ForceUpdateMenu(const char *time);
void ForceShowDetails(const char *view);
void ForceDetailsResolve(const char *payload);
*/
import "C"

import (
	"encoding/json"
	"sync"
	"unsafe"
)

type Geometry struct {
	CompactWidth  float64 `json:"compactWidth"`
	CompactHeight float64 `json:"compactHeight"`
	PanelWidth    float64 `json:"panelWidth"`
	PanelHeight   float64 `json:"panelHeight"`
}

var handler func(int)
var detailHandler func(string) string
var handlerMu sync.RWMutex

//export ForceNativeEvent
func ForceNativeEvent(event C.int) {
	handlerMu.RLock()
	f := handler
	handlerMu.RUnlock()
	if f != nil {
		go f(int(event))
	}
}

func Configure(callback func(int), readDetails func(string) string) Geometry {
	handlerMu.Lock()
	handler = callback
	detailHandler = readDetails
	handlerMu.Unlock()
	return decode(C.ForceConfigure())
}

func CurrentGeometry() Geometry { return decode(C.ForceGeometry()) }

func decode(raw *C.char) Geometry {
	defer C.free(unsafe.Pointer(raw))
	geometry := Geometry{CompactWidth: 280, CompactHeight: 54, PanelWidth: 368, PanelHeight: 456}
	_ = json.Unmarshal([]byte(C.GoString(raw)), &geometry)
	return geometry
}

func Expand(expanded bool) {
	value := 0
	if expanded {
		value = 1
	}
	C.ForceExpand(C.int(value))
}

func Shutdown() { C.ForceShutdown() }

//export ForceDetailRequest
func ForceDetailRequest(raw *C.char) {
	payload := C.GoString(raw)
	handlerMu.RLock()
	read := detailHandler
	handlerMu.RUnlock()
	if read == nil {
		return
	}
	go func() {
		result := C.CString(read(payload))
		defer C.free(unsafe.Pointer(result))
		C.ForceDetailsResolve(result)
	}()
}
func HideToMenu() { C.ForceHide() }
func UpdateMenu(time string) {
	raw := C.CString(time)
	defer C.free(unsafe.Pointer(raw))
	C.ForceUpdateMenu(raw)
}
func ShowDetails(view string) {
	raw := C.CString(view)
	defer C.free(unsafe.Pointer(raw))
	C.ForceShowDetails(raw)
}

func InspectWindow() string {
	raw := C.ForceInspectWindow()
	defer C.free(unsafe.Pointer(raw))
	return C.GoString(raw)
}
