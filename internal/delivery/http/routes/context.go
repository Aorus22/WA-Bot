package routes

import (
	"context"
	"net/http"
	"strconv"
	"time"
)

type ContextKey string

const (
	ContextKeyUserID     ContextKey = "userID"
	ContextKeyRequestID  ContextKey = "requestID"
	ContextKeyStartTime  ContextKey = "startTime"
)

func GetUserID(r *http.Request) string {
	if userID, ok := r.Context().Value(ContextKeyUserID).(string); ok {
		return userID
	}
	return ""
}

func GetRequestID(r *http.Request) string {
	if requestID, ok := r.Context().Value(ContextKeyRequestID).(string); ok {
		return requestID
	}
	return ""
}

func GetStartTime(r *http.Request) time.Time {
	if startTime, ok := r.Context().Value(ContextKeyStartTime).(time.Time); ok {
		return startTime
	}
	return time.Time{}
}

func SetUserID(r *http.Request, userID string) *http.Request {
	ctx := context.WithValue(r.Context(), ContextKeyUserID, userID)
	return r.WithContext(ctx)
}

func SetRequestID(r *http.Request, requestID string) *http.Request {
	ctx := context.WithValue(r.Context(), ContextKeyRequestID, requestID)
	return r.WithContext(ctx)
}

func SetStartTime(r *http.Request, startTime time.Time) *http.Request {
	ctx := context.WithValue(r.Context(), ContextKeyStartTime, startTime)
	return r.WithContext(ctx)
}

func GetIntQueryParam(r *http.Request, key string, defaultValue int) int {
	value := r.URL.Query().Get(key)
	if value == "" {
		return defaultValue
	}
	intValue, err := strconv.Atoi(value)
	if err != nil {
		return defaultValue
	}
	return intValue
}

func GetBoolQueryParam(r *http.Request, key string, defaultValue bool) bool {
	value := r.URL.Query().Get(key)
	if value == "" {
		return defaultValue
	}
	boolValue, err := strconv.ParseBool(value)
	if err != nil {
		return defaultValue
	}
	return boolValue
}

func GetQueryParam(r *http.Request, key string, defaultValue string) string {
	value := r.URL.Query().Get(key)
	if value == "" {
		return defaultValue
	}
	return value
}
