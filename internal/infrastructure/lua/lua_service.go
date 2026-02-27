package lua

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os/exec"
	"regexp"
	"strings"
	"time"

	lua "github.com/yuin/gopher-lua"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/whatsapp"
)

type LuaService struct {
	waClient    *whatsapp.WhatsAppClient
	triggerRepo repository.TriggerRepository
	stateRepo   repository.UserStateRepository
	storage     repository.StorageRepository
}

func NewLuaService(waClient *whatsapp.WhatsAppClient, triggerRepo repository.TriggerRepository, stateRepo repository.UserStateRepository, storage repository.StorageRepository) *LuaService {
	return &LuaService{
		waClient:    waClient,
		triggerRepo: triggerRepo,
		stateRepo:   stateRepo,
		storage:     storage,
	}
}

func (s *LuaService) ExecuteTriggers(ctx context.Context, senderJID string, messageText string) (bool, error) {
	triggers, err := s.triggerRepo.GetAll(ctx)
	if err != nil {
		return false, err
	}

	for _, t := range triggers {
		if !t.IsActive {
			continue
		}

		re, err := regexp.Compile(t.Pattern)
		if err != nil {
			fmt.Printf("LuaService: invalid regex pattern %s: %v\n", t.Pattern, err)
			continue
		}

		if re.MatchString(messageText) {
			matches := re.FindStringSubmatch(messageText)
			go s.runScript(t.Script, senderJID, messageText, matches)
			return true, nil
		}
	}

	return false, nil
}

func (s *LuaService) runScript(script string, sender string, content string, matches []string) {
	L := lua.NewState()
	defer L.Close()

	// Inject globals
	L.SetGlobal("sender", lua.LString(sender))
	L.SetGlobal("content", lua.LString(content))

	luaMatches := L.NewTable()
	for i, m := range matches {
		L.RawSet(luaMatches, lua.LNumber(i), lua.LString(m))
	}
	L.SetGlobal("matches", luaMatches)

	// Inject functions
	L.SetGlobal("send_text", L.NewFunction(s.luaSendText))
	L.SetGlobal("send_sticker", L.NewFunction(s.luaSendSticker))
	L.SetGlobal("send_media", L.NewFunction(s.luaSendMedia))
	L.SetGlobal("fetch", L.NewFunction(s.luaFetch))
	L.SetGlobal("get_state", L.NewFunction(s.luaGetState))
	L.SetGlobal("set_state", L.NewFunction(s.luaSetState))
	
	// JSON functions
	L.SetGlobal("json_decode", L.NewFunction(s.luaJSONDecode))
	L.SetGlobal("json_encode", L.NewFunction(s.luaJSONEncode))

	// Storage functions
	L.SetGlobal("storage_save", L.NewFunction(s.luaStorageSave))
	L.SetGlobal("storage_get", L.NewFunction(s.luaStorageGet))
	L.SetGlobal("storage_delete", L.NewFunction(s.luaStorageDelete))
	L.SetGlobal("storage_exists", L.NewFunction(s.luaStorageExists))
	L.SetGlobal("storage_path", L.NewFunction(s.luaStoragePath))

	// Command line functions
	L.SetGlobal("ffmpeg", L.NewFunction(s.luaFFmpeg))
	L.SetGlobal("ffprobe", L.NewFunction(s.luaFFprobe))

	if err := L.DoString(script); err != nil {
		fmt.Printf("Lua Error: %v\n", err)
	}
}

func (s *LuaService) luaSendText(L *lua.LState) int {
	target := L.CheckString(1)
	text := L.CheckString(2)

	err := s.waClient.SendMessage(context.Background(), target, text, true)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}
	return 0
}

func (s *LuaService) luaSendSticker(L *lua.LState) int {
	target := L.CheckString(1)
	url := L.CheckString(2)

	// We need to download the image first
	resp, err := http.Get(url)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}

	err = s.waClient.SendSticker(context.Background(), target, data, false, url, true)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}
	return 0
}

func (s *LuaService) luaGetState(L *lua.LState) int {
	jid := L.CheckString(1)
	state, _ := s.stateRepo.GetUserState(jid)
	L.Push(lua.LString(state))
	return 1
}

func (s *LuaService) luaSetState(L *lua.LState) int {
	jid := L.CheckString(1)
	state := L.CheckString(2)
	s.stateRepo.SetUserState(jid, state)
	return 0
}

