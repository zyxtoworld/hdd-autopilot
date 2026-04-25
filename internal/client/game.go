package client

import (
	"fmt"
	"net/http"

	"hdd/internal/model"
)

const (
	configPath  = "/tile-api/config"
	historyPath = "/tile-api/history"
	mePath      = "/tile-api/me"
	startPath   = "/tile-api/start"
	stepPath    = "/tile-api/step"
	abandonPath = "/tile-api/abandon"
)

func (c *APIClient) GetConfig(authToken string) (*model.ConfigResponse, error) {
	req, err := c.newJSONRequest(http.MethodGet, configPath, authToken, c.baseURL+"/tile", nil)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.ConfigResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("获取游戏配置失败：%s", localizedStatusMessage(statusCode, body))
	})
}

func (c *APIClient) GetHistory(authToken string) (*model.HistoryResponse, error) {
	req, err := c.newJSONRequest(http.MethodGet, historyPath, authToken, c.baseURL+"/tile", nil)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.HistoryResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("获取对局记录失败：%s", localizedStatusMessage(statusCode, body))
	})
}

func (c *APIClient) GetTileMe(authToken string) (*model.TileMeResponse, error) {
	req, err := c.newJSONRequest(http.MethodGet, mePath, authToken, c.baseURL+"/tile", nil)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.TileMeResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("获取游戏信息失败：%s", localizedStatusMessage(statusCode, body))
	})
}

func (c *APIClient) StartGame(authToken string, difficulty string) (*model.StartResponse, error) {
	req, err := c.newJSONRequest(http.MethodPost, startPath, authToken, c.baseURL+"/tile", model.StartRequest{Difficulty: difficulty})
	if err != nil {
		return nil, err
	}
	resp, err := doJSONRequest[model.StartResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("开始 %s 难度新对局失败：%s", localizedDifficultyLabel(difficulty), localizedStatusMessage(statusCode, body))
	})
	if err != nil {
		return nil, err
	}
	if resp.Difficulty == "" {
		resp.Difficulty = difficulty
	}
	return resp, nil
}

func (c *APIClient) Step(authToken string, request model.StepRequest) (*model.StepResponse, error) {
	req, err := c.newJSONRequest(http.MethodPost, stepPath, authToken, c.baseURL+"/tile", request)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.StepResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("对局 %d 执行动作 %s 失败：%s", request.SessionID, localizedActionLabel(request.Action), localizedStatusMessage(statusCode, body))
	})
}

func (c *APIClient) Abandon(authToken string, sessionID int) (*model.AbandonResponse, error) {
	req, err := c.newJSONRequest(http.MethodPost, abandonPath, authToken, c.baseURL+"/tile", model.AbandonRequest{SessionID: sessionID})
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.AbandonResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("放弃对局 %d 失败：%s", sessionID, localizedStatusMessage(statusCode, body))
	})
}
