// Health check: plain-language warnings about the codebase (secrets, endpoints anyone can call,
// unused files, oversized files), written for people who built an app with AI and don't know its
// insides. Precision beats recall: a false alarm costs this audience more than a missed one.
//
// findIssues(src, file)  — called by scan.mjs for every code/config/docs file with its source.
//                           Returns raw findings for that file, stored in scan.json files[].issues.
//                           Findings never contain a secret's value, only its line, kind and a masked hint.
// buildHealth(ctx)       — called by build.mjs with { graph, scan, arch, root } after the graph is built.
//                           Returns { summary: { high, medium, low }, issues: [...] }, stored as graph.health.

import fs from 'node:fs';
import path from 'node:path';
import { git } from './common.mjs';

const lineIndex = (src) => {
  const starts = [0];
  for (let i = 0; i < src.length; i++) if (src.charCodeAt(i) === 10) starts.push(i + 1);
  return (offset) => {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid] <= offset) lo = mid; else hi = mid - 1; }
    return lo + 1;
  };
};

/** Shannon entropy in bits per character: random keys score ~4+, words and placeholders ~2-3. */
function entropy(s) {
  const counts = new Map();
  for (const c of s) counts.set(c, (counts.get(c) || 0) + 1);
  let e = 0;
  for (const n of counts.values()) { const p = n / s.length; e -= p * Math.log2(p); }
  return e;
}

