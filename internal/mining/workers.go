package mining

import (
	"encoding/hex"
	"sync/atomic"
	"time"
)

func MineWorker(job *Job, workerIndex, totalWorkers int, stop *atomic.Bool, attempts *atomic.Int64) Result {
	nonceStart := workerIndex + 1
	nonceStep := totalWorkers
	localAttempts := int64(0)
	passBuf := make([]byte, 0, job.PassPrefixLength()+20)

	for nonce := nonceStart; !stop.Load(); nonce += nonceStep {
		digestBytes := ComputeDigest(job, nonce, passBuf)

		localAttempts++
		if localAttempts%5 == 0 {
			attempts.Add(5)
		}

		if MeetsDifficulty(digestBytes, job.DifficultyBits()) {
			stop.Store(true)
			attempts.Add(localAttempts % 5)
			return Result{Nonce: nonce, Digest: hex.EncodeToString(digestBytes)}
		}
	}

	attempts.Add(localAttempts % 5)
	return Result{}
}

func runBenchmarkWorker(job *Job, workerIndex, totalWorkers int, deadline time.Time, attempts *atomic.Int64) {
	nonceStart := workerIndex + 1
	nonceStep := totalWorkers
	localAttempts := int64(0)
	passBuf := make([]byte, 0, job.PassPrefixLength()+20)

	for nonce := nonceStart; time.Now().Before(deadline); nonce += nonceStep {
		_ = ComputeDigest(job, nonce, passBuf)
		localAttempts++
		if localAttempts%64 == 0 {
			attempts.Add(64)
		}
	}

	attempts.Add(localAttempts % 64)
}
