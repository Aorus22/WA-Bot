package lua

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"time"

	lua "github.com/yuin/gopher-lua"
	"github.com/aorus22/instagramdl"
	"wa-bot/internal/domain/entity"
	"wa-bot/internal/domain/repository"
	"wa-bot/internal/infrastructure/ai"
	"wa-bot/internal/infrastructure/media"
	"wa-bot/internal/infrastructure/util"
	"wa-bot/internal/infrastructure/whatsapp"
)

type LuaService struct {
	waClient    *whatsapp.WhatsAppClient
	triggerRepo repository.TriggerRepository
	stateRepo   repository.UserStateRepository
	storage     repository.StorageRepository
	gemini      *ai.GeminiService
	mediaDown   *media.MediaDownloader
}

func NewLuaService(
	waClient *whatsapp.WhatsAppClient,
	triggerRepo repository.TriggerRepository,
	stateRepo repository.UserStateRepository,
	storage repository.StorageRepository,
	gemini *ai.GeminiService,
	mediaDown *media.MediaDownloader,
) *LuaService {
	return &LuaService{
		waClient:    waClient,
		triggerRepo: triggerRepo,
		stateRepo:   stateRepo,
		storage:     storage,
		gemini:      gemini,
		mediaDown:   mediaDown,
	}
}

func (s *LuaService) ExecuteTriggers(ctx context.Context, msg *entity.Message) (bool, error) {
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
			fmt.Printf("[LUA] Invalid regex pattern %s: %v\n", t.Pattern, err)
			continue
		}

		if re.MatchString(msg.Text) {
			matches := re.FindStringSubmatch(msg.Text)
			go s.runScript(t.Script, msg, matches)
			return true, nil
		}
	}

	return false, nil
}

func (s *LuaService) runScript(script string, msg *entity.Message, matches []string) {
	L := lua.NewState()
	defer L.Close()

	// Inject globals
	L.SetGlobal("sender", lua.LString(msg.SenderJID))
	L.SetGlobal("content", lua.LString(msg.Text))

	// Inject entire message state as a table
	msgTable := L.NewTable()
	L.RawSet(msgTable, lua.LString("id"), lua.LString(msg.ID))
	L.RawSet(msgTable, lua.LString("sender"), lua.LString(msg.SenderJID))
	L.RawSet(msgTable, lua.LString("chat_id"), lua.LString(msg.ChatID))
	L.RawSet(msgTable, lua.LString("content"), lua.LString(msg.Text))
	L.RawSet(msgTable, lua.LString("type"), lua.LString(msg.Type))
	L.RawSet(msgTable, lua.LString("media_url"), lua.LString(msg.MediaURL))
	L.RawSet(msgTable, lua.LString("timestamp"), lua.LNumber(msg.Timestamp.Unix()))
	L.RawSet(msgTable, lua.LString("is_group"), lua.LBool(msg.IsGroup))
	
	isMedia := msg.Type == "image" || msg.Type == "video" || msg.Type == "sticker" || msg.Type == "document"
	L.RawSet(msgTable, lua.LString("is_media"), lua.LBool(isMedia))
	
	L.SetGlobal("msg", msgTable)

	luaMatches := L.NewTable()
	for i, m := range matches {
		L.RawSet(luaMatches, lua.LNumber(i), lua.LString(m))
	}
	L.SetGlobal("matches", luaMatches)

	// Inject functions
	L.SetGlobal("send_text", L.NewFunction(s.luaSendText))
	L.SetGlobal("send_sticker", L.NewFunction(s.luaSendSticker))
	L.SetGlobal("send_media", L.NewFunction(s.luaSendMedia))
	
	// download_media needs msg context, so we use a closure here
	L.SetGlobal("download_media", L.NewFunction(func(L *lua.LState) int {
		filename := L.CheckString(1)
		data, _, err := s.waClient.DownloadMedia(context.Background(), msg)
		if err != nil {
			L.Push(lua.LNil)
			L.Push(lua.LString(err.Error()))
			return 2
		}
		savedPath, err := s.storage.Save(context.Background(), filename, bytes.NewReader(data))
		if err != nil {
			L.Push(lua.LNil)
			L.Push(lua.LString(err.Error()))
			return 2
		}
		L.Push(lua.LString(savedPath))
		return 1
	}))

	L.SetGlobal("fetch", L.NewFunction(s.luaFetch))
	L.SetGlobal("fetch_to_file", L.NewFunction(s.luaFetchToFile))
	L.SetGlobal("gemini_chat", L.NewFunction(s.luaGeminiChat))
	L.SetGlobal("get_instagram_url", L.NewFunction(s.luaGetInstagramURL))
	L.SetGlobal("get_state", L.NewFunction(s.luaGetState))
	L.SetGlobal("set_state", L.NewFunction(s.luaSetState))

	// Media functions
	L.SetGlobal("get_duration", L.NewFunction(s.luaGetDuration))
	L.SetGlobal("get_mime_type", L.NewFunction(s.luaGetMimeType))

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
	L.SetGlobal("ffprobe_json", L.NewFunction(s.luaFFprobeJSON))
	L.SetGlobal("yt_dlp", L.NewFunction(s.luaYtDlp))
	L.SetGlobal("gallery_dl", L.NewFunction(s.luaGalleryDlp))
	L.SetGlobal("webpmux", L.NewFunction(s.luaWebpmux))
	L.SetGlobal("whatsapp_exif", L.NewFunction(s.luaWhatsappExif))

	if err := L.DoString(script); err != nil {
		fmt.Printf("[LUA] Script Error: %v\n", err)
	}
}