func (s *LuaService) luaFetch(L *lua.LState) int {
	url := L.CheckString(1)
	options := L.OptTable(2, nil)

	method := "GET"
	var body io.Reader

	if options != nil {
		if m := options.RawGet(lua.LString("method")); m != lua.LNil {
			method = m.String()
		}
		if b := options.RawGet(lua.LString("body")); b != lua.LNil {
			body = strings.NewReader(b.String())
		}
	}

	req, err := http.NewRequest(method, url, body)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}

	if options != nil {
		if h := options.RawGet(lua.LString("headers")); h.Type() == lua.LTTable {
			h.(*lua.LTable).ForEach(func(k, v lua.LValue) {
				req.Header.Set(k.String(), v.String())
			})
		}
	}

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	defer resp.Body.Close()

	respBody, _ := io.ReadAll(resp.Body)

	resTable := L.NewTable()
	L.RawSet(resTable, lua.LString("status"), lua.LNumber(resp.StatusCode))
	L.RawSet(resTable, lua.LString("body"), lua.LString(string(respBody)))

	L.Push(resTable)
	return 1
}

// Storage handlers
func (s *LuaService) luaStorageSave(L *lua.LState) int {
	filename := L.CheckString(1)
	content := L.CheckString(2)

	_, err := s.storage.Save(context.Background(), filename, strings.NewReader(content))
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}
	return 0
}

func (s *LuaService) luaStorageGet(L *lua.LState) int {
	filename := L.CheckString(1)

	reader, err := s.storage.Get(context.Background(), filename)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	defer reader.Close()

	data, err := io.ReadAll(reader)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}

	L.Push(lua.LString(string(data)))
	return 1
}

func (s *LuaService) luaStorageDelete(L *lua.LState) int {
	filename := L.CheckString(1)
	err := s.storage.Delete(context.Background(), filename)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}
	return 0
}

func (s *LuaService) luaStorageExists(L *lua.LState) int {
	filename := L.CheckString(1)
	exists := s.storage.Exists(filename)
	L.Push(lua.LBool(exists))
	return 1
}

func (s *LuaService) luaStoragePath(L *lua.LState) int {
	filename := L.CheckString(1)
	path := s.storage.GetPath(filename)
	L.Push(lua.LString(path))
	return 1
}

// FFmpeg and command handlers
func (s *LuaService) luaFFmpeg(L *lua.LState) int {
	argsTable := L.CheckTable(1)
	var args []string
	argsTable.ForEach(func(k, v lua.LValue) {
		args = append(args, v.String())
	})

	cmd := exec.Command("ffmpeg", args...)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	output, err := cmd.Output()
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(fmt.Sprintf("%v: %s", err, stderr.String())))
		return 2
	}

	L.Push(lua.LString(string(output)))
	return 1
}

func (s *LuaService) luaFFprobe(L *lua.LState) int {
	argsTable := L.CheckTable(1)
	var args []string
	argsTable.ForEach(func(k, v lua.LValue) {
		args = append(args, v.String())
	})

	cmd := exec.Command("ffprobe", args...)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	output, err := cmd.Output()
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(fmt.Sprintf("%v: %s", err, stderr.String())))
		return 2
	}

	L.Push(lua.LString(string(output)))
	return 1
}

// TestTrigger simulates a trigger execution and returns logs/results
func (s *LuaService) TestTrigger(ctx context.Context, pattern, script, message string) (map[string]interface{}, error) {
	result := make(map[string]interface{})

	re, err := regexp.Compile(pattern)
	if err != nil {
		return nil, fmt.Errorf("invalid regex: %v", err)
	}

	matched := re.MatchString(message)
	result["matched"] = matched

	if !matched {
		return result, nil
	}

	matches := re.FindStringSubmatch(message)
	result["matches"] = matches

	// Setup Lua VM for testing
	L := lua.NewState()
	defer L.Close()

	var logs []string
	L.SetGlobal("print", L.NewFunction(func(L *lua.LState) int {
		top := L.GetTop()
		var line []string
		for i := 1; i <= top; i++ {
			line = append(line, L.Get(i).String())
		}
		logs = append(logs, strings.Join(line, "\t"))
		return 0
	}))

	// Inject standard globals
	L.SetGlobal("sender", lua.LString("628123456789@s.whatsapp.net"))
	L.SetGlobal("content", lua.LString(message))
	luaMatches := L.NewTable()
	for i, m := range matches {
		L.RawSet(luaMatches, lua.LNumber(i), lua.LString(m))
	}
	L.SetGlobal("matches", luaMatches)

	// Inject mock actions
	var actions []string
	L.SetGlobal("send_text", L.NewFunction(func(L *lua.LState) int {
		target := L.CheckString(1)
		msg := L.CheckString(2)
		actions = append(actions, fmt.Sprintf("Action: send_text(to: %s, msg: %s)", target, msg))
		return 0
	}))
	L.SetGlobal("send_sticker", L.NewFunction(func(L *lua.LState) int {
		target := L.CheckString(1)
		url := L.CheckString(2)
		actions = append(actions, fmt.Sprintf("Action: send_sticker(to: %s, url: %s)", target, url))
		return 0
	}))
	L.SetGlobal("send_media", L.NewFunction(func(L *lua.LState) int {
		target := L.CheckString(1)
		url := L.CheckString(2)
		mType := L.OptString(3, "image")
		actions = append(actions, fmt.Sprintf("Action: send_media(to: %s, type: %s, url: %s)", target, mType, url))
		return 0
	}))

	// Real but safe functions
	L.SetGlobal("fetch", L.NewFunction(s.luaFetch))
	L.SetGlobal("get_state", L.NewFunction(s.luaGetState))
	L.SetGlobal("set_state", L.NewFunction(s.luaSetState))
	L.SetGlobal("json_decode", L.NewFunction(s.luaJSONDecode))
	L.SetGlobal("json_encode", L.NewFunction(s.luaJSONEncode))
	L.SetGlobal("storage_path", L.NewFunction(s.luaStoragePath))
	L.SetGlobal("storage_exists", L.NewFunction(s.luaStorageExists))
	L.SetGlobal("ffmpeg", L.NewFunction(s.luaFFmpeg))
	L.SetGlobal("ffprobe", L.NewFunction(s.luaFFprobe))

	// Execute
	err = L.DoString(script)
	if err != nil {
		result["error"] = err.Error()
	}

	result["logs"] = logs
	result["actions"] = actions

	return result, nil
}

