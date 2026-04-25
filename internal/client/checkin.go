package client

import (
	"fmt"
	"net/http"

	"hdd/internal/model"
)

const (
	checkinMePath    = "/checkin-api/me"
	checkinTodayPath = "/checkin-api/today"
	checkinClaimPath = "/checkin-api/claim"
)

func (c *APIClient) GetCheckinMe(authToken string) (*model.CheckinMeResponse, error) {
	req, err := c.newJSONRequest(http.MethodGet, checkinMePath, authToken, c.baseURL+"/checkin", nil)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.CheckinMeResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("获取签到账号信息失败：%s", localizedStatusMessage(statusCode, body))
	})
}

func (c *APIClient) GetCheckinToday(authToken string) (*model.CheckinTodayResponse, error) {
	req, err := c.newJSONRequest(http.MethodGet, checkinTodayPath, authToken, c.baseURL+"/checkin", nil)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.CheckinTodayResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("获取今日签到状态失败：%s", localizedStatusMessage(statusCode, body))
	})
}

func (c *APIClient) ClaimCheckinToday(authToken string) (*model.CheckinClaimResponse, error) {
	req, err := c.newJSONRequest(http.MethodPost, checkinClaimPath, authToken, c.baseURL+"/checkin", map[string]any{})
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.CheckinClaimResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("执行今日签到失败：%s", localizedStatusMessage(statusCode, body))
	})
}