func (s *LuaService) luaSendText(L *lua.LState) int {
	target := L.CheckString(1)
	text := L.CheckString(2)

	_, err := s.waClient.SendMessage(context.Background(), target, text, true)
	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}
	return 0
}

func (s *LuaService) luaSendSticker(L *lua.LState) int {
	target := L.CheckString(1)
	urlOrPath := L.CheckString(2)

	var data []byte
	var err error

	if strings.HasPrefix(urlOrPath, "http://") || strings.HasPrefix(urlOrPath, "https://") {
		resp, hErr := http.Get(urlOrPath)
		if hErr != nil {
			L.Push(lua.LString(hErr.Error()))
			return 1
		}
		defer resp.Body.Close()
		data, err = io.ReadAll(resp.Body)
	} else {
		safePath := s.storage.GetPath(urlOrPath)
		data, err = os.ReadFile(safePath)
	}

	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}

	_, err = s.waClient.SendSticker(context.Background(), target, data, false, urlOrPath, true)
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

func (s *LuaService) luaFetchToFile(L *lua.LState) int {
	url := L.CheckString(1)
	filename := L.CheckString(2)
	options := L.OptTable(3, nil)

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

	client := &http.Client{Timeout: 30 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {
		L.Push(lua.LNil)
		L.Push(lua.LString(fmt.Sprintf("HTTP Error: %d", resp.StatusCode)))
		return 2
	}

	_, err = s.storage.Save(context.Background(), filename, resp.Body)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}

	resTable := L.NewTable()
	L.RawSet(resTable, lua.LString("status"), lua.LNumber(resp.StatusCode))
	L.RawSet(resTable, lua.LString("path"), lua.LString(s.storage.GetPath(filename)))
	L.RawSet(resTable, lua.LString("filename"), lua.LString(filename))

	L.Push(resTable)
	return 1
}

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

