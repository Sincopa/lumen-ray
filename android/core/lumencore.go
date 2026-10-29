// Package lumencore embeds Xray-core in the Android VPN service.
//
// gomobile exposes only strings, ints, bools and interfaces, so every call returns
// an error text: an empty string means success. The service owns the TUN file
// descriptor; Xray reads it until Stop and never closes it.
package lumencore

import (
	"bytes"
	"encoding/json"
	"os"
	"runtime/debug"
	"strconv"
	"sync"
	"syscall"

	"github.com/xtls/xray-core/common/platform"
	"github.com/xtls/xray-core/core"
	"github.com/xtls/xray-core/features/stats"
	"github.com/xtls/xray-core/infra/conf/serial"
	_ "github.com/xtls/xray-core/main/distro/all"
	"github.com/xtls/xray-core/transport/internet"
)

// Protector keeps an outbound socket off the tunnel (VpnService.protect).
type Protector interface {
	Protect(fd int) bool
}

var (
	mu       sync.Mutex
	instance *core.Instance
	once     sync.Once

	// protectMu is separate from mu: Start holds mu while Xray opens its
	// inbound listeners, and every new socket calls the protector.
	protectMu sync.RWMutex
	protector Protector

	probeMu   sync.Mutex
	probes    = map[int]*core.Instance{}
	nextProbe int
)

// Version reports the embedded Xray-core version.
func Version() string {
	return core.Version()
}

// SetProtector registers the socket protector once; later calls replace the target.
func SetProtector(p Protector) {
	protectMu.Lock()
	protector = p
	protectMu.Unlock()
	once.Do(func() {
		control := func(network, address string, c syscall.RawConn) error {
			protectMu.RLock()
			current := protector
			protectMu.RUnlock()
			if current == nil {
				return nil
			}
			return c.Control(func(fd uintptr) {
				current.Protect(int(fd))
			})
		}
		_ = internet.RegisterDialerController(control)
		_ = internet.RegisterListenerController(control)
	})
}

// SetMemoryLimit applies the same Go runtime tuning the desktop core process uses.
func SetMemoryLimit(megabytes int) {
	debug.SetGCPercent(50)
	if megabytes > 0 {
		debug.SetMemoryLimit(int64(megabytes) << 20)
	}
}

func load(config string) (*core.Config, error) {
	return serial.LoadJSONConfig(bytes.NewReader([]byte(config)))
}

// Validate builds the configuration without starting it, like `xray run -test`.
// assetDir holds geoip.dat and geosite.dat.
func Validate(config string, assetDir string) string {
	mu.Lock()
	defer mu.Unlock()
	_ = os.Setenv(platform.AssetLocation, assetDir)
	built, err := load(config)
	if err != nil {
		return err.Error()
	}
	server, err := core.New(built)
	if err != nil {
		return err.Error()
	}
	_ = server.Close()
	return ""
}

// Start replaces any running instance. tunFd <= 0 starts without a TUN inbound fd.
func Start(config string, tunFd int, assetDir string) string {
	mu.Lock()
	defer mu.Unlock()
	_ = os.Setenv(platform.AssetLocation, assetDir)
	if instance != nil {
		_ = instance.Close()
		instance = nil
	}
	if tunFd > 0 {
		_ = os.Setenv(platform.TunFdKey, strconv.Itoa(tunFd))
	} else {
		_ = os.Unsetenv(platform.TunFdKey)
	}
	built, err := load(config)
	if err != nil {
		return err.Error()
	}
	server, err := core.New(built)
	if err != nil {
		return err.Error()
	}
	if err := server.Start(); err != nil {
		_ = server.Close()
		return err.Error()
	}
	instance = server
	return ""
}

// Stop closes the running instance. It is safe to call when nothing runs.
func Stop() string {
	mu.Lock()
	defer mu.Unlock()
	if instance == nil {
		return ""
	}
	err := instance.Close()
	instance = nil
	if err != nil {
		return err.Error()
	}
	return ""
}

// StartProbe runs a short-lived second instance next to the tunnel, for
// latency checks through its SOCKS inbound. It returns the probe id, or 0 when
// the configuration did not start.
func StartProbe(config string, assetDir string) int {
	_ = os.Setenv(platform.AssetLocation, assetDir)
	built, err := load(config)
	if err != nil {
		return 0
	}
	server, err := core.New(built)
	if err != nil {
		return 0
	}
	if err := server.Start(); err != nil {
		_ = server.Close()
		return 0
	}
	probeMu.Lock()
	defer probeMu.Unlock()
	nextProbe++
	probes[nextProbe] = server
	return nextProbe
}

// StopProbe closes a probe instance. Unknown ids are ignored.
func StopProbe(id int) {
	probeMu.Lock()
	server := probes[id]
	delete(probes, id)
	probeMu.Unlock()
	if server != nil {
		_ = server.Close()
	}
}

// IsRunning reports whether an instance is started.
func IsRunning() bool {
	mu.Lock()
	defer mu.Unlock()
	return instance != nil && instance.IsRunning()
}

// QueryStats returns {"up":n,"down":n} summed over proxy outbounds (not direct,
// block or dns). reset clears the counters after reading.
func QueryStats(reset bool) string {
	mu.Lock()
	server := instance
	mu.Unlock()
	result := map[string]int64{"up": 0, "down": 0}
	if server != nil {
		if manager, ok := server.GetFeature(stats.ManagerType()).(stats.Manager); ok {
			collect := func(name string, counter stats.Counter) {
				var value int64
				if reset {
					value = counter.Set(0)
				} else {
					value = counter.Value()
				}
				switch {
				case hasSuffix(name, ">>>traffic>>>uplink"):
					result["up"] += value
				case hasSuffix(name, ">>>traffic>>>downlink"):
					result["down"] += value
				}
			}
			if visitor, ok := manager.(interface {
				VisitCounters(func(string, stats.Counter) bool)
			}); ok {
				visitor.VisitCounters(func(name string, counter stats.Counter) bool {
					if isProxyOutbound(name) {
						collect(name, counter)
					}
					return true
				})
			}
		}
	}
	out, _ := json.Marshal(result)
	return string(out)
}

func hasSuffix(s, suffix string) bool {
	return len(s) >= len(suffix) && s[len(s)-len(suffix):] == suffix
}

func isProxyOutbound(name string) bool {
	const prefix = "outbound>>>"
	if len(name) <= len(prefix) || name[:len(prefix)] != prefix {
		return false
	}
	rest := name[len(prefix):]
	for _, builtin := range []string{"direct>>>", "block>>>", "dns>>>"} {
		if len(rest) >= len(builtin) && rest[:len(builtin)] == builtin {
			return false
		}
	}
	return true
}
