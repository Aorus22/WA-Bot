package lua

import (
	"strings"

	"github.com/PuerkitoBio/goquery"
	lua "github.com/yuin/gopher-lua"
)

// RegisterCheerio adds the cheerio module to the Lua state
func (s *LuaService) RegisterCheerio(L *lua.LState) {
	mt := L.NewTypeMetatable("cheerio_selection")
	L.SetField(mt, "__index", L.SetFuncs(L.NewTable(), map[string]lua.LGFunction{
		"find": s.cheerioFind,
		"text": s.cheerioText,
		"html": s.cheerioHtml,
		"attr": s.cheerioAttr,
		"each": s.cheerioEach,
		"len":  s.cheerioLength,
	}))

	L.SetGlobal("cheerio", L.NewTable())
	L.SetField(L.GetGlobal("cheerio"), "load", L.NewFunction(s.cheerioLoad))
}

func (s *LuaService) cheerioLoad(L *lua.LState) int {
	html := L.CheckString(1)
	doc, err := goquery.NewDocumentFromReader(strings.NewReader(html))
	if err != nil {
		L.Push(lua.LNil)
		L.Push(lua.LString(err.Error()))
		return 2
	}
	ud := L.NewUserData()
	ud.Value = doc.Selection
	L.SetMetatable(ud, L.GetTypeMetatable("cheerio_selection"))
	L.Push(ud)
	return 1
}

func (s *LuaService) cheerioFind(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	selector := L.CheckString(2)
	newSel := sel.Find(selector)
	
	newUd := L.NewUserData()
	newUd.Value = newSel
	L.SetMetatable(newUd, L.GetTypeMetatable("cheerio_selection"))
	L.Push(newUd)
	return 1
}

func (s *LuaService) cheerioText(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	L.Push(lua.LString(sel.Text()))
	return 1
}

func (s *LuaService) cheerioHtml(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	html, _ := sel.Html()
	L.Push(lua.LString(html))
	return 1
}

func (s *LuaService) cheerioAttr(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	attrName := L.CheckString(2)
	val, exists := sel.Attr(attrName)
	if !exists {
		L.Push(lua.LNil)
		return 1
	}
	L.Push(lua.LString(val))
	return 1
}

func (s *LuaService) cheerioLength(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	L.Push(lua.LNumber(sel.Length()))
	return 1
}

func (s *LuaService) cheerioEach(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	callback := L.CheckFunction(2)

	sel.Each(func(i int, s *goquery.Selection) {
		// Create a new UserData for the current selection
		newUd := L.NewUserData()
		newUd.Value = s
		L.SetMetatable(newUd, L.GetTypeMetatable("cheerio_selection"))

		// Call the Lua callback function
		L.Push(callback)
		L.Push(lua.LNumber(i))
		L.Push(newUd)
		L.Call(2, 0)
	})

	return 0
}
