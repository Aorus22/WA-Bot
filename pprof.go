package main

import (
	"log"
	"net/http"
	_ "net/http/pprof"
)

func init() {
	go func() {
		log.Println("pprof listening on port 6060")
		log.Println(http.ListenAndServe(":6060", nil))
	}()
}