func (s *LuaService) luaSendMedia(L *lua.LState) int {
	target := L.CheckString(1)
	url := L.CheckString(2)
	mediaType := L.OptString(3, "image") // image, video, document
	caption := L.OptString(4, "")

	resp, err := http.Get(url)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(resp.Body)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}

	ctx := context.Background()
	var sendErr error
	switch mediaType {
	case "image":
		sendErr = s.waClient.SendImage(ctx, target, data, caption, "", true)
	case "video":
		sendErr = s.waClient.SendVideo(ctx, target, data, caption, "", true)
	case "document":
		sendErr = s.waClient.SendDocument(ctx, target, data, "document", "", true)
	default:
		sendErr = fmt.Errorf("unsupported media type: %s", mediaType)
	}

	if sendErr != nil {
		L.Push(lua.LString(sendErr.Error()))
		return 1
	}
	return 0
}

// JSON handlers
func (s *LuaService) luaJSONDecode(L *lua.LState) int {
	str := L.CheckString(1)
	var data interface{}
	if err := json.Unmarshal([]byte(str), &data); err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	L.Push(s.goValueToLua(L, data))
	return 1
}

func (s *LuaService) luaJSONEncode(L *lua.LState) int {
	val := L.CheckAny(1)
	goVal := s.luaValueToGo(val)
	data, err := json.Marshal(goVal)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	L.Push(lua.LString(string(data)))
	return 1
}

func (s *LuaService) goValueToLua(L *lua.LState, val interface{}) lua.LValue {
	switch v := val.(type) {
	case map[string]interface{}:
		table := L.NewTable()
		for k, val := range v {
			L.RawSet(table, lua.LString(k), s.goValueToLua(L, val))
		}
		return table
	case []interface{}:
		table := L.NewTable()
		for i, val := range v {
			L.RawSet(table, lua.LNumber(i+1), s.goValueToLua(L, val))
		}
		return table
	case string:
		return lua.LString(v)
	case float64:
		return lua.LNumber(v)
	case bool:
		return lua.LBool(v)
	case nil:
		return lua.LNil
	default:
		return lua.LString(fmt.Sprintf("%v", v))
	}
}

func (s *LuaService) luaValueToGo(val lua.LValue) interface{} {
	switch v := val.(type) {
	case *lua.LTable:
		// Check if it's an array or map
		isArr := true
		maxKey := 0
		v.ForEach(func(k, val lua.LValue) {
			if k.Type() != lua.LTNumber {
				isArr = false
			} else {
				key := int(k.(lua.LNumber))
				if key > maxKey { maxKey = key }
			}
		})

		if isArr && maxKey > 0 {
			arr := make([]interface{}, maxKey)
			v.ForEach(func(k, val lua.LValue) {
				arr[int(k.(lua.LNumber))-1] = s.luaValueToGo(val)
			})
			return arr
		}

		res := make(map[string]interface{})
		v.ForEach(func(k, val lua.LValue) {
			res[k.String()] = s.luaValueToGo(val)
		})
		return res
	case lua.LNumber:
		return float64(v)
	case lua.LString:
		return string(v)
	case lua.LBool:
		return bool(v)
	default:
		return nil
	}
}