func (s *LuaService) luaFFmpeg(L *lua.LState) int {
	argsTable := L.CheckTable(1)
	var args []string
	argsTable.ForEach(func(k, v lua.LValue) {
		args = append(args, v.String())
	})

	cmd := exec.Command(util.GetBinaryPath("ffmpeg"), args...)
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

	cmd := exec.Command(util.GetBinaryPath("ffprobe"), args...)
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

	L.SetGlobal("sender", lua.LString("628123456789@s.whatsapp.net"))
	L.SetGlobal("content", lua.LString(message))

	// Inject entire message state as a table
	msgTable := L.NewTable()
	L.RawSet(msgTable, lua.LString("id"), lua.LString("ABC123456789"))
	L.RawSet(msgTable, lua.LString("sender"), lua.LString("628123456789@s.whatsapp.net"))
	L.RawSet(msgTable, lua.LString("chat_id"), lua.LString("628123456789@s.whatsapp.net"))
	L.RawSet(msgTable, lua.LString("content"), lua.LString(message))
	L.RawSet(msgTable, lua.LString("type"), lua.LString("text"))
	L.RawSet(msgTable, lua.LString("media_url"), lua.LString(""))
	L.RawSet(msgTable, lua.LString("timestamp"), lua.LNumber(time.Now().Unix()))
	L.RawSet(msgTable, lua.LString("is_group"), lua.LBool(false))
	L.SetGlobal("msg", msgTable)

	luaMatches := L.NewTable()
	for i, m := range matches {
		L.RawSet(luaMatches, lua.LNumber(i), lua.LString(m))
	}
	L.SetGlobal("matches", luaMatches)

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
	L.SetGlobal("download_media", L.NewFunction(func(L *lua.LState) int {
		filename := L.CheckString(1)
		actions = append(actions, fmt.Sprintf("Action: download_media(filename: %s)", filename))
		L.Push(lua.LString("media/" + filename))
		return 1
	}))
	L.SetGlobal("gemini_chat", L.NewFunction(func(L *lua.LState) int {
		prompt := L.CheckString(1)
		modelName := L.OptString(2, "gemini-2.0-flash")
		filePath := L.OptString(3, "")
		actions = append(actions, fmt.Sprintf("Action: gemini_chat(prompt: %s, model: %s, file: %s)", prompt, modelName, filePath))
		L.Push(lua.LString("[MOCK GEMINI RESPONSE]"))
		return 1
	}))

	L.SetGlobal("get_duration", L.NewFunction(func(L *lua.LState) int {
		L.Push(lua.LNumber(10.5))
		return 1
	}))
	L.SetGlobal("get_mime_type", L.NewFunction(func(L *lua.LState) int {
		L.Push(lua.LString("image/jpeg"))
		return 1
	}))
	L.SetGlobal("get_instagram_url", L.NewFunction(func(L *lua.LState) int {
		L.Push(lua.LString("https://instagram.fsub1-1.fna.fbcdn.net/v/..."))
		return 1
	}))

	L.SetGlobal("fetch", L.NewFunction(s.luaFetch))
	L.SetGlobal("fetch_to_file", L.NewFunction(s.luaFetchToFile))
	L.SetGlobal("get_state", L.NewFunction(s.luaGetState))
	L.SetGlobal("set_state", L.NewFunction(s.luaSetState))
	L.SetGlobal("json_decode", L.NewFunction(s.luaJSONDecode))
	L.SetGlobal("json_encode", L.NewFunction(s.luaJSONEncode))
	L.SetGlobal("storage_save", L.NewFunction(s.luaStorageSave))
	L.SetGlobal("storage_get", L.NewFunction(s.luaStorageGet))
	L.SetGlobal("storage_delete", L.NewFunction(s.luaStorageDelete))
	L.SetGlobal("storage_exists", L.NewFunction(s.luaStorageExists))
	L.SetGlobal("storage_path", L.NewFunction(s.luaStoragePath))
	L.SetGlobal("ffmpeg", L.NewFunction(s.luaFFmpeg))
	L.SetGlobal("ffprobe", L.NewFunction(s.luaFFprobe))
	L.SetGlobal("ffprobe_json", L.NewFunction(s.luaFFprobeJSON))
	L.SetGlobal("yt_dlp", L.NewFunction(s.luaYtDlp))
	L.SetGlobal("gallery_dl", L.NewFunction(s.luaGalleryDlp))
	L.SetGlobal("webpmux", L.NewFunction(s.luaWebpmux))
	L.SetGlobal("whatsapp_exif", L.NewFunction(s.luaWhatsappExif))

	if err := L.DoString(script); err != nil {
		result["error"] = err.Error()
	}

	result["logs"] = logs
	result["actions"] = actions

	return result, nil
}

