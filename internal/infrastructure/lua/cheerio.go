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
		"find":     s.cheerioFind,
		"filter":   s.cheerioFilter,
		"not":      s.cheerioNot,
		"has":      s.cheerioHas,
		"children": s.cheerioChildren,
		"parent":   s.cheerioParent,
		"parents":  s.cheerioParents,
		"closest":  s.cheerioClosest,
		"siblings": s.cheerioSiblings,
		"next":     s.cheerioNext,
		"prev":     s.cheerioPrev,
		"nextAll":  s.cheerioNextAll,
		"prevAll":  s.cheerioPrevAll,
		"first":    s.cheerioFirst,
		"last":     s.cheerioLast,
		"eq":       s.cheerioEq,
		"slice":    s.cheerioSlice,
		"get":      s.cheerioGet,
		"text":     s.cheerioText,
		"html":     s.cheerioHtml,
		"attr":     s.cheerioAttr,
		"data":     s.cheerioData,
		"val":      s.cheerioVal,
		"each":     s.cheerioEach,
		"map":      s.cheerioMap,
		"len":      s.cheerioLength,
		"is":       s.cheerioIs,
		"hasClass": s.cheerioHasClass,
		"add":      s.cheerioAdd,
		"addBack":  s.cheerioAddBack,
		"end":      s.cheerioEnd,
	}))

	L.SetGlobal("cheerio", L.NewTable())
	L.SetField(L.GetGlobal("cheerio"), "load", L.NewFunction(s.cheerioLoad))
}

func pushSelection(L *lua.LState, sel *goquery.Selection) {
	ud := L.NewUserData()
	ud.Value = sel
	L.SetMetatable(ud, L.GetTypeMetatable("cheerio_selection"))
	L.Push(ud)
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
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioFilter(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	selector := L.CheckString(2)
	newSel := sel.Filter(selector)
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioNot(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	selector := L.CheckString(2)
	newSel := sel.Not(selector)
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioHas(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	selector := L.CheckString(2)
	newSel := sel.Has(selector)
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioChildren(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.Children()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioParent(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.Parent()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioParents(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.Parents()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioClosest(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	selector := L.CheckString(2)
	newSel := sel.Closest(selector)
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioSiblings(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.Siblings()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioNext(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.Next()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioPrev(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.Prev()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioNextAll(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.NextAll()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioPrevAll(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.PrevAll()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioFirst(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.First()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioLast(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.Last()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioEq(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	index := L.CheckInt(2)
	newSel := sel.Eq(index)
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioSlice(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	start := L.CheckInt(2)
	end := L.OptInt(3, -1)
	newSel := sel.Slice(start, end)
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioGet(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	index := L.CheckInt(2)
	node := sel.Get(index)
	if node == nil {
		L.Push(lua.LNil)
		return 1
	}
	// Create a new selection from the node and get its HTML
	newSel := goquery.NewDocumentFromNode(node)
	html, _ := newSel.Html()
	L.Push(lua.LString(html))
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

func (s *LuaService) cheerioData(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	key := L.CheckString(2)
	val, exists := sel.Attr("data-" + key)
	if !exists {
		L.Push(lua.LNil)
		return 1
	}
	L.Push(lua.LString(val))
	return 1
}

func (s *LuaService) cheerioVal(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	val, exists := sel.Attr("value")
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

func (s *LuaService) cheerioIs(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	selector := L.CheckString(2)
	result := sel.Is(selector)
	L.Push(lua.LBool(result))
	return 1
}

func (s *LuaService) cheerioHasClass(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	class := L.CheckString(2)
	result := sel.HasClass(class)
	L.Push(lua.LBool(result))
	return 1
}

func (s *LuaService) cheerioAdd(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	selector := L.CheckString(2)
	newSel := sel.AddSelection(sel.Find(selector))
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioAddBack(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.AddBack()
	pushSelection(L, newSel)
	return 1
}

func (s *LuaService) cheerioEnd(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	newSel := sel.End()
	pushSelection(L, newSel)
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
		newUd := L.NewUserData()
		newUd.Value = s
		L.SetMetatable(newUd, L.GetTypeMetatable("cheerio_selection"))

		L.Push(callback)
		L.Push(lua.LNumber(i))
		L.Push(newUd)
		L.Call(2, 0)
	})

	return 0
}

func (s *LuaService) cheerioMap(L *lua.LState) int {
	ud := L.CheckUserData(1)
	sel, ok := ud.Value.(*goquery.Selection)
	if !ok {
		return 0
	}
	callback := L.CheckFunction(2)

	result := L.NewTable()
	sel.Each(func(i int, s *goquery.Selection) {
		newUd := L.NewUserData()
		newUd.Value = s
		L.SetMetatable(newUd, L.GetTypeMetatable("cheerio_selection"))

		L.Push(callback)
		L.Push(lua.LNumber(i))
		L.Push(newUd)
		L.Call(2, 1)

		L.RawSet(result, lua.LNumber(i+1), L.Get(-1))
		L.Pop(1)
	})

	L.Push(result)
	return 1
}
