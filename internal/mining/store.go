package mining

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

type InviteStore struct {
	mu   sync.Mutex
	path string
}

func NewInviteStore(path string) *InviteStore {
	return &InviteStore{path: path}
}

func (s *InviteStore) Path() string {
	return s.path
}

func (s *InviteStore) Save(code string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := ensureStoreDir(s.path); err != nil {
		return err
	}
	f, err := os.OpenFile(s.path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return err
	}
	defer f.Close()
	timestamp := time.Now().Format("2006-01-02 15:04:05")
	_, err = fmt.Fprintf(f, "[%s] 已保存邀请码：%s\n", timestamp, code)
	return err
}

type BalanceCodeStore struct {
	mu   sync.Mutex
	path string
}

func NewBalanceCodeStore(path string) *BalanceCodeStore {
	return &BalanceCodeStore{path: path}
}

func (s *BalanceCodeStore) Path() string {
	return s.path
}

func (s *BalanceCodeStore) Save(code string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := ensureStoreDir(s.path); err != nil {
		return err
	}
	f, err := os.OpenFile(s.path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return err
	}
	defer f.Close()
	timestamp := time.Now().Format("2006-01-02 15:04:05")
	_, err = fmt.Fprintf(f, "[%s] 已保存余额兑换码：%s\n", timestamp, code)
	return err
}

func ensureStoreDir(path string) error {
	dir := filepath.Dir(path)
	if dir == "." || dir == "" {
		return nil
	}
	return os.MkdirAll(dir, 0700)
}
