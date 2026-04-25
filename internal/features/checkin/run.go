package checkin

import (
	"context"
	"time"

	"hdd/internal/model"
	"hdd/internal/terminal"
)

func Run(apiClient interface {
	GetCheckinMe(authToken string) (*model.CheckinMeResponse, error)
	GetCheckinToday(authToken string) (*model.CheckinTodayResponse, error)
	ClaimCheckinToday(authToken string) (*model.CheckinClaimResponse, error)
}, authToken string, email string) model.CheckinResult {
	return RunWithContext(context.Background(), apiClient, authToken, email)
}

func RunWithContext(ctx context.Context, apiClient interface {
	GetCheckinMe(authToken string) (*model.CheckinMeResponse, error)
	GetCheckinToday(authToken string) (*model.CheckinTodayResponse, error)
	ClaimCheckinToday(authToken string) (*model.CheckinClaimResponse, error)
}, authToken string, email string) model.CheckinResult {
	result := model.CheckinResult{
		Email: email,
		When:  time.Now(),
	}

	if err := terminal.Check(ctx); err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	before, err := apiClient.GetCheckinMe(authToken)
	if err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	if err := terminal.Check(ctx); err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	today, err := apiClient.GetCheckinToday(authToken)
	if err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	if today.Claimed {
		result.Status = "签到失败（今日已签到）"
		result.Success = false
		result.Delta = 0
		result.BalanceAfter = before.User.Balance
		return result
	}

	if err := terminal.Check(ctx); err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	claimResp, err := apiClient.ClaimCheckinToday(authToken)
	if err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	if err := terminal.Check(ctx); err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	after, err := apiClient.GetCheckinMe(authToken)
	if err != nil {
		result.Status = "签到失败"
		result.Err = err
		return result
	}

	result.Success = claimResp.Ok && !claimResp.AlreadyClaimed
	if claimResp.AlreadyClaimed {
		result.Status = "签到失败（今日已签到）"
	} else if result.Success {
		result.Status = "签到成功"
	} else {
		result.Status = "签到失败（签到接口未返回成功标记）"
	}
	result.Delta = after.User.Balance - before.User.Balance
	result.BalanceAfter = after.User.Balance
	if result.Delta == 0 && claimResp.RewardAmount > 0 {
		result.Delta = claimResp.RewardAmount
	}
	return result
}
