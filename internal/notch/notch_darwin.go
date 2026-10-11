//go:build darwin

package notch

/*
#cgo CFLAGS: -x objective-c -fobjc-arc -mmacosx-version-min=13.0
#cgo LDFLAGS: -framework Cocoa -framework WebKit -framework UniformTypeIdentifiers -mmacosx-version-min=13.0
#include <stdlib.h>
char* ForceConfigure(void);
char* ForceGeometry(void);
void ForceExpand(int expanded);
void ForceShutdown(void);
*/
import "C"

import (
	"encoding/json"
	"sync"
	"unsafe"
)

type Geometry struct {
	Gap            float64 `json:"gap"`
	Top            float64 `json:"top"`
	CollapsedWidth float64 `json:"collapsedWidth"`
	ExpandedWidth  float64 `json:"expandedWidth"`
	ExpandedHeight float64 `json:"expandedHeight"`
	HasNotch       bool    `json:"hasNotch"`
}

var handler func(int)
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

func Configure(callback func(int)) Geometry {
	handlerMu.Lock()
	handler = callback
	handlerMu.Unlock()
	return decode(C.ForceConfigure())
}

func CurrentGeometry() Geometry { return decode(C.ForceGeometry()) }

func decode(raw *C.char) Geometry {
	defer C.free(unsafe.Pointer(raw))
	geometry := Geometry{Top: 28, Gap: 0, CollapsedWidth: 224, ExpandedWidth: 432, ExpandedHeight: 480}
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
