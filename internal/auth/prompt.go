package auth

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strings"

	"hdd/internal/model"

	"golang.org/x/term"
)

func PromptEmail() (string, error) {
	reader := bufio.NewReader(os.Stdin)
	for {
		fmt.Print("请输入邮箱: ")
		line, err := reader.ReadString('\n')
		if err != nil && err != io.EOF {
			return "", err
		}
		email := strings.TrimSpace(line)
		if email != "" {
			return email, nil
		}
		if err == io.EOF {
			return "", fmt.Errorf("邮箱不能为空")
		}
		fmt.Println("邮箱不能为空，请重新输入。")
	}
}

func PromptAccountSetupChoice(accounts []model.AuthCache, selectedEmail string) (string, error) {
	accounts = normalizeAccounts(accounts)
	reader := bufio.NewReader(os.Stdin)
	for {
		fmt.Println("当前已保存的账号：")
		if len(accounts) == 0 {
			fmt.Println("（还没有）")
		} else {
			for i, account := range accounts {
				marker := ""
				if selectedEmail != "" && strings.EqualFold(account.Email, selectedEmail) {
					marker = " [上次使用]"
				}
				fmt.Printf("%d. %s%s\n", i+1, account.Email, marker)
			}
		}
		fmt.Println("可选操作：")
		fmt.Println("1. 添加新账号")
		fmt.Println("2. 账号已经准备好，开始运行")
		fmt.Print("请输入选项 (1/2): ")

		line, err := reader.ReadString('\n')
		if err != nil && err != io.EOF {
			return "", err
		}

		choice := strings.TrimSpace(line)
		switch choice {
		case "1", "2":
			return choice, nil
		case "":
			if err == io.EOF {
				return "", fmt.Errorf("未输入选项")
			}
			fmt.Println("你还没输入选项，请输入 1（添加新账号）或 2（开始运行）。")
		default:
			fmt.Printf("看不懂你输入的选项 %q，请输入 1（添加新账号）或 2（开始运行）。\n", choice)
		}
	}
}

func PromptPassword() (string, error) {
	fd := int(os.Stdin.Fd())
	oldState, err := term.MakeRaw(fd)
	if err != nil {
		return "", err
	}
	defer term.Restore(fd, oldState)

PromptLoop:
	for {
		fmt.Print("请输入密码: ")
		reader := bufio.NewReader(os.Stdin)
		password := make([]byte, 0, 32)

		for {
			b, err := reader.ReadByte()
			if err != nil {
				fmt.Println()
				return "", err
			}

			switch b {
			case '\r', '\n':
				fmt.Println()
				if len(password) == 0 {
					fmt.Println("密码不能为空，请重新输入。")
					continue PromptLoop
				}
				return string(password), nil
			case 3:
				fmt.Println()
				return "", fmt.Errorf("输入已取消")
			case 8, 127:
				if len(password) > 0 {
					password = password[:len(password)-1]
					fmt.Print("\b \b")
				}
			case 0, 224:
				if _, err := reader.ReadByte(); err != nil {
					fmt.Println()
					return "", err
				}
			default:
				if b >= 32 {
					password = append(password, b)
					fmt.Print("*")
				}
			}
		}
	}
}
