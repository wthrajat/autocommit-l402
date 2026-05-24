package main

import (
	"fmt"
	"io"
	"net/http"
	"strings"
)

func handle(w http.ResponseWriter, req *http.Request) {
	// Create request to OpenAI
	url := "https://api.openai.com" + req.URL.Path
	if req.URL.RawQuery != "" {
		url += "?" + req.URL.RawQuery
	}

	outReq, err := http.NewRequest(req.Method, url, req.Body)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	// Copy headers, cleaning Authorization
	for name, values := range req.Header {
		if strings.EqualFold(name, "Authorization") {
			// Find the one starting with Bearer
			var bearerVal string
			for _, val := range values {
				if strings.HasPrefix(strings.TrimSpace(val), "Bearer ") {
					bearerVal = val
					break
				}
			}
			if bearerVal != "" {
				outReq.Header.Set("Authorization", bearerVal)
			}
		} else {
			for _, val := range values {
				outReq.Header.Add(name, val)
			}
		}
	}

	// Set Host header correctly
	outReq.Host = "api.openai.com"

	// Forward to OpenAI
	client := &http.Client{}
	resp, err := client.Do(outReq)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	defer resp.Body.Close()

	// Copy response headers
	for name, values := range resp.Header {
		for _, val := range values {
			w.Header().Add(name, val)
		}
	}
	w.WriteHeader(resp.StatusCode)

	// Copy response body
	io.Copy(w, resp.Body)
}

func main() {
	http.HandleFunc("/", handle)
	fmt.Println("Aperture Header Sanitizer listening on http://127.0.0.1:18080")
	if err := http.ListenAndServe("127.0.0.1:18080", nil); err != nil {
		fmt.Printf("error: %v\n", err)
	}
}
