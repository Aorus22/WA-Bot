package middleware

import (
	"net/http"
	"os"

	"github.com/gorilla/mux"
)

func Auth(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusOK)
			return
		}

		secret := r.Header.Get("X-API-Secret")
		if secret == "" {
			secret = r.URL.Query().Get("secret")
		}
		if secret == "" && r.Method == "POST" {
			_ = r.ParseMultipartForm(10 << 20)
			secret = r.FormValue("secret")
		}

		expectedSecret := os.Getenv("API_SECRET")
		// Fail closed: if no secret is configured there is nothing to validate
		// against, so refuse the request rather than falling back to a known
		// default (a default secret is not a defense).
		if expectedSecret == "" || secret != expectedSecret {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusUnauthorized)
			w.Write([]byte(`{"error":"Unauthorized"}`))
			return
		}

		next(w, r)
	}
}

func OptionalAuth(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		next(w, r)
	}
}

func RequireAuth(route *mux.Route) *mux.Route {
	handler := route.GetHandler()
	var handlerFunc http.HandlerFunc
	if hf, ok := handler.(http.HandlerFunc); ok {
		handlerFunc = hf
	} else {
		handlerFunc = func(w http.ResponseWriter, r *http.Request) {
			handler.ServeHTTP(w, r)
		}
	}
	return route.HandlerFunc(Auth(handlerFunc))
}
