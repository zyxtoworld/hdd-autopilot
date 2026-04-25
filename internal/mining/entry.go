package mining

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"time"

	rootconfig "hdd/internal/config"
	"hdd/internal/terminal"
)

func RunAutoTuned(mode Mode) error {
	return RunAutoTunedWithContext(context.Background(), mode)
}

func RunAutoTunedWithContext(ctx context.Context, mode Mode) error {
	cfg := defaultConfig(defaultThreadCount(), mode)
	poolClient := NewClient(cfg)
	inviteStore := NewInviteStore(cfg.InviteOutputFile)
	balanceCodeStore := NewBalanceCodeStore(cfg.BalanceOutputFile)
	r := NewRunner(ctx, cfg, poolClient, inviteStore, balanceCodeStore)
	return r.RunAutoTuned()
}

type GPUAvailability struct {
	Available   bool
	Reason      string
	SidecarName string
	SidecarPath string
}

func DetectGPUAvailability(mode Mode) GPUAvailability {
	return detectGPUAvailability(mode, newGPUDetector())
}

func RunAutoTunedGPU(mode Mode) error {
	return RunAutoTunedGPUWithContext(context.Background(), mode)
}

func RunAutoTunedGPUWithContext(ctx context.Context, mode Mode) error {
	availability := DetectGPUAvailability(mode)
	if !availability.Available {
		if availability.Reason == "" {
			return errors.New("当前环境不支持 GPU 挖矿")
		}
		return errors.New(availability.Reason)
	}
	cmd := exec.CommandContext(ctx, availability.SidecarPath, "auto")
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		if errors.Is(ctx.Err(), context.Canceled) {
			return terminal.ErrInterrupted
		}
		return fmt.Errorf("GPU 挖矿运行失败: %w", err)
	}
	return nil
}

type gpuDetector struct {
	goos                string
	goarch              string
	resolveDataFilePath func(string) string
	fileExists          func(string) bool
	hasNVIDIA           func() bool
}

func newGPUDetector() gpuDetector {
	return gpuDetector{
		goos:                runtime.GOOS,
		goarch:              runtime.GOARCH,
		resolveDataFilePath: rootconfig.ResolveDataFilePath,
		fileExists: func(path string) bool {
			info, err := os.Stat(path)
			if err != nil {
				return false
			}
			return !info.IsDir()
		},
		hasNVIDIA: hasNVIDIAEnvironment,
	}
}

func detectGPUAvailability(mode Mode, detector gpuDetector) GPUAvailability {
	if detector.goos != "windows" || detector.goarch != "amd64" {
		return GPUAvailability{Reason: "当前环境不是 Windows x64，自动使用 CPU 挖矿。"}
	}
	sidecarName, ok := gpuSidecarName(mode)
	if !ok {
		return GPUAvailability{Reason: fmt.Sprintf("%s目前暂不提供 GPU 挖矿，自动使用 CPU 挖矿。", modeLabel(mode))}
	}
	sidecarPath := detector.resolveDataFilePath(filepath.Join("dist", sidecarName))
	availability := GPUAvailability{SidecarName: sidecarName, SidecarPath: sidecarPath}
	if !detector.fileExists(sidecarPath) {
		availability.Reason = fmt.Sprintf("未找到 GPU sidecar：%s，自动使用 CPU 挖矿。", sidecarName)
		return availability
	}
	if !detector.hasNVIDIA() {
		availability.Reason = "未检测到可用的 NVIDIA 环境，自动使用 CPU 挖矿。"
		return availability
	}
	availability.Available = true
	return availability
}

func gpuSidecarName(mode Mode) (string, bool) {
	switch normalizeMode(mode) {
	case ModeInviteThenBalance:
		return "hdd-miner-gpu-win-x64.exe", true
	case ModeInviteOnly:
		return "invite-miner-gpu-win-x64.exe", true
	case ModeBalanceOnly:
		return "balance-miner-gpu-win-x64.exe", true
	default:
		return "", false
	}
}

func hasNVIDIAEnvironment() bool {
	smiPath, ok := findNVIDIASMI()
	if !ok {
		return false
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	if err := exec.CommandContext(ctx, smiPath, "-L").Run(); err != nil {
		return false
	}
	return true
}

func findNVIDIASMI() (string, bool) {
	for _, candidate := range []string{"nvidia-smi.exe", "nvidia-smi"} {
		if path, err := exec.LookPath(candidate); err == nil {
			return path, true
		}
	}
	var candidates []string
	if systemRoot := os.Getenv("SystemRoot"); systemRoot != "" {
		candidates = append(candidates, filepath.Join(systemRoot, "System32", "nvidia-smi.exe"))
	}
	if programFiles := os.Getenv("ProgramFiles"); programFiles != "" {
		candidates = append(candidates, filepath.Join(programFiles, "NVIDIA Corporation", "NVSMI", "nvidia-smi.exe"))
	}
	if programW6432 := os.Getenv("ProgramW6432"); programW6432 != "" {
		candidates = append(candidates, filepath.Join(programW6432, "NVIDIA Corporation", "NVSMI", "nvidia-smi.exe"))
	}
	for _, candidate := range candidates {
		info, err := os.Stat(candidate)
		if err == nil && !info.IsDir() {
			return candidate, true
		}
	}
	return "", false
}
