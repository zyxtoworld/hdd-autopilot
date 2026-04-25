package mining

import (
	"log"
	"runtime"
	"sync"
	"sync/atomic"
	"time"
)

type BenchmarkResult struct {
	Workers      int
	GOMAXPROCS   int
	Attempts     int64
	Elapsed      time.Duration
	AttemptsPerS float64
}

func FindBestBenchmarkConfig() BenchmarkResult {
	const benchmarkCaseDuration = 5 * time.Second

	job := NewJob(JobConfig{
		ChallengeID:    1,
		DifficultyBits: 255,
		MemoryCostMB:   64,
		Parallelism:    1,
		RoundID:        1,
		Seed:           "benchmark-seed-fixed",
		SessionSalt:    "benchmark-session-salt-fixed",
		TimeCost:       1,
		VisitorID:      "benchmark-visitor-fixed",
	})

	cpuCount := runtime.NumCPU()
	gomaxCandidates := make([]int, 0, 3)
	for _, n := range []int{1, cpuCount / 2, cpuCount} {
		if n > 0 && !containsInt(gomaxCandidates, n) {
			gomaxCandidates = append(gomaxCandidates, n)
		}
	}

	workerCandidates := []int{1}
	for _, n := range []int{cpuCount / 4, cpuCount / 2, cpuCount, cpuCount * 2} {
		if n > 1 && !containsInt(workerCandidates, n) {
			workerCandidates = append(workerCandidates, n)
		}
	}

	totalCases := len(gomaxCandidates) * len(workerCandidates)
	log.Printf("自动调优开始：%d 组组合，每组 %s，预计约 %s", totalCases, benchmarkCaseDuration, time.Duration(totalCases)*benchmarkCaseDuration)

	var best BenchmarkResult
	originalGOMAXPROCS := runtime.GOMAXPROCS(0)
	defer runtime.GOMAXPROCS(originalGOMAXPROCS)

	caseIndex := 0
	for _, gomax := range gomaxCandidates {
		runtime.GOMAXPROCS(gomax)
		for _, workers := range workerCandidates {
			caseIndex++
			log.Printf("自动调优进度 %d/%d: workers=%d gomaxprocs=%d", caseIndex, totalCases, workers, gomax)
			result := RunBenchmarkCase(job, workers, gomax, benchmarkCaseDuration)
			log.Printf("自动调优结果 %d/%d: workers=%d gomaxprocs=%d aps=%.2f", caseIndex, totalCases, workers, gomax, result.AttemptsPerS)
			if result.AttemptsPerS > best.AttemptsPerS {
				best = result
			}
		}
	}

	log.Printf("自动调优完成：最佳组合 workers=%d gomaxprocs=%d aps=%.2f", best.Workers, best.GOMAXPROCS, best.AttemptsPerS)
	return best
}

func RunBenchmarkCase(job *Job, workers, gomaxprocs int, duration time.Duration) BenchmarkResult {
	var attempts atomic.Int64
	var wg sync.WaitGroup
	deadline := time.Now().Add(duration)
	startedAt := time.Now()

	for i := 0; i < workers; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			runBenchmarkWorker(job, idx, workers, deadline, &attempts)
		}(i)
	}
	wg.Wait()

	elapsed := time.Since(startedAt)
	if elapsed <= 0 {
		elapsed = duration
	}
	count := attempts.Load()
	return BenchmarkResult{
		Workers:      workers,
		GOMAXPROCS:   gomaxprocs,
		Attempts:     count,
		Elapsed:      elapsed,
		AttemptsPerS: float64(count) / elapsed.Seconds(),
	}
}

func containsInt(values []int, target int) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
