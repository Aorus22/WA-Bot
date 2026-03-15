package lua

import (
	"bytes"
	"context"
	"time"

	"github.com/chromedp/chromedp"
	lua "github.com/yuin/gopher-lua"
)

func (s *LuaService) RegisterBrowser(L *lua.LState) {
	L.SetGlobal("browser", L.NewTable())
	L.SetField(L.GetGlobal("browser"), "run", L.NewFunction(s.browserRun))
}

func (s *LuaService) browserRun(L *lua.LState) int {
	actionsTable := L.CheckTable(1)
	headless := true
	if L.GetTop() > 1 {
		options := L.CheckTable(2)
		if val := options.RawGetString("headless"); val.Type() == lua.LTBool {
			headless = bool(val.(lua.LBool))
		}
	}

	opts := append(chromedp.DefaultExecAllocatorOptions[:],
		chromedp.Flag("headless", headless),
		chromedp.Flag("disable-gpu", true),
		chromedp.Flag("no-sandbox", true),
		chromedp.Flag("ignore-certificate-errors", true),
	)

	allocCtx, cancel := chromedp.NewExecAllocator(context.Background(), opts...)
	defer cancel()

	ctx, cancel := chromedp.NewContext(allocCtx)
	defer cancel()

	// Set timeout
	ctx, cancel = context.WithTimeout(ctx, 120*time.Second)
	defer cancel()

	var actions []chromedp.Action
	results := make(map[string]string)

	// Helper to safely get string from table
	getString := func(tbl *lua.LTable, key string) string {
		val := tbl.RawGetString(key)
		if val.Type() == lua.LTString {
			return val.String()
		}
		return ""
	}

	// Helper to get int from table
	getInt := func(tbl *lua.LTable, key string) int {
		val := tbl.RawGetString(key)
		if val.Type() == lua.LTNumber {
			return int(val.(lua.LNumber))
		}
		return 0
	}

	actionsTable.ForEach(func(k, v lua.LValue) {
		if v.Type() != lua.LTTable {
			return
		}
		tbl := v.(*lua.LTable)
		actionType := getString(tbl, "action")

		switch actionType {
		case "navigate":
			url := getString(tbl, "url")
			actions = append(actions, chromedp.Navigate(url))

		case "wait_visible":
			sel := getString(tbl, "selector")
			actions = append(actions, chromedp.WaitVisible(sel))

		case "click":
			sel := getString(tbl, "selector")
			actions = append(actions, chromedp.Click(sel))

		case "type":
			sel := getString(tbl, "selector")
			text := getString(tbl, "text")
			actions = append(actions, chromedp.SendKeys(sel, text))

		case "text":
			sel := getString(tbl, "selector")
			key := getString(tbl, "key")
			if key == "" {
				key = "text_" + sel
			}
			actions = append(actions, chromedp.ActionFunc(func(c context.Context) error {
				var res string
				err := chromedp.Text(sel, &res).Do(c)
				if err == nil {
					results[key] = res
				}
				return err
			}))

		case "html":
			key := getString(tbl, "key")
			if key == "" {
				key = "html"
			}
			actions = append(actions, chromedp.ActionFunc(func(c context.Context) error {
				var res string
				err := chromedp.OuterHTML("html", &res).Do(c)
				if err == nil {
					results[key] = res
				}
				return err
			}))

		case "screenshot":
			filename := getString(tbl, "filename")
			sel := getString(tbl, "selector")
			quality := getInt(tbl, "quality")
			if quality == 0 {
				quality = 90
			}
			
			actions = append(actions, chromedp.ActionFunc(func(c context.Context) error {
				var buf []byte
				var err error
				if sel != "" {
					err = chromedp.Screenshot(sel, &buf, chromedp.NodeVisible).Do(c)
				} else {
					err = chromedp.FullScreenshot(&buf, quality).Do(c)
				}
				
				if err == nil {
					// Save to storage
					_, err = s.storage.Save(context.Background(), filename, bytes.NewReader(buf))
					if err == nil {
						results[filename] = s.storage.GetPath(filename)
					}
				}
				return err
			}))
			
		case "sleep":
			ms := getInt(tbl, "ms")
			actions = append(actions, chromedp.Sleep(time.Duration(ms)*time.Millisecond))
		}
	})

	if err := chromedp.Run(ctx, actions...); err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}

	resTable := L.NewTable()
	for k, v := range results {
		L.SetField(resTable, k, lua.LString(v))
	}
	L.Push(resTable)
	return 1
}
