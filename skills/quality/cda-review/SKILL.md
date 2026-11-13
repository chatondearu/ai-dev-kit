---
name: cda-review
description:
  Use this skill to review code. It supports both local changes (staged or working tree)
  and remote Pull Requests (by ID or URL). It focuses on correctness, maintainability,
  and adherence to project standards.
---

# Code Reviewer

This skill guides the agent in conducting professional and thorough code reviews for both local development and remote Pull Requests.

When `CDA_GRAPHIFY=1` and `graphify-out/graph.json` exists, read `graphify-out/GRAPH_REPORT.md` (or query the graph) before grepping or reading files.

## Workflow

### 1. Determine Review Target
*   **Remote PR**: If the user provides a PR number or URL (e.g., "Review PR #123"), target that remote PR.
*   **Local Changes**: If no specific PR is mentioned, or if the user asks to "review my changes", target the current local file system states (staged and unstaged changes).

### 2. Preparation

#### For Remote PRs:
1.  **Checkout**: Use the GitHub CLI to checkout the PR.
    ```bash
    gh pr checkout <PR_NUMBER>
    ```
    If its branch is already checked out in another worktree (a worker's), review there or from `gh pr diff <PR_NUMBER>` instead; do not check it out.
2.  **Preflight**: Execute the project's standard verification suite to catch automated failures early. **Detect the command** rather than assuming one — look for the conventional entrypoint in this order and run the first that applies:
    *   A `Makefile` target such as `make check` / `make test` / `make lint`.
    *   An npm/pnpm/yarn script (`package.json` → `preflight`, `check`, `test`, `lint`).
    *   A language-native runner (`cargo test`, `pytest`, `go test ./...`, etc.).
    *   For project toolchain commands, follow **nix-develop-shell** (host/Mise/direnv bare; `nix develop -c` only after command-not-found, once per batch — never wrap `git`/`gh`/system tools or every command).

    If no verification command is discoverable, skip this step and note it in the review.
3.  **Context**: Read the PR description and any existing comments to understand the goal and history.

#### For Local Changes:
1.  **Identify Changes**:
    *   Check status: `git status`
    *   Read diffs: `git diff` (working tree) and/or `git diff --staged` (staged).
2.  **Preflight (Optional)**: If the changes are substantial, ask the user if they want to run the project's verification suite (detected as in the Remote PR flow above) before reviewing. **Unattended (cda-dev worker):** run the preflight unconditionally, do not prompt to switch branches; ask nothing.

### 3. In-Depth Analysis
Analyze the code changes based on the following pillars:

*   **Correctness**: Does the code achieve its stated purpose without bugs or logical errors?
*   **Maintainability**: Is the code clean, well-structured, and easy to understand and modify in the future? Check: names say what things do, no function does two jobs, existing repo patterns are followed.
*   **Readability**: Is the code well-commented (where necessary) and consistently formatted according to our project's coding style guidelines?
*   **Efficiency**: Are there any obvious performance bottlenecks or resource inefficiencies introduced by the changes?
*   **Security**: Are there any potential security vulnerabilities or insecure coding practices?
*   **Edge Cases and Error Handling**: Are empty, null and boundary inputs and failing calls handled, with no swallowed errors?
*   **Testability**: Does each new or changed branch have a test that fails without it (even if preflight passes)? Suggest additional test cases that would improve coverage or robustness.
*   **Leanness & reuse** (KISS lens): Could this fully work with less code? Does it reinvent a helper, component, service or pattern that already exists in the repo — or something the standard library, the platform, or an already-installed dependency provides? Flag unrequested abstractions, options, wrappers and "for later" code. **Read beyond the diff**: open the code the change touches *and its callers* — the worst finding is often an untouched file that now breaks. State each finding as: what the code does, what goes wrong, the fix, and the cost of skipping it. Never label removing validation, security, accessibility or error handling as "leaner".

**Prove it.** A Critical or Important finding carries its proof, the way `cda-diagnose` Phase 1 does: a loop that goes red on it (failing test, scratch script, curl; outside the tracked tree or deleted after), or the failing input (values, order, timing) traced through every caller to an observable effect. One hop is not a trace. A leanness finding is proven by quoting the existing helper or the request it goes beyond. No proof → it is a question for the author, not a finding.

### 4. Refute
Before the conclusion, hand the Critical and Important findings (not your reasoning) to a fresh sub-agent when the host can start one; otherwise re-check them yourself, each file re-opened. Each must survive:
*   the line exists, says what you quote, and is new or made worse by the change;
*   the input can occur: no type, schema or caller upstream rules it out, nothing downstream absorbs it;
*   it is not a recorded decision (ADRs, plan, issue, `git log -S '<snippet>'`).

Drop what fails; never downgrade a finding to keep it.

### 5. Provide Feedback

#### Structure
*   **Summary**: A high-level overview of the review.
*   **Findings**:
    *   **Critical**: Bugs, security issues, or breaking changes.
    *   **Important**: Maintainability, leanness/reuse, missing tests, performance problems worth fixing before merge.
    *   **Minor**: Formatting or minor style issues (optional).
*   **Refute**: kept n / dropped n.
*   **Conclusion**: Clear recommendation (Approved / Request Changes).

#### Tone
*   Plain and factual: what the code does, what goes wrong, the fix. No filler praise.
*   Explain *why* a change is requested.
*   Make wins visible: one line on what now works.

### 6. Cleanup (Remote PRs only)
*   After the review, ask the user if they want to switch back to the default branch (e.g., `main` or `master`) (skip when running as a cda-dev worker; see Local preflight above).