// Files where keys are usually fake: tests, fixtures, examples, docs.
const TEST_FILE = /(^|\/)(tests?|__tests__|__mocks__|spec|specs|e2e|fixtures?|testdata|mocks?)\/|\.(test|spec|stories)\.[a-z]+$|(^|\/)test_[^/]*\.py$|_test\.(py|go)$|(^|\/)conftest\.py$/i;
const EXAMPLE_FILE = /(^|\/)(examples?|samples?|docs?|demo)\/|\.(example|sample|template|dist|tmpl)(\.|$)|(^|\/)[^/]*example[^/]*$/i;
const PLACEHOLDER = /your|xxxx|example|changeme|change[-_]?me|placeholder|replace|dummy|fake|sample|redacted|insert|<[^>]*>|\$\{|\{\{|%\(|\*\*\*|0{8,}|1234567|abcdef|_here\b|-here\b/i;

// ---------- 1. secrets ----------

// Well-known key formats. `sev` defaults to high; `live` formats are flagged even in test files.
const FORMATS = [
  { re: /\bAKIA[0-9A-Z]{16}\b/g, what: 'an Amazon Web Services (AWS) access key', svc: 'AWS', live: true },
  { re: /\b(?:sk|rk)_live_[0-9a-zA-Z]{20,}/g, what: 'a Stripe live secret key', svc: 'Stripe', live: true },
  { re: /\b(?:sk|rk)_test_[0-9a-zA-Z]{20,}/g, what: 'a Stripe test key', svc: 'Stripe', sev: 'medium' },
  { re: /\bwhsec_[A-Za-z0-9]{24,}/g, what: 'a Stripe webhook signing secret', svc: 'Stripe' },
  { re: /\bgh[pousr]_[A-Za-z0-9]{36,}\b/g, what: 'a GitHub access token', svc: 'GitHub', live: true },
  { re: /\bgithub_pat_[A-Za-z0-9_]{50,}/g, what: 'a GitHub access token', svc: 'GitHub', live: true },
  { re: /\bglpat-[A-Za-z0-9_-]{20,}/g, what: 'a GitLab access token', svc: 'GitLab', live: true },
  { re: /\bxox[abposr]-[A-Za-z0-9-]{10,}/g, what: 'a Slack token', svc: 'Slack', live: true },
  { re: /https:\/\/hooks\.slack\.com\/services\/T[A-Z0-9]+\/B[A-Z0-9]+\/[A-Za-z0-9]{20,}/g, what: 'a Slack webhook address (anyone with it can post to your Slack)', svc: 'Slack', sev: 'medium' },
  { re: /\bAIza[0-9A-Za-z_\-]{35}\b/g, what: 'a Google API key', svc: 'Google Cloud', sev: 'medium', skipIf: /firebase|authDomain|messagingSenderId/i },
  { re: /\bGOCSPX-[A-Za-z0-9_\-]{20,}/g, what: 'a Google sign-in client secret', svc: 'Google Cloud' },
  { re: /\bsk-ant-[a-z]+\d*-[A-Za-z0-9_\-]{40,}/g, what: 'an Anthropic (Claude) API key', svc: 'Anthropic', live: true },
  { re: /\bsk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_\-]{16,}T3BlbkFJ[A-Za-z0-9_\-]{16,}/g, what: 'an OpenAI API key', svc: 'OpenAI', live: true },
  { re: /\bsk-(?:proj|svcacct|admin)-[A-Za-z0-9_\-]{40,}/g, what: 'an OpenAI API key', svc: 'OpenAI', live: true },
  { re: /-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?-----/g, what: 'a private key', svc: null, whole: true },
  { re: /\bSG\.[A-Za-z0-9_\-]{22}\.[A-Za-z0-9_\-]{43}\b/g, what: 'a SendGrid email API key', svc: 'SendGrid', live: true },
  { re: /\bsq0csp-[0-9A-Za-z_\-]{40,}/g, what: 'a Square app secret', svc: 'Square', live: true },
  { re: /\bsq0atp-[0-9A-Za-z_\-]{20,}/g, what: 'a Square access token', svc: 'Square', live: true },
  { re: /\bEAAA[A-Za-z0-9_\-]{60,}/g, what: 'a Square access token', svc: 'Square', live: true },
  { re: /\bshp(?:at|ss|ca|pa)_[a-fA-F0-9]{32}\b/g, what: 'a Shopify access token', svc: 'Shopify', live: true },
  { re: /\bhf_[A-Za-z0-9]{30,}\b/g, what: 'a Hugging Face token', svc: 'Hugging Face', live: true },
  { re: /\bnpm_[A-Za-z0-9]{36}\b/g, what: 'an npm publishing token', svc: 'npm', live: true },
  { re: /\bsbp_[a-f0-9]{40}\b/g, what: 'a Supabase account token', svc: 'Supabase', live: true },
  { re: /\bsb_secret_[A-Za-z0-9_\-]{20,}/g, what: 'a Supabase secret key', svc: 'Supabase', live: true },
  { re: /\b\d{8,10}:AA[0-9A-Za-z_\-]{33}\b/g, what: 'a Telegram bot token', svc: 'Telegram', live: true },
  { re: /\bdop_v1_[a-f0-9]{64}\b/g, what: 'a DigitalOcean token', svc: 'DigitalOcean', live: true },
  { re: /\bpat[A-Za-z0-9]{14}\.[a-f0-9]{64}\b/g, what: 'an Airtable token', svc: 'Airtable', live: true },
];
const JWT = /\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{20,}/g;
const DB_URL = /\b(?:postgres(?:ql)?|mysql|mariadb|mongodb(?:\+srv)?|rediss?|amqps?)(?:\+\w+)?:\/\/([^:\s\/'"@]+):([^@\s'"]+)@([^\/\s'":?,]+)/g;
const LOCAL_HOST = /^(localhost|127\.0\.0\.1|0\.0\.0\.0|db|postgres|mysql|redis|mongo|database|host\.docker\.internal)$/i;

// Names that hold secrets (after camelCase → snake_case). Public-by-design keys are excluded below.
const SECRET_NAME = /(?:^|[_.-])(api_?key|apikey|secret(?:_?key)?|client_?secret|app_?secret|token|access_?token|refresh_?token|auth_?token|authtoken|bearer_?token|passw(?:or)?d|passwd|pwd|private_?key|secret_?access_?key|signing_?(?:key|secret)|signature_?key|webhook_?secret|encryption_?key|auth(?:orization)?_?key|master_?key|credentials?)$/i;
const PUBLIC_NAME = /(^|_)(public|publishable|anon|site_?key|example|sample|test|dummy|fake|mock|placeholder|default|field|label|header|name|type|url|uri|endpoint|path|file|env|var|prefix|length|len|min|max|regex|pattern|format|expires?|ttl)(_|$)|^(next_public|vite|react_app|expo_public|nuxt_public|gatsby)_/i;
const ENV_LOOKUP = /os\.environ|os\.getenv|getenv\(|process\.env|import\.meta\.env|ENV\[|ENV\.fetch|System\.getenv|Deno\.env|secrets\.|config\(\s*['"]|env\(\s*['"]|\$\{\{/;
const PASSWORD_NAME = /pass|pwd/i;
const CONFIG_LANGS = new Set(['yaml', 'yml', 'ini', 'cfg', 'conf', 'properties', 'env', 'toml']);

const snake = (s) => s.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
const kindFromName = (name) => {
  const n = snake(name);
  if (/pass|pwd/.test(n)) return 'a password';
  if (/private_?key/.test(n)) return 'a private key';
  if (/token/.test(n)) return 'an access token';
  if (/api_?key|apikey/.test(n)) return 'an API key';
  if (/credential/.test(n)) return 'login credentials';
  return 'a secret key';
};
const mask = (v, password) => password || v.length < 12 ? `${v.length} characters` : `starts “${v.slice(0, 4)}…”, ${v.length} characters`;

function valueLooksReal(v, name) {
  if (/\s/.test(v) || PLACEHOLDER.test(v)) return false;
  if (/^(none|null|nil|undefined|true|false|string|number|text|required|optional|bearer|basic|secret|token|password|test|dev|development|local|admin)$/i.test(v)) return false;
  if (/^[A-Z][A-Z0-9_]+$/.test(v)) return false;                 // the NAME of an env var
  if (/^[a-z_]+(\.[a-z_]+)+$/i.test(v)) return false;            // dotted identifier / key path
  if (/^(\.{0,2}\/|~\/|https?:\/\/|[a-z]:\\)/i.test(v)) return false; // paths and URLs
  if (/\.(json|pem|key|txt|ya?ml|env|crt|p12)$/i.test(v)) return false; // a file name, not the key
  if (/^x-[\w-]+$/i.test(v) || /^(application|text)\//.test(v)) return false;
  if (/^(.)\1+$/.test(v)) return false;
  if (/^(pass(word)?|secret|admin|test|user|changeit|letmein|qwerty|demo|guest|root|welcome)[\d!@#$._-]*$/i.test(v)) return false;
  if (PASSWORD_NAME.test(name)) return v.length >= 6 && !/^[a-z]+$/.test(v);
  return v.length >= 16 && entropy(v) >= 3.2 && /\d/.test(v) && /[a-zA-Z]/.test(v);
}

/** 1-based line numbers inside docstrings or block comments (examples there aren't real config). */
function docLines(src, lang) {
  const set = new Set();
  if (!['python', 'typescript', 'javascript', 'java', 'kotlin', 'go', 'csharp', 'php', 'rust', 'swift'].includes(lang)) return set;
  const re = lang === 'python' ? /("""|''')[\s\S]*?\1/g : /\/\*[\s\S]*?\*\//g;
  const at = lineIndex(src);
  for (const m of src.matchAll(re)) { const a = at(m.index), b = at(m.index + m[0].length); for (let l = a; l <= b; l++) set.add(l); }
  return set;
}

function findSecrets(src, f, out) {
  const at = lineIndex(src);
  const isTest = TEST_FILE.test(f.path) || EXAMPLE_FILE.test(f.path);
  const seenLines = new Set();
  const add = (line, rec) => { if (seenLines.has(line)) return; seenLines.add(line); out.push({ k: 'secret', line, ...rec }); };

  for (const fm of FORMATS) {
    if (fm.skipIf?.test(src)) continue;
    for (const m of src.matchAll(fm.re)) {
      const v = m[0];
      if (isTest && (!fm.live || fm.whole)) continue;
      if (!fm.whole && (/EXAMPLE|xxxx|XXXX|0{10,}/.test(v) || entropy(v) < 3)) continue;
      add(at(m.index), { what: fm.what, svc: fm.svc, sev: fm.sev || 'high', hint: fm.whole ? 'private key block' : mask(v) });
    }
  }
  if (!isTest) {
    for (const m of src.matchAll(JWT)) {
      let payload = null;
      try { payload = JSON.parse(Buffer.from(m[0].split('.')[1].replace(/-/g, '+').replace(/_/g, '/'), 'base64').toString('utf8')); } catch { continue; }
      if (payload?.role !== 'service_role') continue; // "anon" keys are public by design
      add(at(m.index), { what: 'a Supabase service-role key (full access to your database, skipping all security rules)', svc: 'Supabase', sev: 'high', hint: mask(m[0]) });
    }
    for (const m of src.matchAll(DB_URL)) {
      const [, user, pw, host] = m;
      if (LOCAL_HOST.test(host) || PLACEHOLDER.test(pw) || /^[:$%{<]/.test(pw) || pw.length < 4 || /^(password|pass|postgres|root|secret|admin|user)$/i.test(pw)) continue;
      add(at(m.index), { what: 'a database password inside a connection address', svc: 'your database', sev: 'high', hint: mask(pw, true) });
    }
  }
  if (isTest || f.category === 'docs') return;

  // Assignments: api_key = "…", "password": "…", PASSWORD=… (config files may leave values unquoted).
  const lines = src.split('\n');
  const inDoc = docLines(src, f.lang);
  // CI / docker-compose database containers get throwaway passwords
  const throwawayDb = /(^|\/)\.github\/|(^|\/)(docker-)?compose[\w.-]*\.ya?ml$/i.test(f.path);
  const quoted = /(["']?)([A-Za-z_][\w.-]*)\1\s*(?::=|=>|=|:)\s*(?:[rbuRBU]{1,2})?(["'`])([^"'`\n]{6,200})\3/g;
  const unquoted = /^\s*(?:export\s+)?([A-Za-z_][\w.-]*)\s*[:=]\s*([^\s#'"]{6,200})\s*(?:#.*)?$/;
  lines.forEach((text, i) => {
    if (text.length > 400 || ENV_LOOKUP.test(text) || inDoc.has(i + 1) || /^\s*(#|\/\/|\*|\/\*|<!--)/.test(text)) return;
    const pairs = [...text.matchAll(quoted)].map((m) => [m[2], m[4]]);
    if (!pairs.length && (f.category === 'config' && CONFIG_LANGS.has(f.lang))) { const u = unquoted.exec(text); if (u) pairs.push([u[1], u[2]]); }
    for (const [name, value] of pairs) {
      const n = snake(name.split('.').pop());
      if (!SECRET_NAME.test(n) || PUBLIC_NAME.test(n) || !valueLooksReal(value, n)) continue;
      if (throwawayDb && /^(postgres|mysql|mariadb|mongo(_initdb_root)?|redis|pg|rabbitmq_default|db|database)_(root_)?(password|pass|pwd)$/.test(n)) continue;
      add(i + 1, { what: kindFromName(name), name: name.split('.').pop(), sev: 'high', hint: mask(value, PASSWORD_NAME.test(n)), generic: true });
    }
  });
}

// ---------- 2. who may call each endpoint ----------

// Decorators / middleware / dependencies whose names say "check who is calling".
const GUARD_NAME = /(^|_)(login|auth|authenticated|authorized?|jwt|token|permission|permissions|perm|role|roles|staff|admin|superuser|owner|scope|scopes|session|api_?key|signature|signed|hmac|device|member|user)(_|$)|_required$|^requires?_|^ensure_|^verify_|^check_(auth|token|user|session|permission|role|api)|protect|^is_?authenticated|^is_?admin/i;
const NOT_GUARD = /^(route|get|post|put|patch|delete|cached?|limit|limiter|rate_?limit\w*|cross_origin|wraps|staticmethod|classmethod|property|retry|timed|log\w*|swag_from|validate\w*|expects|marshal\w*|doc|json_required|fields?_required|params?_required|body_required|transaction\w*|csrf\w*|exempt)$/i;
const isGuardName = (name) => !!name && !NOT_GUARD.test(name) && GUARD_NAME.test(snake(name));
// FastAPI dependencies that hand over a database session rather than check the caller
// (SessionDep, get_db, get_sql_session, SessionLocal, a unit of work), matched on the snake_case name
const DB_DEPENDENCY = /^(get_|new_|create_)?(async_)?(db|database|session|db_session|sessionmaker)$|(^|_)(db|database|sql\w*|engine|conn|connection|cursor|uow|unit_of_work)(_|$)|session_?(local|maker|factory)/i;
const isDbDep = (name) => DB_DEPENDENCY.test(snake(name));
const isGuardDep = (name) => isGuardName(name) && !isDbDep(name);
// A handler body that inspects the caller itself.
const INLINE_CHECK = /request\.headers(?:\.get)?\(?\s*\[?\s*['"](authorization|x-api-key|x-[\w-]*(token|key|secret|signature)[\w-]*)['"]|req\.headers(?:\.|\[\s*['"])(authorization|x-api-key)|\b(verify|check|require|validate|authenticate|authorize)_?\w*(auth|token|session|user|key|signature|webhook|hmac|jwt|device|staff|admin|permission|role|request|caller)\w*\s*\(|\bcurrent_user\b|get_jwt_identity|verify_jwt_in_request|compare_digest|abort\(\s*40[13]\b|,\s*40[13]\s*\)?\s*$|status(?:Code)?\s*[:=(]\s*40[13]\b|\.status\(\s*40[13]\s*\)|req\.user\b|req\.session\.user|req\.auth\b|jwt\.verify|jwtVerify|getServerSession|auth\(\)|currentUser\(|getAuth\(|auth\.getUser|getSession\(|verifyIdToken|getToken\(/m;
const FIREBASE_CHECK = /\b\w+\.auth\b(?!\s*\()|HttpsError\(\s*["'](unauthenticated|permission-denied)["']|enforceAppCheck\s*:\s*true|\b\w+\.app\b\s*(==|!=|===|!==)\s*(undefined|null)|!\s*\w+\.app\b/;
const SIGNATURE_CHECK =/hmac|signature|verify_?webhook|construct_?event|x-hub-signature|compare_digest|svix|webhooks?\.verify|jwt\.decode|oidc|id_token|authenticated|verify_\w*(token|request|push)/i;
const NEXT_GUARD = /getServerSession|\bauth\(\)|currentUser\(|getAuth\(|auth\.getUser|supabase\.auth\.get(User|Session)|getSession\(|verifyIdToken|jwt\.verify|jwtVerify|getToken\(|clerkClient|requireAuth|requireUser|withAuth|validateRequest|headers\(\)\.get\(\s*['"]authorization|status:\s*40[13]\b/;
const NEXT_MIDDLEWARE_GUARD = /getToken|withAuth|clerkMiddleware|authMiddleware|auth\(|supabase|getSession|jwtVerify|jwt\.verify|cookies\.get\([^)]*(session|token|auth)|redirect\([^)]*login/i;

function blockAfter(lines, defIdx) {
  // A Python def's body: every line after it that's more indented, up to 120 lines.
  const ind = /^\s*/.exec(lines[defIdx])[0].length;
  let end = defIdx + 1;
  while (end < lines.length && end < defIdx + 120 && (!lines[end].trim() || /^\s*/.exec(lines[end])[0].length > ind || /^\s*[)\]]/.test(lines[end]))) end++;
  return lines.slice(defIdx, end).join('\n');
}

function pyRoutes(src, f, out) {
  const lines = src.split('\n');
  // Blueprint / app-wide before_request hooks and router-level dependencies that check the caller.
  const guardedVars = new Map();
  for (const m of src.matchAll(/^@(\w+)\.before_(?:app_)?request\s*\n(?:@.*\n)*\s*def\s+(\w+)/gm)) {
    const defIdx = src.slice(0, m.index + m[0].length).split('\n').length - 1;
    const body = blockAfter(lines, defIdx);
    if (!/\b40[13]\b|Unauthorized|Forbidden|verify_jwt_in_request|login_required|is_authenticated|current_user/.test(body)) continue;
    guardedVars.set(m[1], m[2]);
    if (m[1] === 'app') out.push({ k: 'global-guard', framework: 'flask', name: m[2], line: defIdx + 1 });
  }
  for (const m of src.matchAll(/^\s*(\w+)\s*=\s*(?:\w+\.)?APIRouter\(([^)]*(?:\([^)]*\)[^)]*)*)\)/gm)) {
    const dep = /dependencies\s*=\s*\[[^\]]*Depends\(\s*([\w.]+)/.exec(m[2]);
    if (dep && isGuardDep(dep[1].split('.').pop())) guardedVars.set(m[1], dep[1]);
  }
  // FastAPI dependency aliases, `AdminUser = Annotated[Session, Depends(admin_user)]`, and the dependency
  // functions behind them. They're usually imported from a shared deps.py, so build decides repo-wide
  // which ones check the caller (see pyAliasGuards).
  const lineAt = (i) => src.slice(0, i).split('\n').length - 1;
  const sigAt = (idx) => lines.slice(idx, idx + 15).join('\n').split(/\)\s*(->[^:]*)?:\s*$/m)[0];
  const depsIn = (text) => [...text.matchAll(/\b(?:Depends|Security)\(\s*([\w.]+)/g)].map((m) => m[1].split('.').pop());
  const typesIn = (sig) => [...new Set([...sig.matchAll(/\b\w+\s*:\s*(?:[\w.]*Annotated\[\s*)?([A-Za-z_][\w.]*)/g)].map((m) => m[1].split('.').pop()))];
  if (/\b(Depends|Security)\(/.test(src)) {
    for (const m of src.matchAll(/^(?:type\s+)?(\w+)\s*(?::[^=\n]*)?=\s*(?:[\w.]+\.)?Annotated\[/gm)) {
      let depth = 1, j = m.index + m[0].length;
      while (j < src.length && depth && j - m.index < 800) { if (src[j] === '[') depth++; else if (src[j] === ']') depth--; j++; }
      const fn = depsIn(src.slice(m.index, j))[0];
      if (fn) out.push({ k: 'py-alias', name: m[1], fn, line: lineAt(m.index) + 1 });
    }
    // top-level functions that aren't routes, with the dependencies and types they take
    for (const m of src.matchAll(/^(?:async\s+)?def\s+(\w+)\s*\(/gm)) {
      const idx = lineAt(m.index);
      if (idx > 0 && /^\s*@/.test(lines[idx - 1])) continue;
      const sig = sigAt(idx), deps = depsIn(sig), types = typesIn(sig);
      if (deps.length || types.length) out.push({ k: 'py-dep-fn', name: m[1], ...(deps.length ? { deps } : {}), ...(types.length ? { types } : {}) });
    }
  }
  // Decorators defined here: auth-named ones, and project wrappers like `@endpoint(write=True)` whose
  // body applies an auth decorator itself. Repeat once so a wrapper of a wrapper counts too.
  const localGuards = new Set();
  const defs = [...src.matchAll(/^def\s+(\w+)\s*\(/gm)].map((m) => { const idx = src.slice(0, m.index).split('\n').length - 1; return { name: m[1], line: idx + 1, body: blockAfter(lines, idx) }; });
  const decoratorish = (body) => /wraps|decorat|def\s+\w+\(\s*\*args|return\s+\w+\s*$/m.test(body);
  for (let pass = 0; pass < 2; pass++) for (const d of defs) {
    if (localGuards.has(d.name) || !decoratorish(d.body)) continue;
    const applied = [...d.body.matchAll(/^\s*@([\w.]+)/gm)].map((m) => m[1].split('.').pop());
    const wraps = applied.some((a) => isGuardName(a) || localGuards.has(a));
    if (isGuardName(d.name) || wraps) { localGuards.add(d.name); out.push({ k: 'guard-def', name: d.name, line: d.line, ...(wraps && !isGuardName(d.name) ? { wrapper: true } : {}) }); }
  }

  for (const r of f.routes || []) {
    if (r.framework === 'django') continue;
    const i0 = r.line - 1;
    // decorators above the route decorator (wrong place in Flask/FastAPI: the route registers the unwrapped function)
    const above = [];
    for (let i = i0 - 1; i >= 0 && /^\s*@/.test(lines[i]); i--) above.push(/^\s*@([\w.]+)/.exec(lines[i])[1].split('.').pop());
    // decorators below it, up to the def
    const below = [];
    let defIdx = -1;
    for (let i = i0 + 1; i < lines.length && i < i0 + 30; i++) {
      if (/^\s*(async\s+)?def\s/.test(lines[i])) { defIdx = i; break; }
      const d = /^\s*@([\w.]+)/.exec(lines[i]);
      if (d) below.push(d[1].split('.').pop());
    }
    const decoText = lines.slice(i0, defIdx > 0 ? defIdx : i0 + 1).join('\n');
    const sig = defIdx > 0 ? lines.slice(defIdx, defIdx + 15).join('\n').split(/\)\s*(->[^:]*)?:\s*$/m)[0] : '';
    const body = defIdx > 0 ? blockAfter(lines, defIdx) : '';
    const rec = { k: 'route', line: r.line, method: r.method };
    const deco = below.find((d) => (isGuardName(d) || localGuards.has(d)) && !/^(route|get|post|put|patch|delete)$/.test(d));
    const dep = /Depends\(\s*([\w.]+)|Security\(\s*([\w.]+)/g;
    const deps = [...(decoText + '\n' + sig).matchAll(dep)].map((m) => (m[1] || m[2]).split('.').pop());
    if (deco) Object.assign(rec, { guard: deco, via: 'decorator' });
    else if (guardedVars.has(r.var)) Object.assign(rec, { guard: guardedVars.get(r.var), via: 'blueprint' });
    else if (deps.some(isGuardDep)) Object.assign(rec, { guard: deps.find(isGuardDep), via: 'dependency' });
    else if (INLINE_CHECK.test(body)) Object.assign(rec, { via: 'inline' });
    else {
      const misplaced = above.find((d) => isGuardName(d) && !/^(route|get|post|put|patch|delete)$/.test(d));
      if (misplaced) rec.misplaced = misplaced;
      // other decorators may be auth wrappers defined in another file; build resolves them
      const other = below.filter((d) => !/^(route|get|post|put|patch|delete|api_route|websocket)$/.test(d));
      if (other.length) rec.decos = other;
      // so may parameter types: `user: AdminUser`, a dependency alias (pyAliasGuards)
      const types = typesIn(sig);
      if (types.length) rec.types = types;
    }
    if (SIGNATURE_CHECK.test(body)) rec.signed = true;
    if (defIdx > 0) rec.fn = /def\s+(\w+)/.exec(lines[defIdx])[1];
    out.push(rec);
  }
}

function jsRoutes(src, f, out) {
  const routes = f.routes || [];
  if (!routes.length && !/\.use\(/.test(src) && !/(^|\/)middleware\.(t|j)s$/.test(f.path)) return;
  const lines = src.split('\n');
  // Next.js middleware.ts protecting the app
  if (/(^|\/)(src\/)?middleware\.(t|j)s$/.test(f.path) && NEXT_MIDDLEWARE_GUARD.test(src)) {
    const matcher = /matcher\s*:\s*([^\n]+)/.exec(src)?.[1] || '';
    if (!matcher || /api|\(\?!/.test(matcher)) out.push({ k: 'global-guard', framework: 'next', line: 1 });
  }
  // app.use(auth) / router.use(requireAuth) without a path guards everything registered after it
  const uses = [];
  for (const m of src.matchAll(/\b(\w+)\.use\(\s*([^'"`)][^)]*)\)/g)) {
    const names = m[2].match(/[A-Za-z_$][\w$]*/g) || [];
    if (names.some(isGuardName)) uses.push({ v: m[1], line: src.slice(0, m.index).split('\n').length, name: names.find(isGuardName) });
  }
  for (const u of uses) if (u.v === 'app' || u.v === 'server') out.push({ k: 'global-guard', framework: 'express', name: u.name, line: u.line });

  routes.forEach((r, idx) => {
    const rec = { k: 'route', line: r.line, method: r.method };
    if (r.framework === 'firebase') {
      // a callable checks request.auth (v1: context.auth) or turns on App Check; an onRequest function
      // checks a token or signature itself, like any other handler
      const next = routes.slice(idx + 1).find((x) => x.line > r.line)?.line || r.line + 80;
      const body = lines.slice(r.line - 1, Math.min(next - 1, r.line + 80)).join('\n');
      if (FIREBASE_CHECK.test(body) || INLINE_CHECK.test(body)) rec.via = 'inline';
      if (SIGNATURE_CHECK.test(body)) rec.signed = true;
      rec.fn = r.handler;
      out.push(rec);
      return;
    }
    if (r.framework === 'next') {
      if (NEXT_GUARD.test(src)) rec.via = 'inline';
      if (SIGNATURE_CHECK.test(src)) rec.signed = true;
      rec.fn = r.method;
      out.push(rec);
      return;
    }
    // the route call's own arguments: router.post('/x', requireAuth, async (req, res) => …)
    const head = lines.slice(r.line - 1, r.line + 4).join('\n');
    const args = (/\.\w+\(\s*["'`][^"'`]*["'`]\s*,([\s\S]*?)(?:async\s*)?(?:\(\s*req\b|function\b|\breq\s*=>|\(\s*\{|\(\s*c\s*\)|\(\s*ctx\b)/.exec(head)?.[1] || '');
    const mw = (args.match(/[A-Za-z_$][\w$.]*/g) || []).map((x) => x.split('.').pop()).find(isGuardName);
    const next = routes[idx + 1]?.line || r.line + 60;
    const body = lines.slice(r.line - 1, Math.min(next - 1, r.line + 60)).join('\n');
    const prior = uses.find((u) => u.v === r.var && u.line < r.line);
    if (mw) Object.assign(rec, { guard: mw, via: 'middleware' });
    else if (prior) Object.assign(rec, { guard: prior.name, via: 'router' });
    else if (INLINE_CHECK.test(body)) rec.via = 'inline';
    if (SIGNATURE_CHECK.test(body)) rec.signed = true;
    out.push(rec);
  });
}

/**
 * FastAPI dependency aliases that check the caller, per repo: Map(repo -> Map(alias -> file)).
 * An alias checks the caller when its dependency function does: by its name (admin_user), by taking a
 * checking dependency (Depends(require_login)), or by taking a checking alias itself, e.g.
 * `AdminOffice = Annotated[Office, Depends(admin_owned_office)]` where admin_owned_office takes `admin: AdminUser`.
 * An alias whose own name reads as a check counts too (`AdminAccess = Annotated[dict, Depends(check)]`),
 * unless either name says it's a database session (`SqlSessionDep` over `get_sql_session`).
 */
function pyAliasGuards(files, findings) {
  const byRepo = new Map(); // repo -> { aliases: Map(name -> {fn, file}), fns: Map(name -> {deps, types}) }
  for (const f of files) for (const x of findings(f.path)) {
    if (x.k !== 'py-alias' && x.k !== 'py-dep-fn') continue;
    const k = f.repo ?? '';
    if (!byRepo.has(k)) byRepo.set(k, { aliases: new Map(), fns: new Map() });
    const r = byRepo.get(k);
    if (x.k === 'py-alias') r.aliases.set(x.name, { fn: x.fn, file: f.path });
    else r.fns.set(x.name, { deps: x.deps || [], types: x.types || [] });
  }
  const out = new Map();
  for (const [repo, { aliases, fns }] of byRepo) {
    const ok = new Map();
    const checks = (fn) => isGuardDep(fn) || (!isDbDep(fn) && !!fns.get(fn) && (fns.get(fn).deps.some(isGuardDep) || fns.get(fn).types.some((t) => ok.has(t))));
    const aliasChecks = (name, fn) => checks(fn) || (isGuardDep(name) && !isDbDep(fn));
    // a few passes, so aliases built on other aliases resolve in any file order
    for (let pass = 0; pass < 3; pass++) for (const [name, a] of aliases) if (!ok.has(name) && aliasChecks(name, a.fn)) ok.set(name, a.file);
    if (ok.size) out.set(repo, ok);
  }
  return out;
}

// ---------- per-file entry point ----------

export function findIssues(src, f) {
  const out = [];
  findSecrets(src, f, out);
  if (f.category !== 'code') return out;
  if (f.lang === 'python') pyRoutes(src, f, out);
  else if (['typescript', 'javascript'].includes(f.lang)) jsRoutes(src, f, out);
  // run directly or registered by name (so "nothing imports it" doesn't mean unused)
  if (/^#!|if\s+__name__\s*==\s*['"]__main__['"]|@(?:\w+\.)*(?:task|shared_task|command|group|receiver|scheduled_job|on_event|listener|hookimpl|periodic_task|cli\.command)\b|require\.main\s*===\s*module|import\.meta\.main/m.test(src)) out.push({ k: 'runnable' });
  // a frontend calling a fixed localhost address (breaks once deployed)
  // (only in files with no environment / hostname switching, where it can't be a dev-only fallback)
  if (['typescript', 'javascript', 'vue', 'svelte'].includes(f.lang) && !TEST_FILE.test(f.path) && !/process\.env|import\.meta\.env|NODE_ENV|location\.(hostname|host|origin)|isDev|__DEV__/.test(src)) {
    src.split('\n').forEach((t, i) => {
      if (/["'`]https?:\/\/(localhost|127\.0\.0\.1):\d+/.test(t) && !/\?\?|\|\||local|dev|fallback|default|^\s*(\/\/|\*)/i.test(t.replace(/https?:\/\/(localhost|127\.0\.0\.1)/g, ''))) out.push({ k: 'localhost', line: i + 1 });
    });
  }
  return out;
}

// ======================= build: turn findings into issues =======================

const SEV_ORDER = { high: 0, medium: 1, low: 2 };
const PUBLIC_PATH = /(^|\/)(health|healthz|healthcheck|ping|status|ready|readiness|live|liveness|version|metrics|robots\.txt|sitemap[\w.]*|favicon[\w.]*|\.well-known|login|logout|signin|pair|sign-in|signout|signup|sign-up|register|forgot[\w-]*|reset[\w-]*|verify[\w-]*|oauth\w*|callback|auth|token|refresh|static|assets|public|newsletter|contact|waitlist|unsubscribe|openapi\.json|docs|swagger[\w.]*|i|images?)(\/|$)/i;
const WEBHOOK_PATH = /webhook|\/hooks?\//i;
// endpoints under a webhook URL that manage the webhooks (create, list, test…) rather than receive them
const WEBHOOK_ADMIN = /\/(manage|admin|setup|settings|config|list|create|register|delete|test[\w-]*)(\/|$)/i;
const NOT_USED_KINDS = new Set(['test', 'script', 'infra', 'config', 'worker', 'external']);
const SPECIAL_FILE = /(^|\/)(index|main|app|server|cli|manage|wsgi|asgi|__init__|__main__|conftest|setup|config|settings|env|middleware|instrumentation|layout|page|route|loading|error|not-found|template|default|global-error|sitemap|robots|manifest|opengraph-image|twitter-image|icon|apple-icon|_app|_document|_error|404|500|models|admin|apps|views|urls|signals|tasks|forms|serializers|celery|gunicorn\.conf|vite-env\.d|worker|sw|service-worker)\.[a-z]+$|\.d\.ts$|\.config\.[a-z]+$|\.stories\.|(^|\/)(pages|migrations|alembic|versions|scripts|bin|tools|seeds?|fixtures?|examples?|docs?|public|static|templates|\.github|e2e|cypress|playwright)\//i;
const USED_LANGS = new Set(['typescript', 'javascript', 'python', 'vue', 'svelte', 'astro']);
const plural = (n, w, ws) => `${n} ${n === 1 ? w : ws || w + 's'}`;

export function buildHealth({ graph, scan, arch, root }) {
  const issues = [];
  const nodeById = new Map(graph.nodes.map((n) => [n.id, n]));
  const fileByPath = new Map(graph.files.map((f) => [f.p, f]));
  const scanByPath = new Map(scan.files.map((f) => [f.path, f]));
  const lineage = (id) => { const out = []; let n = nodeById.get(id); while (n) { out.push(n); n = n.parent ? nodeById.get(n.parent) : null; } return out; };
  const kinds = (id) => lineage(id).map((n) => n.kind);
  const isFrontend = (p) => { const f = fileByPath.get(p); return !!f && kinds(f.n).includes('ui'); };
  const nameOf = (p) => fileByPath.get(p)?.name || p.split('/').pop();
  const label = (id) => nodeById.get(id)?.label || id;
  // Files outside the map (config, .env) point at the module that owns their folder instead.
  const dirOwner = new Map();
  for (const f of graph.files) { let d = f.p; while (d.includes('/')) { d = d.slice(0, d.lastIndexOf('/')); if (!dirOwner.has(d)) dirOwner.set(d, new Map()); const m = dirOwner.get(d); m.set(f.n, (m.get(f.n) || 0) + 1); } }
  const itemFor = (p) => {
    if (fileByPath.has(p)) return 'f:' + p;
    let d = p;
    while (d.includes('/')) {
      d = d.slice(0, d.lastIndexOf('/'));
      const m = dirOwner.get(d);
      const top = m && [...m.entries()].filter(([n]) => n !== 'unassigned').sort((a, b) => b[1] - a[1])[0];
      if (top) return 'n:' + top[0];
    }
    return null;
  };
  const findings = (p) => scanByPath.get(p)?.issues || [];

  // ---------- secrets in code and config ----------
  const secretFiles = [];
  for (const f of scan.files) {
    const s = findings(f.path).filter((x) => x.k === 'secret');
    if (s.length) secretFiles.push({ path: f.path, list: s });
  }
  // credential files (service-account JSON, .pem keys) that the scanner doesn't read as config
  for (const f of scan.files) {
    const base = f.path.split('/').pop();
    if (TEST_FILE.test(f.path) || EXAMPLE_FILE.test(f.path) || /\.pub$/.test(base)) continue;
    if (!/\.(pem|key|p12|pfx|jks|keystore)$|^id_(rsa|dsa|ecdsa|ed25519)$|service[-_]?account.*\.json$|^credentials?\.json$|firebase-adminsdk.*\.json$|^client_secret.*\.json$|-key\.json$/i.test(base)) continue;
    let head = '';
    try { head = readHead(path.join(root, f.path), 65536); } catch { continue; }
    if (/PRIVATE KEY|"private_key"\s*:\s*"-----|"client_secret"\s*:\s*"[^"]{8,}/.test(head) || /\.(p12|pfx|jks|keystore)$/i.test(base)) secretFiles.push({ path: f.path, list: [{ k: 'secret', line: 1, what: 'a credentials file (a private key that logs in as your app)', sev: 'high', hint: 'whole file' }], file: true });
  }
  const secretIssue = (group) => {
    const locations = [], notes = {}, items = [];
    const kinds = new Set(), svcs = new Set();
    let frontend = false;
    for (const { path: p, list, file } of group) {
      list.sort((a, b) => a.line - b.line);
      const it = itemFor(p);
      if (it && !items.includes(it)) items.push(it);
      for (const s of list) {
        locations.push({ file: p, line: s.line });
        kinds.add(s.what);
        if (s.svc) svcs.add(s.svc);
      }
      if (isFrontend(p)) frontend = true;
      if (it) notes[it] = (notes[it] ? notes[it] + ' · ' : '') + list.slice(0, 4).map((s) => `${file ? p.split('/').pop() : `line ${s.line}`}: ${s.name ? `${s.name}, ` : ''}${s.what.replace(/ \(.*\)$/, '')} (${s.hint})`).join(' · ') + (list.length > 4 ? ` · +${list.length - 4} more` : '');
    }
    const n = locations.length;
    const sev = group.some((g) => g.list.some((s) => s.sev === 'high')) ? 'high' : 'medium';
    const one = group.length === 1 ? group[0].path : null;
    const kindText = kinds.size === 1 ? [...kinds][0].replace(/ \(.*\)$/, '') : 'passwords or keys';
    return {
      id: 'secret:' + (one || group.map((g) => g.path).join('|')),
      kind: 'secret', severity: sev,
      title: n === 1 ? `A secret key is written into ${one ? one.split('/').pop() : `${group.length} more files`}` : `${n} passwords or keys are written into ${one ? one.split('/').pop() : `${group.length} more files`}`,
      detail: `${one ? `${nameOf(one)} (${one})` : plural(group.length, 'file')} contains ${n === 1 ? kindText : `${n} secrets (${kindText})`}. Anyone who can see the code can use ${n === 1 ? 'it' : 'them'} to get into ${svcs.size ? [...svcs].slice(0, 4).join(', ') : 'the services they unlock'} as you${frontend ? ', and this code is sent to every visitor’s browser' : ''}. Treat ${n === 1 ? 'it' : 'them'} as leaked: move ${n === 1 ? 'it' : 'them'} out of the code and replace ${n === 1 ? 'it with a new one' : 'them with new ones'}.`,
      items, notes, locations,
      fix: [
        `Security fix: hard-coded secrets. Do not print, echo or repeat any secret value in your replies, commits or logs.`,
        ``,
        `These lines contain ${n === 1 ? 'a secret' : 'secrets'} written directly in the code:`,
        ...group.flatMap(({ path: p, list, file }) => list.map((s) => `- ${p}${file ? ' (the whole file)' : `:${s.line}`}: ${s.name ? `${s.name} — ` : ''}${s.what.replace(/ \(.*\)$/, '')}`)),
        ``,
        `For each one:`,
        `1. Replace the literal with an environment variable lookup (${group.some((g) => /\.py$/.test(g.path)) ? `os.environ["NAME"] in Python` : ''}${group.some((g) => /\.py$/.test(g.path)) && group.some((g) => /\.[jt]sx?$/.test(g.path)) ? ', ' : ''}${group.some((g) => /\.[jt]sx?$/.test(g.path)) ? 'process.env.NAME in JavaScript/TypeScript' : ''}${!group.some((g) => /\.(py|[jt]sx?)$/.test(g.path)) ? 'or a secret manager reference' : ''}), using a clear variable name. If the app can't run without it, fail with a clear error when it is missing instead of falling back to a hard-coded value.`,
        `2. Add each variable name (with an empty value, never the real one) to .env.example, and make sure .env is listed in .gitignore.`,
        frontend ? `3. This is frontend code that ships to the browser. Move the call that needs the secret to a server-side route or function so the key never reaches visitors.` : `3. Update any deployment config (Docker, Cloud Run, Vercel, etc.) so the variable is set in production.`,
        group.some((g) => g.file) ? `4. Remove the credentials file from the repository (git rm --cached), add its name to .gitignore, and load it from a path given by an environment variable.` : `4. Run the app or its tests to confirm it still starts with the variables set.`,
        ``,
        `Then give me a short checklist (without the values) of which services these keys belong to, so I can rotate each one: create a new key in that service's dashboard, put it in my environment, and revoke the old one. Because the old values are in git history, assume they are compromised.`,
      ].join('\n'),
    };
  };
  secretFiles.sort((a, b) => b.list.length - a.list.length);
  const MAX_SECRET_ISSUES = 8;
  secretFiles.slice(0, MAX_SECRET_ISSUES).forEach((g) => issues.push(secretIssue([g])));
  if (secretFiles.length > MAX_SECRET_ISSUES) issues.push(secretIssue(secretFiles.slice(MAX_SECRET_ISSUES)));

  // ---------- .env files that are (or will be) committed ----------
  for (const f of scan.files) {
    const base = f.path.split('/').pop();
    if (!/^\.env(\.[\w.-]+)?$/i.test(base) || /example|sample|template|dist|defaults?|schema|test/i.test(base)) continue;
    let src = '';
    try { src = readHead(path.join(root, f.path), 262144); } catch { continue; }
    const secretVars = [];
    src.split('\n').forEach((t, i) => {
      const m = /^\s*(?:export\s+)?([A-Za-z_][\w.]*)\s*=\s*["']?([^"'#\s]*)/.exec(t);
      if (!m || !m[2]) return;
      const n = snake(m[1]);
      const known = FORMATS.some((fm) => { fm.re.lastIndex = 0; const hit = fm.re.test(m[2]); fm.re.lastIndex = 0; return hit; });
      if (known || (SECRET_NAME.test(n) && !PUBLIC_NAME.test(n) && valueLooksReal(m[2], n)) || (/url|uri|dsn/i.test(n) && /:\/\/[^:\s\/]+:[^@\s]+@/.test(m[2]))) secretVars.push({ name: m[1], line: i + 1 });
    });
    if (!secretVars.length) continue;
    const repoAbs = f.repo ? path.join(root, f.repo) : root;
    const relInRepo = f.repo ? f.path.slice(f.repo.length + 1) : f.path;
    const tracked = !!git(repoAbs, ['ls-files', '--', relInRepo]);
    const it = itemFor(f.path);
    issues.push({
      id: 'env-file:' + f.path, kind: 'env-file', severity: tracked ? 'high' : 'medium',
      title: tracked ? 'Your private settings file is saved in git' : 'Your private settings file isn’t hidden from git',
      detail: tracked
        ? `${f.path} holds ${plural(secretVars.length, 'password or key', 'passwords and keys')} and is committed, so everyone with access to the code (and its history) can read them. This file is meant to stay on your computer only.`
        : `${f.path} holds ${plural(secretVars.length, 'password or key', 'passwords and keys')} and isn’t listed in .gitignore, so the next commit will upload it with your code.`,
      items: it ? [it] : [], notes: it ? { [it]: `${f.path}: ${secretVars.slice(0, 6).map((v) => v.name).join(', ')}${secretVars.length > 6 ? '…' : ''}` } : {},
      locations: secretVars.map((v) => ({ file: f.path, line: v.line })),
      fix: [
        `Security fix: ${f.path} contains secrets and ${tracked ? 'is committed to git' : 'is not ignored by git'}. Do not print or repeat any value from it.`,
        ``,
        `1. Add ${base} (and .env*.local) to ${f.repo ? f.repo + '/' : ''}.gitignore, keeping .env.example tracked.`,
        tracked ? `2. Stop tracking it without deleting my local copy: git rm --cached "${relInRepo}" (run inside ${f.repo || 'the repo'}), then commit.` : `2. Confirm with git status that the file no longer shows up as a new file.`,
        `3. Create or update .env.example next to it with the same variable names and empty values: ${secretVars.map((v) => v.name).join(', ')}.`,
        tracked ? `4. Give me a checklist (names only, no values) of which keys I need to rotate in each service's dashboard, since they are in git history. Mention that removing them from history needs a tool like git filter-repo, and ask before doing that.` : `4. Tell me which deployment settings need these variables.`,
      ].join('\n'),
    });
  }

  // ---------- endpoints anyone can call ----------
  const routeFinding = new Map(); // `${file}|${line}|${method}` -> finding
  const globalGuards = new Map(); // repo -> [{framework, name, file}]
  const guardDefs = new Map();    // name -> [{ file, wrapper }]
  for (const f of scan.files) for (const x of findings(f.path)) {
    if (x.k === 'route') routeFinding.set(`${f.path}|${x.line}|${x.method}`, x);
    // app-wide checks count only from the app itself, not from scripts or tests that spin up their own
    if (x.k === 'global-guard' && !TEST_FILE.test(f.path) && !/(^|\/)(scripts?|tools|bin|examples?)\//.test(f.path) && !(fileByPath.has(f.path) && kinds(fileByPath.get(f.path).n).some((k) => NOT_USED_KINDS.has(k)))) { const k = f.repo ?? ''; if (!globalGuards.has(k)) globalGuards.set(k, []); globalGuards.get(k).push({ ...x, file: f.path }); }
    if (x.k === 'guard-def' && !TEST_FILE.test(f.path)) { if (!guardDefs.has(x.name)) guardDefs.set(x.name, []); guardDefs.get(x.name).push({ file: f.path, wrapper: !!x.wrapper }); }
  }
  const aliasGuards = pyAliasGuards(scan.files.filter((f) => !TEST_FILE.test(f.path)), findings);
  for (const [repo, aliases] of aliasGuards) for (const [name, file] of aliases) {
    if (!guardDefs.has(name)) guardDefs.set(name, []);
    guardDefs.get(name).push({ file, alias: true, repo });
  }
  // a sign-in check applied where the route's router is mounted: app.use('/admin', requireStaff, adminRouter)
  const mountGuard = new Map((scan.routes || []).filter((r) => r.mountMw?.some(isGuardName)).map((r) => [`${r.file}|${r.line}|${r.method}`, r.mountMw.find(isGuardName)]));
  const perRepo = new Map(); // repo -> { guarded, open: [], guardCounts }
  graph.routes.forEach((r, i) => {
    const fnd = routeFinding.get(`${r.f}|${r.l}|${r.m}`);
    if (!fnd) return; // framework we don't analyse: say nothing
    const gf = fileByPath.get(r.f);
    if (!gf || kinds(gf.n).some((k) => NOT_USED_KINDS.has(k)) || TEST_FILE.test(r.f)) return;
    const repo = gf.r ?? '';
    if (!perRepo.has(repo)) perRepo.set(repo, { guarded: 0, total: 0, open: [], misplaced: [], guardCounts: new Map(), fileGuards: new Map(), guardedIn: new Map() });
    const pr = perRepo.get(repo);
    pr.total++;
    const global = (globalGuards.get(repo) || []).find((g) => g.framework === 'flask' || g.framework === 'express' || (g.framework === 'next' && fnd.fn === r.m));
    // an unrecognised decorator that turns out to be an auth wrapper defined elsewhere in the project
    const viaWrapper = !fnd.via && fnd.decos?.find((d) => guardDefs.get(d)?.some((g) => (g.file.split('/')[0] === r.f.split('/')[0])));
    const mounted = !fnd.via && mountGuard.get(`${r.f}|${r.l}|${r.m}`);
    // a parameter typed with a dependency alias that checks the caller: `user: AdminUser`
    const viaAlias = !fnd.via && fnd.types?.find((t) => aliasGuards.get(repo)?.has(t));
    if (fnd.via || global || viaWrapper || mounted || viaAlias) {
      pr.guarded++; pr.guardedIn.set(r.f, (pr.guardedIn.get(r.f) || 0) + 1);
      const name = fnd.guard || viaWrapper || mounted || viaAlias;
      if (name) {
        pr.guardCounts.set(name, (pr.guardCounts.get(name) || 0) + 1);
        if (!pr.fileGuards.has(r.f)) pr.fileGuards.set(r.f, new Map());
        pr.fileGuards.get(r.f).set(name, (pr.fileGuards.get(r.f).get(name) || 0) + 1);
      }
      return;
    }
    if (r.m === 'OPTIONS' || r.m === 'HEAD') return;
    // webhook receivers: by URL, or a webhook-ish handler in a webhook file (not that file's admin endpoints)
    const webhook = (WEBHOOK_PATH.test(r.p) && !WEBHOOK_ADMIN.test(r.p)) || (/webhook|hooks?\//i.test(r.f) && /webhook|hook|notify|event|ingress|pubsub|callback|receive|inbound/i.test(r.h || '') && !/create|list|manage|register|delete|test|setup|sync/i.test((r.h || '') + r.p));
    if (webhook && fnd.signed) return;
    if (!webhook && PUBLIC_PATH.test(r.p.replace(/<[^>]*>|\{[^}]*\}|:\w+|\[[^\]]*\]/g, 'x'))) return;
    if (r.p === '/' && r.m === 'GET') return;
    const entry = { i, r, fnd, write: !['GET', 'ANY'].includes(r.m), webhook };
    if (fnd.misplaced) pr.misplaced.push(entry); else pr.open.push(entry);
  });
  for (const [repo, pr] of perRepo) {
    const top = (m) => m && [...m.entries()].sort((a, b) => b[1] - a[1])[0]?.[0];
    // prefer what this file already uses, then the project's most common real (non-wrapper) check
    const guardHint = (file, lang) => {
      const local = top(pr.fileGuards.get(file));
      const name = local || top(new Map([...pr.guardCounts].filter(([n]) => !guardDefs.get(n)?.every((g) => g.wrapper))));
      if (!name) return `the project's existing login check (find how other protected endpoints verify the user; if there is none, ask me before adding one)`;
      const defs = guardDefs.get(name) || [];
      const where = defs.find((g) => g.file === file) ? 'defined in this file' : defs.length ? `defined in ${defs[0].file}` : '';
      const how = defs.some((g) => g.alias) ? `a parameter typed ${name}` : `${lang === 'python' ? '@' : ''}${name}`;
      return `the same check ${local ? 'the other endpoints in this file' : 'the protected endpoints'} already use (${how}${where ? ', ' + where : ''})`;
    };
    const routeLine = (e) => `- ${e.r.m} ${e.r.p} — ${e.r.f}:${e.r.l}${e.fnd.fn && !/^[A-Z]+$/.test(e.fnd.fn) ? ` (${e.fnd.fn})` : ''}`;
    if (pr.misplaced.length) {
      const list = pr.misplaced;
      issues.push({
        id: 'misplaced-guard:' + repo, kind: 'unprotected-endpoint', severity: 'high',
        title: 'A login check is in the wrong place, so it does nothing',
        detail: `On ${plural(list.length, 'endpoint')} the login check (@${list[0].fnd.misplaced}) is written above the route line instead of below it. Flask only sees what's below, so ${list.length === 1 ? 'it is' : 'they are'} open to anyone even though ${list.length === 1 ? 'it looks' : 'they look'} protected.`,
        items: list.slice(0, 24).map((e) => 'r:' + e.i),
        notes: Object.fromEntries(list.slice(0, 24).map((e) => ['r:' + e.i, `@${e.fnd.misplaced} sits above the route line, so it is skipped`])),
        locations: list.map((e) => ({ file: e.r.f, line: e.r.l })),
        fix: [`Security fix: these endpoints have their auth decorator above the route decorator, so the framework registers the unprotected function:`, ...list.map(routeLine), ``, `Move each auth decorator directly below the @…route(...) line (route decorator first, then the auth decorator, then def). Change nothing else, then run the tests that cover these endpoints.`].join('\n'),
      });
    }
    if (!pr.open.length) continue;
    // Nothing in this repo checks the caller: probably protected elsewhere (gateway, private network) or not at all.
    if (pr.guarded === 0) {
      const list = pr.open.sort((a, b) => b.write - a.write);
      issues.push({
        id: 'no-auth:' + repo, kind: 'unprotected-endpoint', severity: 'medium',
        title: `None of ${repo ? `${repo}’s` : 'your'} endpoints check who is calling`,
        detail: `${plural(list.length, 'endpoint')} answer anyone who knows the address, with no login check in the code. That's fine only if this server is private (for example behind a company network or a gateway that checks logins); otherwise anyone could read or change its data.`,
        items: list.slice(0, 24).map((e) => 'r:' + e.i),
        notes: Object.fromEntries(list.slice(0, 24).map((e) => ['r:' + e.i, e.write ? 'Changes data, no login check' : 'No login check'])),
        locations: list.map((e) => ({ file: e.r.f, line: e.r.l })),
        fix: [`Review access control${repo ? ` in ${repo}` : ''}. None of these endpoints verify the caller in code:`, ...list.slice(0, 40).map(routeLine), list.length > 40 ? `- …and ${list.length - 40} more` : null, ``, `First tell me in plain words whether this server is reachable from the internet and whether anything in front of it (gateway, network rules) checks logins. If it is public, propose the smallest safe way to require login for the data-changing endpoints (POST/PUT/PATCH/DELETE) first, show me the plan, and wait for my OK before changing code.`].filter((x) => x !== null).join('\n'),
      });
      continue;
    }
    // One issue per route file
    const byFile = new Map();
    for (const e of pr.open) { if (!byFile.has(e.r.f)) byFile.set(e.r.f, []); byFile.get(e.r.f).push(e); }
    for (const [file, list] of byFile) {
      list.sort((a, b) => b.write - a.write || a.r.l - b.r.l);
      const writes = list.filter((e) => e.write && !e.webhook), hooks = list.filter((e) => e.webhook), reads = list.filter((e) => !e.write && !e.webhook);
      // many providers can't sign webhooks at all, so an unsigned one alone stays low
      const sev = writes.length ? 'high' : reads.length > 2 ? 'medium' : 'low';
      const lang = scanByPath.get(file)?.lang;
      const fileGuarded = pr.guardedIn.get(file) || 0;
      const sv = (n, one, many) => `${n} ${n === 1 ? one : many}`;
      const parts = [];
      if (writes.length) parts.push(sv(writes.length, 'endpoint that changes data (create, update or delete)', 'endpoints that change data (create, update or delete)'));
      if (reads.length) parts.push(sv(reads.length, 'endpoint that returns data', 'endpoints that return data'));
      const nOpen = writes.length + reads.length;
      const hookText = hooks.length ? `${sv(hooks.length, 'webhook (an automatic message from another service) is', 'webhooks (automatic messages from another service) are')} accepted without checking that ${hooks.length === 1 ? 'it really comes' : 'they really come'} from that service, so anyone could send fake ones. Some providers can't sign their messages; if so, that's expected.` : '';
      issues.push({
        id: 'open-endpoints:' + file, kind: 'unprotected-endpoint', severity: sev,
        title: writes.length ? `Anyone could change data through ${nameOf(file)}` : hooks.length ? `Anyone could send fake events to ${nameOf(file)}` : `Anyone could read data from ${nameOf(file)}`,
        detail: [
          parts.length ? `${file.split('/').pop()} has ${parts.join(' and ')} with no login check, so anyone who finds the address can use ${nOpen === 1 ? 'it' : 'them'}${fileGuarded > 0 ? ` (${sv(fileGuarded, 'other endpoint', 'other endpoints')} in the same file ${fileGuarded === 1 ? 'is' : 'are'} protected)` : ''}. If ${nOpen === 1 ? 'it is' : 'some are'} meant to be public, that's fine; otherwise ${nOpen === 1 ? 'it needs' : 'they need'} a login check.` : '',
          parts.length && hooks.length ? `It also accepts ${sv(hooks.length, 'webhook', 'webhooks')} without checking who sent ${hooks.length === 1 ? 'it' : 'them'}.` : hookText,
        ].filter(Boolean).join(' '),
        items: list.slice(0, 24).map((e) => 'r:' + e.i),
        notes: Object.fromEntries(list.slice(0, 24).map((e) => ['r:' + e.i, e.webhook ? 'Webhook with no visible signature check' : e.write ? 'Changes data, no login check' : 'Returns data, no login check'])),
        locations: list.map((e) => ({ file: e.r.f, line: e.r.l })),
        fix: [
          `Security fix in ${file}: these endpoints don't check who is calling:`,
          ...list.map((e) => routeLine(e) + (e.webhook ? ' [webhook]' : '')),
          ``,
          `For each endpoint that isn't meant to be public, add ${guardHint(file, lang)}.`,
          hooks.length ? `For webhooks, verify the provider's signature (using its signing secret from an environment variable) before doing anything, and reject requests that fail.` : null,
          `If an endpoint is intentionally public (for example a public page or a health check), leave it and tell me why. Don't change anything else; afterwards list exactly which endpoints you protected and run the related tests.`,
        ].filter((x) => x !== null).join('\n'),
      });
    }
  }

  // ---------- unused files ----------
  const incoming = new Set();
  for (const l of graph.links) if (l.a !== l.b && l.b.startsWith('f:')) incoming.add(l.b.slice(2));
  const entry = new Set(graph.project?.entrypoints || []);
  const routeFiles = new Set(graph.routes.map((r) => r.f));
  for (const fl of graph.flows || []) for (const s of fl.steps || []) if (s.ref?.startsWith('f:')) entry.add(s.ref.slice(2));
  const candidates = graph.files.filter((f) => {
    if (incoming.has(f.p) || entry.has(f.p) || routeFiles.has(f.p) || !USED_LANGS.has(f.l)) return false;
    if (TEST_FILE.test(f.p) || SPECIAL_FILE.test(f.p) || /(^|\/)app\/.*\/(page|layout|route|loading|error|template|default)\.[jt]sx?$/.test(f.p)) return false;
    if (/(^|\/)src\/routes\//.test(f.p) && /\+|\$|\[/.test(f.p.split('/').pop())) return false;
    if (kinds(f.n).some((k) => NOT_USED_KINDS.has(k))) return false;
    if (findings(f.p).some((x) => x.k === 'runnable' || x.k === 'route')) return false;
    return true;
  });
  // Safety net for dynamic loading: skip a file whose name appears in any other source file.
  if (candidates.length) {
    const want = new Map(candidates.map((f) => [stemOf(f.p), f.p]));
    const mentioned = new Set();
    for (const f of scan.files) {
      if (!['code', 'config', 'docs'].includes(f.category) || f.bytes > 524288) continue;
      let src;
      try { src = fs.readFileSync(path.join(root, f.path), 'utf8'); } catch { continue; }
      for (const tok of new Set(src.match(/[\w$-]+/g) || [])) { const p = want.get(tok); if (p && p !== f.path) mentioned.add(p); }
    }
    const unused = candidates.filter((f) => !mentioned.has(f.p));
    // A module where most files look unused is more likely loaded in a way we can't see; stay quiet.
    const byNode = new Map();
    for (const f of unused) { if (!byNode.has(f.n)) byNode.set(f.n, []); byNode.get(f.n).push(f); }
    const groups = [...byNode.entries()].filter(([n, list]) => { const total = graph.files.filter((f) => f.n === n).length; return n === 'unassigned' || list.length <= 3 || list.length / total <= 0.6; }).sort((a, b) => b[1].length - a[1].length);
    const MAX = 5;
    const merged = groups.length > MAX ? [...groups.slice(0, MAX - 1), ['', groups.slice(MAX - 1).flatMap(([, l]) => l)]] : groups;
    for (const [nid, list] of merged) {
      list.sort((a, b) => b.loc - a.loc);
      const where = nid ? label(nid) : 'other areas';
      const lines = list.reduce((s, f) => s + f.loc, 0);
      issues.push({
        id: 'unused:' + (nid || 'other'), kind: 'unused-file', severity: 'low',
        title: list.length === 1 ? `A file in ${where} looks unused` : `${list.length} files in ${where} look unused`,
        detail: `Nothing in the app imports or calls ${list.length === 1 ? 'this file' : 'these files'} (${lines.toLocaleString('en-US')} lines). Leftover code confuses AI assistants, which may edit or copy the wrong version. Double-check before deleting: some files are loaded in ways the map can't see.`,
        items: list.slice(0, 24).map((f) => 'f:' + f.p),
        notes: Object.fromEntries(list.slice(0, 24).map((f) => ['f:' + f.p, `Nothing uses this file (${f.loc.toLocaleString('en-US')} lines)`])),
        locations: list.map((f) => ({ file: f.p, line: 1 })),
        fix: [`Cleanup check: these files look unused (nothing imports or calls them):`, ...list.slice(0, 40).map((f) => `- ${f.p}`), list.length > 40 ? `- …and ${list.length - 40} more` : null, ``, `For each file, search the codebase (including configs, scripts, HTML templates and dynamic imports) to confirm nothing uses it. Then tell me which ones are safe to delete and which are actually used, with one line of evidence each. Don't delete anything until I confirm.`].filter((x) => x !== null).join('\n'),
      });
    }
  }

  // ---------- very large files ----------
  const big = graph.files.filter((f) => f.loc > 1000 && USED_LANGS.has(f.l) && !TEST_FILE.test(f.p) && !/(^|\/)(migrations|alembic|versions|fixtures?|seeds?|generated|__generated__)\//i.test(f.p) && !/\.d\.ts$/.test(f.p)).sort((a, b) => b.loc - a.loc);
  if (big.length) {
    const huge = big.filter((f) => f.loc >= 2000);
    issues.push({
      id: 'large-files', kind: 'large-file', severity: huge.length ? 'medium' : 'low',
      title: big.length === 1 ? 'One file is very large' : `${big.length} files are very large`,
      detail: `${big.length === 1 ? 'This file has' : 'These files each have'} over 1,000 lines${huge.length ? ` (the biggest has ${big[0].loc.toLocaleString('en-US')})` : ''}. AI assistants read and edit big files less reliably: they lose track of what's where and can break one part while fixing another. Splitting them into smaller files by job makes future changes safer.`,
      items: big.slice(0, 24).map((f) => 'f:' + f.p),
      notes: Object.fromEntries(big.slice(0, 24).map((f) => ['f:' + f.p, `${f.loc.toLocaleString('en-US')} lines`])),
      locations: big.map((f) => ({ file: f.p, line: 1 })),
      fix: [`Refactor plan: these files are very large:`, ...big.slice(0, 15).map((f) => `- ${f.p} (${f.loc.toLocaleString('en-US')} lines)`), ``, `Start with ${big[0].p}. Read it and propose how to split it into smaller files, each with one clear job (for example by feature or by type of work), keeping every public function name and behaviour the same. Show me the plan first. After I approve, move code in small steps, update the imports, and run the tests after each step. Don't change behaviour.`].join('\n'),
    });
  }

  // ---------- frontend hard-coded to localhost ----------
  const local = [];
  for (const f of graph.files) if (isFrontend(f.p)) for (const x of findings(f.p)) if (x.k === 'localhost') local.push({ file: f.p, line: x.line });
  if (local.length) {
    const filesL = [...new Set(local.map((x) => x.file))];
    issues.push({
      id: 'localhost-url', kind: 'hardcoded-url', severity: 'low',
      title: 'The app points at your own computer’s address',
      detail: `${filesL.length === 1 ? 'A frontend file' : `${filesL.length} frontend files`} call${filesL.length === 1 ? 's' : ''} a fixed “localhost” address. That only works on your machine; once the app is online those calls will fail for everyone else.`,
      items: filesL.slice(0, 24).map((p) => 'f:' + p),
      notes: Object.fromEntries(filesL.slice(0, 24).map((p) => ['f:' + p, `localhost address on line ${local.filter((x) => x.file === p).map((x) => x.line).join(', ')}`])),
      locations: local,
      fix: [`These frontend lines use a hard-coded localhost URL:`, ...local.map((x) => `- ${x.file}:${x.line}`), ``, `Replace each with the project's existing API base URL setting (or add one read from a public environment variable such as NEXT_PUBLIC_API_URL / VITE_API_URL, with the localhost value only in .env.development). Don't change anything else.`].join('\n'),
    });
  }

  const KIND_ORDER = ['secret', 'env-file', 'unprotected-endpoint', 'large-file', 'hardcoded-url', 'unused-file'];
  issues.sort((a, b) => SEV_ORDER[a.severity] - SEV_ORDER[b.severity] || KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) || (b.locations?.length || 0) - (a.locations?.length || 0));
  const kept = issues.slice(0, 40);
  const summary = { high: 0, medium: 0, low: 0 };
  for (const i of kept) summary[i.severity]++;
  return { summary, issues: kept };
}

function readHead(file, max) {
  const fd = fs.openSync(file, 'r');
  try { const buf = Buffer.alloc(max); const n = fs.readSync(fd, buf, 0, max, 0); return buf.subarray(0, n).toString('utf8'); } finally { fs.closeSync(fd); }
}
function stemOf(p) {
  const base = p.split('/').pop().replace(/\.[^.]+$/, '');
  return base === 'index' ? p.split('/').slice(-2, -1)[0] : base;
}
