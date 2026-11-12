const sessionListEl = document.getElementById("session-list");
const paneEl = document.getElementById("pane");
const graphWrap = document.getElementById("graph-wrap");
const graphCanvas = document.getElementById("orch-canvas");
const graphStatus = document.getElementById("graph-status");
const healthEl = document.getElementById("health");
const refreshBtn = document.getElementById("refresh");
const sendForm = document.getElementById("send-form");
const lineInput = document.getElementById("line");
const enterInput = document.getElementById("enter");
const sendTarget = document.getElementById("send-target");
const sendStatus = document.getElementById("send-status");
const viewTabs = document.querySelectorAll(".view-tab");

let selectedSession = null;
let selectedPendingAsk = false;
let lastGrouped = null;
let activeView = "pane";
let graphPollTimer = null;
let lastGraph = null;

async function api(path, options = {}) {
  // Shared secret for CDAMUX_TAILNET_TOKEN; set once via localStorage.setItem("cdamux_token", "...").
  const token = localStorage.getItem("cdamux_token");
  if (token) {
    options.headers = { ...(options.headers || {}), "X-Cdamux-Token": token };
  }
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

function sessionBadges(s) {
  const parts = [];
  if (s.unread > 0) parts.push(`✉${s.unread}`);
  if (s.pending_ask) parts.push("?");
  return parts.length
    ? `<span class="badges">${parts.map(escapeHtml).join(" ")}</span>`
    : "";
}

function sessionGlyph(s) {
  return s.role === "orchestrator" ? "◎" : "•";
}

/** Nest sessions under `@cdamux_parent` within one group (same as Fleet). */
function nestSessions(sessions) {
  const byName = new Map((sessions || []).map((s) => [s.name, s]));
  const children = new Map();
  const roots = [];
  for (const s of sessions || []) {
    const parent = s.parent && byName.has(s.parent) ? s.parent : null;
    if (parent) {
      if (!children.has(parent)) children.set(parent, []);
      children.get(parent).push(s);
    } else {
      roots.push(s);
    }
  }
  const out = [];
  const walk = (s, depth) => {
    out.push({ session: s, depth });
    for (const kid of children.get(s.name) || []) walk(kid, depth + 1);
  };
  for (const r of roots) walk(r, 0);
  return out;
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
    for (const { session: s, depth } of nestSessions(group.sessions || [])) {
      const li = document.createElement("li");
      li.style.paddingLeft = `${depth * 0.85}rem`;
      const btn = document.createElement("button");
      btn.type = "button";
      btn.dataset.name = s.name;
      btn.dataset.pendingAsk = s.pending_ask ? "1" : "0";
      btn.setAttribute(
        "aria-current",
        s.name === selectedSession ? "true" : "false"
      );
      const stateLabel = s.working_stale ? `${s.state} ⚠` : s.state;
      const nest = depth > 0 ? "└ " : "";
      btn.innerHTML = `<span class="name">${nest}${sessionGlyph(s)} ${escapeHtml(s.name)}</span>
        ${sessionBadges(s)}
        <div class="meta"><span class="${stateClass(s.state)}">${escapeHtml(stateLabel)}</span> · ${escapeHtml(s.path)}</div>`;
      btn.addEventListener("click", () => selectSession(s.name));
      li.appendChild(btn);
      sessionListEl.appendChild(li);
    }
  }
  if (!sessionListEl.children.length) {
    sessionListEl.innerHTML = "<li class=\"muted\">No tmux sessions.</li>";
  }
}

function findSession(name) {
  for (const g of lastGrouped?.groups || []) {
    for (const s of g.sessions || []) {
      if (s.name === name) return s;
    }
  }
  return null;
}

