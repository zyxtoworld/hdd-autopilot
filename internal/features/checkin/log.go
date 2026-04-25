package checkin

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"hdd/internal/model"
)

var (
	cstZone          = time.FixedZone("UTC+8", 8*60*60)
	checkinLogMu sync.Mutex
)

func AppendCheckinLog(logDir string, result model.CheckinResult) error {
	if err := os.MkdirAll(logDir, 0700); err != nil {
		return err
	}
	path := filepath.Join(logDir, "checkin.log")
	status := result.Status
	if status == "" {
		status = "签到失败（未知原因）"
	}
	line := fmt.Sprintf("[%s] 账号 %s：%s，本次余额增加 %.2f，签到后余额 %.8f。\n",
		result.When.In(cstZone).Format("2006-01-02 15:04:05"),
		result.Email,
		status,
		result.Delta,
		result.BalanceAfter,
	)
	checkinLogMu.Lock()
	defer checkinLogMu.Unlock()
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		return err
	}
	defer file.Close()
	if _, err := file.WriteString(line); err != nil {
		return err
	}
	return file.Sync()
}
