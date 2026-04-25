package client

import (
	"fmt"
	"net/http"
	"strings"

	"hdd/internal/model"
)

const (
	scratchPlayPath    = "/scratch-api/play"
	scratchRevealPath  = "/scratch-api/reveal"
	scratchHistoryPath = "/scratch-api/history"
)

func (c *APIClient) PlayScratch(authToken string, gameType string) (*model.ScratchPlayResponse, error) {
	req, err := c.newJSONRequest(http.MethodPost, scratchPlayPath, authToken, c.baseURL+"/scratch", model.ScratchPlayRequest{GameType: gameType})
	if err != nil {
		return nil, err
	}

	playResp, err := doJSONRequest[model.ScratchPlayResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("开始随机刮刮乐失败：%s", localizedStatusMessage(statusCode, body))
	})
	if err != nil {
		return nil, err
	}
	if strings.TrimSpace(playResp.GameType) == "" {
		playResp.GameType = gameType
	}
	return playResp, nil
}

func (c *APIClient) RevealScratch(authToken string, playID int, revealToken string) (*model.ScratchRevealResponse, error) {
	req, err := c.newJSONRequest(http.MethodPost, scratchRevealPath, authToken, c.baseURL+"/scratch", model.ScratchRevealRequest{PlayID: playID, RevealToken: revealToken})
	if err != nil {
		return nil, err
	}

	return doJSONRequest[model.ScratchRevealResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("对局 %d 开奖失败：%s", playID, localizedStatusMessage(statusCode, body))
	})
}

func (c *APIClient) GetScratchHistory(authToken string) (*model.ScratchHistoryResponse, error) {
	req, err := c.newJSONRequest(http.MethodGet, scratchHistoryPath, authToken, c.baseURL+"/scratch", nil)
	if err != nil {
		return nil, err
	}
	return doJSONRequest[model.ScratchHistoryResponse](c, req, func(statusCode int, body []byte) error {
		return fmt.Errorf("获取刮刮乐记录失败：%s", localizedStatusMessage(statusCode, body))
	})
}