function escapeHtml(str) {
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function setActiveView(view) {
  activeView = view;
  document.querySelectorAll(".detail-view").forEach((el) => {
    const match = el.dataset.view === view;
    el.hidden = !match;
  });
  viewTabs.forEach((btn) => {
    const on = btn.dataset.view === view;
    btn.classList.toggle("active", on);
    btn.setAttribute("aria-selected", on ? "true" : "false");
  });
  if (view === "graph" && selectedSession) {
    loadOrchGraph();
    startGraphPoll();
  } else {
    stopGraphPoll();
  }
}

viewTabs.forEach((btn) => {
  btn.addEventListener("click", () => setActiveView(btn.dataset.view));
});

function stopGraphPoll() {
  if (graphPollTimer) {
    clearInterval(graphPollTimer);
    graphPollTimer = null;
  }
}

function startGraphPoll() {
  stopGraphPoll();
  graphPollTimer = setInterval(() => {
    if (activeView === "graph" && selectedSession) loadOrchGraph();
  }, 1000);
}

function nodeColor(status) {
  switch (status) {
    case "done":
      return "#34a853";
    case "failed":
    case "blocked":
      return "#ea4335";
    case "running":
      return "#7cacf8";
    default:
      return "#9aa0a6";
  }
}

function layoutGraph(graph) {
  const nodes = graph.nodes || [];
  const edges = graph.edges || [];
  const byId = new Map(nodes.map((n, i) => [n.id, { ...n, i }]));
  const levels = new Map();
  const roots = nodes.filter((n) => !n.parent && n.id === "integrator");
  const queue = roots.length ? roots.map((n) => n.id) : nodes.map((n) => n.id);
  queue.forEach((id, idx) => levels.set(id, 0));
  for (const e of edges) {
    if (!levels.has(e.to)) levels.set(e.to, (levels.get(e.from) || 0) + 1);
  }
  const positions = new Map();
  const perLevel = new Map();
  nodes.forEach((n) => {
    const lv = levels.get(n.id) ?? 1;
    perLevel.set(lv, (perLevel.get(lv) || 0) + 1);
    const idx = perLevel.get(lv) - 1;
    positions.set(n.id, { x: 80 + lv * 140, y: 60 + idx * 70 });
  });
  return { positions, byId, edges };
}

function drawOrchGraph(graph) {
  const ctx = graphCanvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1;
  const rect = graphCanvas.getBoundingClientRect();
  graphCanvas.width = Math.max(640, rect.width) * dpr;
  graphCanvas.height = 360 * dpr;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, graphCanvas.width, graphCanvas.height);

  const { positions, byId, edges } = layoutGraph(graph);
  ctx.strokeStyle = "#5f6368";
  ctx.lineWidth = 1;
  for (const e of edges) {
    const a = positions.get(e.from);
    const b = positions.get(e.to);
    if (!a || !b) continue;
    ctx.beginPath();
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
    ctx.stroke();
  }
  for (const n of graph.nodes || []) {
    const p = positions.get(n.id);
    if (!p) continue;
    ctx.fillStyle = nodeColor(n.status);
    ctx.beginPath();
    ctx.arc(p.x, p.y, 14, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "#e8eaed";
    ctx.font = "11px system-ui";
    ctx.fillText(n.id.slice(0, 18), p.x - 40, p.y + 28);
    ctx.fillStyle = "#9aa0a6";
    ctx.font = "10px system-ui";
    ctx.fillText(n.phase, p.x - 30, p.y + 40);
  }
}

async function loadOrchGraph() {
  if (!selectedSession) return;
  graphStatus.textContent = "Loading graph…";
  try {
    const graph = await api(
      `/v1/sessions/${encodeURIComponent(selectedSession)}/orch-graph`
    );
    lastGraph = graph;
    graphStatus.textContent = `run ${graph.run_id} · updated ${graph.updated_at}`;
    drawOrchGraph(graph);
  } catch (e) {
    graphStatus.textContent = e.message;
    const ctx = graphCanvas.getContext("2d");
    ctx.clearRect(0, 0, graphCanvas.width, graphCanvas.height);
  }
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
    lastGrouped = grouped;
    renderSessions(grouped);
  } catch (e) {
    sessionListEl.innerHTML = `<li class="muted">${escapeHtml(e.message)}</li>`;
  }
}

async function selectSession(name) {
  selectedSession = name;
  sendForm.hidden = false;
  sendStatus.textContent = "";
  await loadSessions();
  const s = findSession(name);
  selectedPendingAsk = !!(s && s.pending_ask);
  sendTarget.textContent = selectedPendingAsk ? `${name} (answer)` : name;
  if (activeView === "pane") {
    paneEl.textContent = "Loading pane…";
    try {
      const pane = await api(`/v1/sessions/${encodeURIComponent(name)}/pane`);
      paneEl.textContent = pane.text || "(empty)";
    } catch (e) {
      paneEl.textContent = e.message;
    }
  } else {
    await loadOrchGraph();
    startGraphPoll();
  }
}

sendForm.addEventListener("submit", async (ev) => {
  ev.preventDefault();
  if (!selectedSession) return;
  const asAnswer = selectedPendingAsk;
  sendStatus.textContent = asAnswer ? "Answering…" : "Sending…";
  try {
    const body = {
      line: lineInput.value,
      enter: enterInput.checked,
    };
    if (asAnswer) body.kind = "answer";
    await api(`/v1/sessions/${encodeURIComponent(selectedSession)}/send`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    sendStatus.textContent = asAnswer ? "Answered." : "Sent.";
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

window.addEventListener("resize", () => {
  if (lastGraph) drawOrchGraph(lastGraph);
});

(async () => {
  await loadHealth();
  await loadSessions();
})();
