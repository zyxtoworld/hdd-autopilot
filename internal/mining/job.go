package mining

import "strconv"

type JobConfig struct {
	Seed           string
	RoundID        int
	VisitorID      string
	ChallengeID    int
	SessionSalt    string
	TimeCost       int
	MemoryCostMB   int
	Parallelism    int
	DifficultyBits int
}

type Job struct {
	seedBytes      []byte
	passPrefix     []byte
	timeCost       uint32
	memoryCostKB   uint32
	parallelism    uint8
	difficultyBits int
}

type Result struct {
	Nonce  int
	Digest string
}

func NewJob(cfg JobConfig) *Job {
	job := &Job{
		seedBytes:      []byte(cfg.Seed),
		passPrefix:     []byte(cfg.Seed + ":" + strconv.Itoa(cfg.RoundID) + ":" + cfg.VisitorID + ":" + strconv.Itoa(cfg.ChallengeID) + ":" + cfg.SessionSalt + ":"),
		timeCost:       uint32(cfg.TimeCost),
		memoryCostKB:   uint32(cfg.MemoryCostMB) * 1024,
		parallelism:    uint8(cfg.Parallelism),
		difficultyBits: cfg.DifficultyBits,
	}
	if job.parallelism < 1 {
		job.parallelism = 1
	}
	return job
}

func (j *Job) DifficultyBits() int {
	return j.difficultyBits
}

func (j *Job) PassPrefixLength() int {
	return len(j.passPrefix)
}
