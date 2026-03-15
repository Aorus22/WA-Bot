package lua

import (
	"bytes"
	"context"
	"os"
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

	var allocCtx context.Context
	var cancel1 context.CancelFunc

	cdpURL := os.Getenv("LIGHTPANDA_CDP_URL")
	if cdpURL != "" {
		// Use Remote Allocator for Lightpanda CDP
		allocCtx, cancel1 = chromedp.NewRemoteAllocator(context.Background(), cdpURL)
	} else {
		// Use Local Allocator (Chromium must be installed)
		opts := append(chromedp.DefaultExecAllocatorOptions[:],
			chromedp.Flag("headless", headless),
			chromedp.Flag("disable-gpu", true),
			chromedp.Flag("no-sandbox", true),
			chromedp.Flag("disable-dev-shm-usage", true),
			chromedp.Flag("ignore-certificate-errors", true),
		)
		if chromeBin := os.Getenv("CHROME_BIN"); chromeBin != "" {
			opts = append(opts, chromedp.ExecPath(chromeBin))
		}
		allocCtx, cancel1 = chromedp.NewExecAllocator(context.Background(), opts...)
	}
	defer cancel1()

	ctx, cancel2 := chromedp.NewContext(allocCtx)
	defer cancel2()

	// Set timeout
	ctx, cancel3 := context.WithTimeout(ctx, 120*time.Second)
	defer cancel3()

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

		case "press_key":
			sel := getString(tbl, "selector")
			key := getString(tbl, "key")
			if sel != "" {
				actions = append(actions, chromedp.SendKeys(sel, key))
			}

		case "hover":
			sel := getString(tbl, "selector")
			actions = append(actions, chromedp.Hover(sel))

		case "evaluate":
			script := getString(tbl, "script")
			key := getString(tbl, "key")
			if key == "" {
				key = "eval_result"
			}
			actions = append(actions, chromedp.Evaluate(script, nil, func(p *chromedp.EvaluateParams) *chromedp.EvaluateParams {
				return p.WithAwaitPromise(true)
			}))
			// Note: For complex return values, we'd need more logic, 
			// but for now let's support simple string returns via a different pattern if needed.
			// Implementing a simplified version that captures the result:
			var res interface{}
			actions = append(actions, chromedp.Evaluate(script, &res))
			actions = append(actions, chromedp.ActionFunc(func(c context.Context) error {
				results[key] = fmt.Sprintf("%v", res)
				return nil
			}))

		case "attribute":
			sel := getString(tbl, "selector")
			attr := getString(tbl, "attribute")
			key := getString(tbl, "key")
			if key == "" {
				key = "attr_" + attr
			}
			actions = append(actions, chromedp.ActionFunc(func(c context.Context) error {
				var val string
				var ok bool
				err := chromedp.AttributeValue(sel, attr, &val, &ok).Do(c)
				if err == nil && ok {
					results[key] = val
				}
				return err
			}))

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
			
			actions = append(actions, chromedp.ActionFunc(func(c context.Context) error {
				var buf []byte
				var err error
				if sel != "" {
					err = chromedp.Screenshot(sel, &buf, chromedp.NodeVisible).Do(c)
				} else {
					// Use CaptureScreenshot instead of FullScreenshot for better compatibility
					// format defaults to png
					err = chromedp.CaptureScreenshot(&buf).Do(c)
				}
				
				if err != nil {
					// Don't fail the whole chain, just log the error
					results[filename] = "Screenshot failed: " + err.Error()
					return nil 
				}
				
				// Save to storage
				_, err = s.storage.Save(context.Background(), filename, bytes.NewReader(buf))
				if err == nil {
					results[filename] = s.storage.GetPath(filename)
				}
				return nil
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
