const sessionListEl = document.getElementById("session-list");
const paneEl = document.getElementById("pane");
const healthEl = document.getElementById("health");
const refreshBtn = document.getElementById("refresh");
const sendForm = document.getElementById("send-form");
const lineInput = document.getElementById("line");
const enterInput = document.getElementById("enter");
const sendTarget = document.getElementById("send-target");
const sendStatus = document.getElementById("send-status");

let selectedSession = null;

async function api(path, options) {
  const res = await fetch(path, options);
  const text = await res.text();
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    data = text;
  }
  if (!res.ok) {
    const msg =
      data && data.error && data.error.message
        ? data.error.message
        : `HTTP ${res.status}`;
    throw new Error(msg);
  }
  return data;
}

function stateClass(state) {
  return `state-${state}`;
}

function renderSessions(grouped) {
  sessionListEl.innerHTML = "";
  const groups = grouped.groups || [];
  for (const group of groups) {
    if (group.key) {
      const heading = document.createElement("li");
      heading.className = "meta";
      heading.textContent = group.key;
      sessionListEl.appendChild(heading);
    }
    for (const s of group.sessions || []) {
      const li = document.createElement("li");
      const btn = document.createElement("button");
      btn.type = "button";
      btn.dataset.name = s.name;
      btn.setAttribute(
        "aria-current",
        s.name === selectedSession ? "true" : "false"
      );
      btn.innerHTML = `<span class="name">${escapeHtml(s.name)}</span>
        <div class="meta"><span class="${stateClass(s.state)}">${s.state}</span> · ${escapeHtml(s.path)}</div>`;
      btn.addEventListener("click", () => selectSession(s.name));
      li.appendChild(btn);
      sessionListEl.appendChild(li);
    }
  }
  if (!sessionListEl.children.length) {
    sessionListEl.innerHTML = "<li class=\"muted\">No tmux sessions.</li>";
  }
}

function escapeHtml(str) {
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function loadHealth() {
  try {
    const h = await api("/v1/health");
    healthEl.textContent = `v${h.version} · tmux ${h.tmux_reachable ? "ok" : "unreachable"}`;
  } catch (e) {
    healthEl.textContent = `Health check failed: ${e.message}`;
  }
}

async function loadSessions() {
  try {
    const grouped = await api("/v1/sessions");
    renderSessions(grouped);
  } catch (e) {
    sessionListEl.innerHTML = `<li class="muted">${escapeHtml(e.message)}</li>`;
  }
}

async function selectSession(name) {
  selectedSession = name;
  sendForm.hidden = false;
  sendTarget.textContent = name;
  sendStatus.textContent = "";
  await loadSessions();
  paneEl.textContent = "Loading pane…";
  try {
    const pane = await api(`/v1/sessions/${encodeURIComponent(name)}/pane`);
    paneEl.textContent = pane.text || "(empty)";
  } catch (e) {
    paneEl.textContent = e.message;
  }
}

sendForm.addEventListener("submit", async (ev) => {
  ev.preventDefault();
  if (!selectedSession) return;
  sendStatus.textContent = "Sending…";
  try {
    await api(`/v1/sessions/${encodeURIComponent(selectedSession)}/send`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        line: lineInput.value,
        enter: enterInput.checked,
      }),
    });
    sendStatus.textContent = "Sent.";
    lineInput.value = "";
    await selectSession(selectedSession);
  } catch (e) {
    sendStatus.textContent = e.message;
  }
});

refreshBtn.addEventListener("click", async () => {
  await loadHealth();
  await loadSessions();
  if (selectedSession) await selectSession(selectedSession);
});

(async () => {
  await loadHealth();
  await loadSessions();
})();
