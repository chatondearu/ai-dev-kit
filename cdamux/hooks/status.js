#!/usr/bin/env node
// cdamux — status adapter for Claude / Cursor / Codex hooks (ADR-011 §3 + §4, ADR-018).
// Maps lifecycle events → `cdamux state append agents <id> {"status":…,"turn":…}`.
// `turn` (`running` from prompt submit / permission request, `ended` at turn end) is never
// sticky: `cdamux msg` types its wake line only into a pane whose turn ended.
// Message bodies never reach a pane: a Claude turn end with unread messages blocks the
// stop and hands them over as the reason; a prompt submit injects them; both ack them.
// Other clients on these events (codex) keep the unacked digest: `msg read --ack` is theirs.
// Only the pane's top-level CLI has effects: a nested `claude -p` (tool call) inherits the
// agent id and must not take the parent's inbox nor flip its status / turn.
// Turn end of an orchestrator also records its reply in the events feed (docs/events.md),
// including a tool-only stop that has no assistant prose,
// and pastes the next queued `cdamux say` prompt (docs/say.md).
// Fail-open, local, fast; no-op without CDAMUX_AGENT_ID / @cdamux_id or without
// `cdamux` on PATH. Stickiness: done/error/blocked stay until the next `working`.
//
// Cursor: sessionStart/sessionEnd → idle; beforeSubmitPrompt → working when fired
// (cannot inject context — see docs); stop → idle + followup_message digest.
// Print-mode (`cursor-agent -p`) may omit beforeSubmitPrompt — rely on sessionStart/End.
// Cursor CLI runs queued messages (stop followup_message, text typed mid-turn) without
// beforeSubmitPrompt, so a stop that queues a followup stays `working` and postToolUse
// lifts `idle` back to `working` (never a sticky status) and `turn` to `running`.
const fs = require('fs');
const {
  setAgentState,
  readAgentStatus,
  readAgentField,
  recordReply,
  drainSay,
  resolveAgentId,
  unreadDigest,
  takeUnread,
  formatMessages,
  isTopLevelHook,
  readUnreadMessages,
  takeStopBlock,
} = require('./ledger');

const MAP = {
  UserPromptSubmit: 'working',
  beforeSubmitPrompt: 'working',
  Stop: 'idle',
  stop: 'idle',
  SessionStart: 'idle',
  sessionStart: 'idle',
  SessionEnd: 'idle',
  sessionEnd: 'idle',
  PermissionRequest: 'needs_input',
};

// Synchronous: printed before any slower step, so a hook killed later still hands it over.
function writeJson(obj) {
  fs.writeSync(1, JSON.stringify(obj) + '\n');
}

// What each client expects from a hook that does nothing.
function writeNoop(event) {
  if (event === 'beforeSubmitPrompt') writeJson({ continue: true });
  else if (/^(stop|sessionStart|sessionEnd|postToolUse|postToolUseFailure)$/.test(event)) writeJson({});
}

// `working` only from idle / unset: sticky done/error/blocked (open ask) must survive.
// `turn` is not sticky; written only when it changes (postToolUse fires per tool).
function liftIdle() {
  const id = resolveAgentId(process.env);
  const cur = readAgentStatus(process.env, id);
  const status = !cur || cur === 'idle' ? 'working' : null;
  const turn = readAgentField(process.env, id, 'turn') === 'running' ? null : 'running';
  setAgentState(process.env, status, turn);
}

function main() {
  const input = JSON.parse(fs.readFileSync(0, 'utf8'));
  const event = input.hook_event_name || input.event || '';
  if (!isTopLevelHook(process.env)) {
    writeNoop(event);
    return;
  }
  let status = MAP[event];
  if (event === 'postToolUse' || event === 'postToolUseFailure') {
    liftIdle();
    writeJson({});
    return;
  }
  // Claude also fires Notification as `idle_prompt` ~60 s after a turn ends; only a
  // permission prompt means needs_input (older payloads have no type: match the text).
  if (event === 'Notification') {
    const type = input.notification_type;
    const perm = type ? type === 'permission_prompt' : /permission/i.test(input.message || '');
    if (perm) status = 'needs_input';
  }
  const id = resolveAgentId(process.env);
  const claude = readAgentField(process.env, id, 'client') === 'claude';
  // Claude turn end with unread messages: keep the turn going and hand them over (Q21).
  // Taken (acked) before `turn: ended` is written, so nothing typed lands mid-turn; a line
  // appended after this read is left to the `msg wake` backstop. At most MAX_STOP_BLOCKS
  // in a row; then the turn ends and the rest waits for the wake line.
  if (
    event === 'Stop'
    && claude
    && readUnreadMessages(process.env, id).length
    && takeStopBlock(process.env, !!input.stop_hook_active)
  ) {
    const msgs = takeUnread(process.env);
    if (msgs.length) {
      writeJson({ decision: 'block', reason: formatMessages(msgs) });
      recordReply(process.env, input);
      return;
    }
  }
  const digest = event === 'stop' || !claude ? unreadDigest(process.env) : '';
  if (event === 'stop' && digest && (Number(input.loop_count) || 0) === 0) {
    liftIdle();
    status = null;
  }
  let turn = null;
  if (status === 'working' || status === 'needs_input') turn = 'running';
  if (status === 'idle' && /^(Stop|stop)$/.test(event)) turn = 'ended';
  // The CLI exited; its pane may be a shell now: never type into it.
  if (/^(SessionEnd|sessionEnd)$/.test(event)) turn = 'exited';
  setAgentState(process.env, status, turn);
  if (event === 'Stop' || event === 'stop') recordReply(process.env, input);
  // Not when a Cursor stop queued a followup (status null): that turn is still running.
  if (status === 'idle' && (event === 'Stop' || event === 'stop')) drainSay(process.env);

  if (event === 'beforeSubmitPrompt') {
    // Cursor cannot inject additional_context here (continue / user_message only).
    writeJson({ continue: true });
    return;
  }
  if (event === 'UserPromptSubmit' || (event === 'Stop' && !claude && !input.stop_hook_active)) {
    const msgs = claude ? takeUnread(process.env) : [];
    const context = msgs.length ? formatMessages(msgs) : digest;
    if (context) {
      writeJson({ hookSpecificOutput: { hookEventName: event, additionalContext: context } });
    }
    return;
  }
  if (event === 'Stop') return;
  if (event === 'stop') {
    // Cursor stop: followup_message continues the turn. Cap at first auto-loop.
    const loop = Number(input.loop_count) || 0;
    if (digest && loop === 0) {
      writeJson({ followup_message: digest });
    } else {
      writeJson({});
    }
    return;
  }
  if (event === 'sessionStart' || event === 'sessionEnd') {
    writeJson({});
  }
}

try { main(); } catch (e) { /* fail-open */ }
process.exit(0);
