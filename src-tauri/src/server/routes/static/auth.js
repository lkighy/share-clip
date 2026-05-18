const next = new URLSearchParams(location.search).get("next") || "/";
const message = document.getElementById("message");
const requestState = document.getElementById("request-state");

function selectedScopes() {
  return Array.from(document.querySelectorAll("input[name=scope]:checked")).map((item) => item.value);
}

function setMessage(text, ok = false) {
  message.textContent = text || "";
  message.className = ok ? "state ok" : "state error";
}

async function postJson(url, body) {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const payload = await response.json().catch(() => ({ error: "请求失败" }));
  if (!response.ok) throw new Error(payload.error || payload.message || "请求失败");
  return payload;
}

document.getElementById("password-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  setMessage("");
  try {
    const password = document.getElementById("password").value;
    await postJson("/auth/password", { password, scopes: selectedScopes() });
    setMessage("已授权，正在跳转...", true);
    location.href = next;
  } catch (error) {
    setMessage(error instanceof Error ? error.message : "授权失败");
  }
});

document.getElementById("request-button")?.addEventListener("click", async () => {
  const button = document.getElementById("request-button");
  button.disabled = true;
  setMessage("");
  requestState.textContent = "正在发送请求...";
  try {
    const request = await postJson("/auth/request", {
      scopes: selectedScopes(),
      client_label: navigator.userAgent || "Web 浏览器",
    });
    requestState.textContent = "已发送，等待桌面端确认...";
    const timer = window.setInterval(async () => {
      try {
        const response = await fetch(`/auth/status/${encodeURIComponent(request.id)}`);
        const status = await response.json();
        if (status.auth_status === 2) {
          window.clearInterval(timer);
          requestState.textContent = "已授权，正在跳转...";
          requestState.className = "state ok";
          location.href = next;
        } else if (status.auth_status === 3) {
          window.clearInterval(timer);
          button.disabled = false;
          requestState.textContent = "授权已拒绝";
          requestState.className = "state error";
        } else if (status.auth_status === 4) {
          window.clearInterval(timer);
          button.disabled = false;
          requestState.textContent = "授权请求已超时";
          requestState.className = "state error";
        }
      } catch (error) {
        window.clearInterval(timer);
        button.disabled = false;
        requestState.textContent = error instanceof Error ? error.message : "查询授权状态失败";
        requestState.className = "state error";
      }
    }, 1200);
  } catch (error) {
    button.disabled = false;
    requestState.textContent = error instanceof Error ? error.message : "发送授权请求失败";
    requestState.className = "state error";
  }
});
