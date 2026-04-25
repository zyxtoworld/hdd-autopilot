package checkin

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"hdd/internal/model"
	"hdd/internal/terminal"
)

type fakeAPIClient struct {
	before *model.CheckinMeResponse
	after  *model.CheckinMeResponse
	today  *model.CheckinTodayResponse
	claim  *model.CheckinClaimResponse
	err    error
	calls  int
}

func (f *fakeAPIClient) GetCheckinMe(authToken string) (*model.CheckinMeResponse, error) {
	if f.err != nil {
		return nil, f.err
	}
	f.calls++
	if f.calls == 1 {
		return f.before, nil
	}
	return f.after, nil
}

func (f *fakeAPIClient) GetCheckinToday(authToken string) (*model.CheckinTodayResponse, error) {
	if f.err != nil {
		return nil, f.err
	}
	return f.today, nil
}

func (f *fakeAPIClient) ClaimCheckinToday(authToken string) (*model.CheckinClaimResponse, error) {
	if f.err != nil {
		return nil, f.err
	}
	return f.claim, nil
}

func TestRunReturnsAlreadyClaimedResult(t *testing.T) {
	api := &fakeAPIClient{
		before: &model.CheckinMeResponse{User: model.CheckinUser{Balance: 10}},
		today:  &model.CheckinTodayResponse{Claimed: true},
	}
	result := Run(api, "token", "demo@example.com")
	if result.Status != "签到失败（今日已签到）" {
		t.Fatalf("unexpected status: %s", result.Status)
	}
	if result.BalanceAfter != 10 {
		t.Fatalf("expected balance 10, got %v", result.BalanceAfter)
	}
}

func TestRunReturnsSuccessResult(t *testing.T) {
	api := &fakeAPIClient{
		before: &model.CheckinMeResponse{User: model.CheckinUser{Balance: 10}},
		after:  &model.CheckinMeResponse{User: model.CheckinUser{Balance: 12}},
		today:  &model.CheckinTodayResponse{Claimed: false},
		claim:  &model.CheckinClaimResponse{Ok: true, RewardAmount: 2},
	}
	result := Run(api, "token", "demo@example.com")
	if result.Status != "签到成功" {
		t.Fatalf("unexpected status: %s", result.Status)
	}
	if result.Delta != 2 || result.BalanceAfter != 12 {
		t.Fatalf("unexpected totals: %+v", result)
	}
}

func TestRunPropagatesErrors(t *testing.T) {
	api := &fakeAPIClient{err: errors.New("boom")}
	result := Run(api, "token", "demo@example.com")
	if result.Err == nil {
		t.Fatal("expected error")
	}
	if result.Status != "签到失败" {
		t.Fatalf("unexpected status: %s", result.Status)
	}
}

func TestRunWithContextReturnsInterruptedBeforeRequest(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	result := RunWithContext(ctx, &fakeAPIClient{}, "token", "demo@example.com")
	if !errors.Is(result.Err, terminal.ErrInterrupted) {
		t.Fatalf("expected ErrInterrupted, got %v", result.Err)
	}
	if result.Status != "签到失败" {
		t.Fatalf("unexpected status: %s", result.Status)
	}
}

func TestAppendCheckinLogWritesSingleSharedFile(t *testing.T) {
	tempDir := t.TempDir()
	result := model.CheckinResult{
		Email:        "demo@example.com",
		Status:       "签到成功",
		Delta:        1,
		BalanceAfter: 12.34,
	}
	if err := AppendCheckinLog(tempDir, result); err != nil {
		t.Fatalf("AppendCheckinLog failed: %v", err)
	}
	content, err := os.ReadFile(filepath.Join(tempDir, "checkin.log"))
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	text := string(content)
	if !strings.Contains(text, "demo@example.com") || !strings.Contains(text, "签到成功") {
		t.Fatalf("unexpected log content: %q", text)
	}
}