func (s *LuaService) luaGetInstagramURL(L *lua.LState) int {
	url := L.CheckString(1)
	fmt.Printf("[LUA] Fetching Instagram URL: %s\n", url)
	urls, err := instagramdl.GetInstagramMediaURLs(url)
	if err != nil {
		fmt.Printf("[LUA] Instagram error: %v\n", err)
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	if len(urls) == 0 {
		fmt.Printf("[LUA] Instagram error: no media found\n")
		L.Push(lua.LNil)
		L.Push(lua.LString("no media found in instagram post"))
		return 2
	}
	fmt.Printf("[LUA] Instagram success: %s\n", urls[0])
	L.Push(lua.LString(urls[0]))
	return 1
}

func (s *LuaService) luaSendMedia(L *lua.LState) int {
	target := L.CheckString(1)
	urlOrPath := L.CheckString(2)
	mediaType := L.OptString(3, "image")
	caption := L.OptString(4, "")

	var data []byte
	var err error

	if strings.HasPrefix(urlOrPath, "http://") || strings.HasPrefix(urlOrPath, "https://") {
		resp, hErr := http.Get(urlOrPath)
		if hErr != nil {
			L.Push(lua.LString(hErr.Error()))
			return 1
		}
		defer resp.Body.Close()
		data, err = io.ReadAll(resp.Body)
	} else {
		// Use storage gatekeeper
		safePath := s.storage.GetPath(urlOrPath)
		data, err = os.ReadFile(safePath)
	}

	if err != nil {
		L.Push(lua.LString(err.Error()))
		return 1
	}

	ctx := context.Background()
	var sendErr error
	switch mediaType {
	case "image":
		_, sendErr = s.waClient.SendImage(ctx, target, data, caption, "", true)
	case "video":
		_, sendErr = s.waClient.SendVideo(ctx, target, data, caption, "", true)
	case "document":
		_, sendErr = s.waClient.SendDocument(ctx, target, data, "document", "", true)
	default:
		sendErr = fmt.Errorf("unsupported media type: %s", mediaType)
	}

	if sendErr != nil {
		L.Push(lua.LString(sendErr.Error()))
		return 1
	}
	return 0
}

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
		isArr := true
		maxKey := 0
		v.ForEach(func(k, val lua.LValue) {
			if k.Type() != lua.LTNumber {
				isArr = false
			} else {
				key := int(k.(lua.LNumber))
				if key > maxKey {
					maxKey = key
				}
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

func (s *LuaService) luaDownloadMedia(L *lua.LState) int {
	// Note: In runScript, this is overridden by a closure that captures the actual message.
	// This method serves as a fallback or for shared structure.
	L.Push(lua.LNil)
	L.Push(lua.LString("download_media requires message context"))
	return 2
}

func (s *LuaService) luaGetDuration(L *lua.LState) int {
	path := L.CheckString(1)
	safePath := s.storage.GetPath(path)
	duration, err := s.mediaDown.GetDuration(safePath)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	L.Push(lua.LNumber(duration))
	return 1
}

func (s *LuaService) luaGetMimeType(L *lua.LState) int {
	path := L.CheckString(1)
	safePath := s.storage.GetPath(path)
	mime, err := s.mediaDown.GetMimeType(safePath)
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	L.Push(lua.LString(mime))
	return 1
}

func (s *LuaService) luaFFprobeJSON(L *lua.LState) int {
	argsTable := L.CheckTable(1)
	var args []string
	argsTable.ForEach(func(k, v lua.LValue) {
		args = append(args, v.String())
	})

	// Add JSON flags if not present
	args = append(args, "-print_format", "json", "-show_format", "-show_streams")

	cmd := exec.Command(util.GetBinaryPath("ffprobe"), args...)
	output, err := cmd.Output()
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}

	var data interface{}
	if err := json.Unmarshal(output, &data); err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}

	L.Push(s.goValueToLua(L, data))
	return 1
}

func (s *LuaService) luaYtDlp(L *lua.LState) int {
	argsTable := L.CheckTable(1)
	var args []string
	argsTable.ForEach(func(k, v lua.LValue) {
		args = append(args, v.String())
	})

	// Security: Resolve file paths in common flags
	for i, arg := range args {
		if (arg == "-o" || arg == "--output" || arg == "--cookies") && i+1 < len(args) {
			args[i+1] = s.storage.GetPath(args[i+1])
		}
	}

	cmd := exec.Command(util.GetBinaryPath("yt-dlp"), args...)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	output, err := cmd.Output()
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(fmt.Sprintf("%s", stderr.String())))
		return 2
	}

	L.Push(lua.LString(string(output)))
	return 1
}

