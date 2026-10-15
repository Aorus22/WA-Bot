"""Auth fixtures for the mock backend: an unlinked account that can pair, plus switches to simulate failures.

    mb = MockBackend(areas=["auth"]).start()      # unlinked by default (logged_in=False)
    mb.state["logged_in"] = True                   # linked (phone in mb.state["phone"])
    mb.state["status_fail"] = 503                  # GET /status answers this HTTP status (simulates a sick backend)
    mb.state["status_delay"] = 0.5                 # seconds before /status answers (keeps the UI in "checking")
    mb.state["qr"] = "2@..."                       # what GET /qr-code returns; "" = no QR yet
    mb.state["logout_fail"] = True                 # POST /logout answers 500
    auth_success(mb) / logged_out(mb) / push_qr(mb, code)   # drive the WS events like the Go backend does
"""
from __future__ import annotations

import time

QR_A = "2@4xKJvGq0mZpYH1nQ8bFwTsRLc9dEaUo7VtXiN3hMkPjy5ZBe6WfSrDgCAlOuI2vqz1+8Xb7cYtHn0Ka9wMdP3JeLfRgQ5sVoUiZyBTAhNx4CmEj6kWlO,Fm9aXq2VnTbR8cHjYdKuPe3ZgLsW1oI5vMyNtCxQ7BzAfD0rE4hUiJkSw6lOp=,Y3dXn0VcAq8TbRzFhKjLmEuP2oWi7gYsNtB5vCxQeD1aZrHk4UfJlSw9OyM6pI==,k8Gt0q1Hn5Xc3RzVbJmAe7WdPu2LsYoNfCiQ4TlBxZa6KrEh9vUjMw=="
QR_B = "2@Rz7LqNw3VcT5bHjYdKuPe9ZgXsF1oI4aMyBtCxQ0DnAfE8rU2hWiJkSl6OpGv,Tn4aXq2VmEbR8cHjYdKuPe3ZgLsW1oI5vMyNtCxQ7BzAfD0rE4hUiJkSw6lOp=,Y3dXn0VcAq8TbRzFhKjLmEuP2oWi7gYsNtB5vCxQeD1aZrHk4UfJlSw9OyM6pI==,k8Gt0q1Hn5Xc3RzVbJmAe7WdPu2LsYoNfCiQ4TlBxZa6KrEh9vUjMw=="


def install(mb) -> None:
    st = mb.state
    st["logged_in"] = False
    st["phone"] = "628123456789"
    st["qr"] = QR_A

    @mb.route("GET", "/status")
    def status(req):
        if st.get("status_delay"):
            time.sleep(float(st["status_delay"]))
        if st.get("status_fail"):
            return int(st["status_fail"]), {"error": "backend is warming up"}
        return 200, {"isLoggedIn": st["logged_in"], "phone": st["phone"] if st["logged_in"] else ""}

    @mb.route("GET", "/qr-code")
    def qr(req):
        return 200, {"code": "" if st["logged_in"] else st.get("qr", "")}

    @mb.route("POST", "/logout")
    def logout(req):
        if st.get("logout_fail"):
            return 500, {"error": "logout exploded"}
        st["logged_in"] = False
        st["qr"] = ""  # the Go backend clears the cached QR until pairing restarts
        mb.broadcast("logged_out")
        return 200, {"status": "ok"}


def push_qr(mb, code: str = QR_B) -> None:
    mb.state["qr"] = code
    mb.broadcast("qr_code", {"code": code})


def auth_success(mb, phone: str | None = None) -> None:
    mb.state["logged_in"] = True
    if phone:
        mb.state["phone"] = phone
    mb.broadcast("auth_success", {"phone": mb.state["phone"]})


def logged_out(mb) -> None:
    mb.state["logged_in"] = False
    mb.broadcast("logged_out")
