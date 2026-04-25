package mining

import "testing"

func TestDetectGPUAvailabilityRejectsNonWindowsX64(t *testing.T) {
	availability := detectGPUAvailability(ModeInviteThenBalance, gpuDetector{
		goos:   "darwin",
		goarch: "arm64",
	})
	if availability.Available {
		t.Fatal("expected GPU to be unavailable on non-Windows x64")
	}
	if availability.Reason == "" {
		t.Fatal("expected fallback reason")
	}
}

func TestDetectGPUAvailabilityRejectsUnsupportedMode(t *testing.T) {
	availability := detectGPUAvailability(ModeBalanceThenInvite, gpuDetector{
		goos:                "windows",
		goarch:              "amd64",
		resolveDataFilePath: func(name string) string { return name },
		fileExists:          func(string) bool { return true },
		hasNVIDIA:           func() bool { return true },
	})
	if availability.Available {
		t.Fatal("expected GPU to be unavailable for unsupported mode")
	}
	if availability.SidecarName != "" {
		t.Fatalf("expected no sidecar name, got %q", availability.SidecarName)
	}
}

func TestDetectGPUAvailabilityRejectsMissingSidecar(t *testing.T) {
	availability := detectGPUAvailability(ModeInviteOnly, gpuDetector{
		goos:                "windows",
		goarch:              "amd64",
		resolveDataFilePath: func(name string) string { return `E:\项目\hdd\` + name },
		fileExists:          func(string) bool { return false },
		hasNVIDIA:           func() bool { return true },
	})
	if availability.Available {
		t.Fatal("expected GPU to be unavailable without sidecar")
	}
	if availability.SidecarName != "invite-miner-gpu-win-x64.exe" {
		t.Fatalf("unexpected sidecar name %q", availability.SidecarName)
	}
}

func TestDetectGPUAvailabilityRejectsMissingNVIDIA(t *testing.T) {
	availability := detectGPUAvailability(ModeBalanceOnly, gpuDetector{
		goos:                "windows",
		goarch:              "amd64",
		resolveDataFilePath: func(name string) string { return name },
		fileExists:          func(string) bool { return true },
		hasNVIDIA:           func() bool { return false },
	})
	if availability.Available {
		t.Fatal("expected GPU to be unavailable without NVIDIA")
	}
	if availability.SidecarName != "balance-miner-gpu-win-x64.exe" {
		t.Fatalf("unexpected sidecar name %q", availability.SidecarName)
	}
}

func TestDetectGPUAvailabilityAcceptsSupportedMode(t *testing.T) {
	availability := detectGPUAvailability(ModeInviteThenBalance, gpuDetector{
		goos:                "windows",
		goarch:              "amd64",
		resolveDataFilePath: func(name string) string { return `E:\项目\hdd\` + name },
		fileExists:          func(string) bool { return true },
		hasNVIDIA:           func() bool { return true },
	})
	if !availability.Available {
		t.Fatalf("expected GPU to be available, got reason %q", availability.Reason)
	}
	if availability.SidecarName != "hdd-miner-gpu-win-x64.exe" {
		t.Fatalf("unexpected sidecar name %q", availability.SidecarName)
	}
}
