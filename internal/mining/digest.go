package mining

import (
	"strconv"

	"golang.org/x/crypto/argon2"
)

func ComputeDigest(job *Job, nonce int, passBuf []byte) []byte {
	passBuf = append(passBuf[:0], job.passPrefix...)
	passBuf = strconv.AppendInt(passBuf, int64(nonce), 10)
	return argon2.IDKey(passBuf, job.seedBytes, job.timeCost, job.memoryCostKB, job.parallelism, 32)
}

func MeetsDifficulty(digest []byte, difficultyBits int) bool {
	fullBytes := difficultyBits / 8
	for i := 0; i < fullBytes; i++ {
		if i >= len(digest) || digest[i] != 0 {
			return false
		}
	}

	remainingBits := difficultyBits % 8
	if remainingBits == 0 {
		return true
	}
	if fullBytes >= len(digest) {
		return false
	}

	mask := byte(0xFF << (8 - remainingBits))
	return digest[fullBytes]&mask == 0
}
