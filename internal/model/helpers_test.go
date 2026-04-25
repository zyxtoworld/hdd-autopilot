package model

import (
	"testing"
	"time"
)

func TestScratchTicketReward(t *testing.T) {
	reward := 3.5
	if got := ScratchTicketReward(ScratchTicketPayload{RewardAmount: &reward}); got != reward {
		t.Fatalf("expected %v, got %v", reward, got)
	}
	if got := ScratchTicketReward(ScratchTicketPayload{}); got != 0 {
		t.Fatalf("expected zero reward, got %v", got)
	}
}

func TestScratchRevealReadyAtPrefersEarliestRevealAt(t *testing.T) {
	target := time.UnixMilli(1714000000000)
	got := ScratchRevealReadyAt(&ScratchPlayResponse{
		EarliestRevealAtMs: target.UnixMilli(),
		IssuedAtMs:         target.Add(-time.Second).UnixMilli(),
		MinScratchMs:       2000,
	})
	if !got.Equal(target) {
		t.Fatalf("expected %v, got %v", target, got)
	}
}

func TestScratchRevealReadyAtFallsBackToIssuedAtPlusScratchTime(t *testing.T) {
	issuedAt := time.UnixMilli(1714000000000)
	got := ScratchRevealReadyAt(&ScratchPlayResponse{
		IssuedAtMs:   issuedAt.UnixMilli(),
		MinScratchMs: 2500,
	})
	want := issuedAt.Add(2500 * time.Millisecond)
	if !got.Equal(want) {
		t.Fatalf("expected %v, got %v", want, got)
	}
}

func TestScratchCountMatchedNumbers(t *testing.T) {
	got := ScratchCountMatchedNumbers([]ScratchNumber{{Matched: true}, {Matched: false}, {Matched: true}})
	if got != 2 {
		t.Fatalf("expected 2, got %d", got)
	}
}
