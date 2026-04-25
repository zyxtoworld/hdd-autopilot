//go:build windows

package terminal

import (
	"context"
	"fmt"
	"os"
	"sync"
	"syscall"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

const (
	keyEventType = 0x0001
	vkEscape     = 0x001B
)

type keyEventRecord struct {
	KeyDown        int32
	RepeatCount    uint16
	VirtualKeyCode uint16
	VirtualScanCode uint16
	UnicodeChar    uint16
	ControlKeyState uint32
}

type inputRecord struct {
	EventType uint16
	_         uint16
	KeyEvent  keyEventRecord
}

var (
	kernel32               = windows.NewLazySystemDLL("kernel32.dll")
	procPeekConsoleInputW  = kernel32.NewProc("PeekConsoleInputW")
	procReadConsoleInputW  = kernel32.NewProc("ReadConsoleInputW")
)

func StartEscapeWatcher(prompt string) (context.Context, func(), error) {
	ctx, cancel := context.WithCancel(context.Background())
	handle := windows.Handle(os.Stdin.Fd())
	var originalMode uint32
	if err := windows.GetConsoleMode(handle, &originalMode); err != nil {
		return ctx, cancel, nil
	}
	if prompt != "" {
		fmt.Println(prompt)
	}
	mode := originalMode &^ (windows.ENABLE_ECHO_INPUT | windows.ENABLE_LINE_INPUT)
	if err := windows.SetConsoleMode(handle, mode); err != nil {
		cancel()
		return nil, nil, err
	}
	stop := make(chan struct{})
	done := make(chan struct{})
	go func() {
		defer close(done)
		watchEscape(handle, ctx, cancel, stop)
	}()
	var once sync.Once
	cleanup := func() {
		once.Do(func() {
			close(stop)
			cancel()
			<-done
			_ = windows.SetConsoleMode(handle, originalMode)
		})
	}
	return ctx, cleanup, nil
}

func watchEscape(handle windows.Handle, ctx context.Context, cancel context.CancelFunc, stop <-chan struct{}) {
	for {
		select {
		case <-ctx.Done():
			return
		case <-stop:
			return
		default:
		}
		var record inputRecord
		var count uint32
		if err := peekConsoleInput(handle, &record, 1, &count); err != nil {
			return
		}
		if count == 0 {
			time.Sleep(50 * time.Millisecond)
			continue
		}
		if err := readConsoleInput(handle, &record, 1, &count); err != nil {
			return
		}
		if count == 0 || record.EventType != keyEventType || record.KeyEvent.KeyDown == 0 {
			continue
		}
		if record.KeyEvent.VirtualKeyCode == vkEscape {
			cancel()
			return
		}
	}
}

func peekConsoleInput(handle windows.Handle, record *inputRecord, length uint32, read *uint32) error {
	r1, _, e1 := procPeekConsoleInputW.Call(uintptr(handle), uintptr(unsafe.Pointer(record)), uintptr(length), uintptr(unsafe.Pointer(read)))
	if r1 != 0 {
		return nil
	}
	if e1 != syscall.Errno(0) {
		return error(e1)
	}
	return syscall.EINVAL
}

func readConsoleInput(handle windows.Handle, record *inputRecord, length uint32, read *uint32) error {
	r1, _, e1 := procReadConsoleInputW.Call(uintptr(handle), uintptr(unsafe.Pointer(record)), uintptr(length), uintptr(unsafe.Pointer(read)))
	if r1 != 0 {
		return nil
	}
	if e1 != syscall.Errno(0) {
		return error(e1)
	}
	return syscall.EINVAL
}