func (s *LuaService) luaGalleryDlp(L *lua.LState) int {
	argsTable := L.CheckTable(1)
	var args []string
	argsTable.ForEach(func(k, v lua.LValue) {
		args = append(args, v.String())
	})

	// Security: Force directory to storage or resolve specific flags
	hasDir := false
	for i, arg := range args {
		if arg == "-D" || arg == "--directory" {
			if i+1 < len(args) {
				args[i+1] = s.storage.GetPath(args[i+1])
				hasDir = true
			}
		}
		if arg == "--cookies" && i+1 < len(args) {
			args[i+1] = s.storage.GetPath(args[i+1])
		}
	}

	if !hasDir {
		args = append(args, "-D", s.storage.GetPath(""))
	}

	cmd := exec.Command(util.GetBinaryPath("gallery-dl"), args...)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	output, err := cmd.Output()
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(fmt.Sprintf("%s", stderr.String())))
		return 2
	}

	L.Push(lua.LString(string(output)))
	return 1
}

func (s *LuaService) luaWebpmux(L *lua.LState) int {
	argsTable := L.CheckTable(1)
	var args []string
	argsTable.ForEach(func(k, v lua.LValue) {
		args = append(args, v.String())
	})

	// Security: Resolve paths in args if they look like filenames
	for i, arg := range args {
		if !strings.HasPrefix(arg, "-") && len(arg) > 0 {
			// If it's a file argument, make it safe
			if _, err := os.Stat(s.storage.GetPath(arg)); err == nil {
				args[i] = s.storage.GetPath(arg)
			}
		}
	}

	cmd := exec.Command(util.GetBinaryPath("webpmux"), args...)
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

func (s *LuaService) luaWhatsappExif(L *lua.LState) int {
	packName := L.CheckString(1)
	author := L.CheckString(2)

	// WhatsApp Sticker Exif Format (TIFF based)
	startingBytes := []byte{0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x41, 0x57, 0x07, 0x00}
	endingBytes := []byte{0x16, 0x00, 0x00, 0x00}

	meta := map[string]interface{}{
		"sticker-pack-id":        "site.alyza.lua",
		"sticker-pack-name":      packName,
		"sticker-pack-publisher": author,
	}
	jsonBytes, _ := json.Marshal(meta)

	// Length of JSON must be in LittleEndian (4 bytes)
	lenBuf := make([]byte, 4)
	importBinary := func() {
		// We'll use a manual approach to avoid complex imports if possible
		length := uint32(len(jsonBytes))
		lenBuf[0] = byte(length)
		lenBuf[1] = byte(length >> 8)
		lenBuf[2] = byte(length >> 16)
		lenBuf[3] = byte(length >> 24)
	}
	importBinary()

	var b bytes.Buffer
	b.Write(startingBytes)
	b.Write(lenBuf)
	b.Write(endingBytes)
	b.Write(jsonBytes)

	L.Push(lua.LString(b.String()))
	return 1
}

func (s *LuaService) luaGeminiChat(L *lua.LState) int {
	prompt := L.CheckString(1)
	modelName := L.OptString(2, "gemini-2.0-flash")
	filePath := L.OptString(3, "")

	if s.gemini == nil {
		L.Push(lua.LNil)
		L.Push(lua.LString("Gemini service not available"))
		return 2
	}

	ctx := context.Background()
	var res string
	var err error

	if filePath != "" {
		safePath := s.storage.GetPath(filePath)
		res, err = s.gemini.GenerateWithFile(ctx, modelName, prompt, safePath)
	} else {
		res, err = s.gemini.GenerateText(ctx, modelName, prompt)
	}

	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}

	L.Push(lua.LString(res))
	return 1
}
