---
name: cda-review
description:
  Use this skill to review code. It supports both local changes (staged or working tree)
  and remote Pull Requests (by ID or URL). It focuses on correctness, maintainability,
  and adherence to project standards.
---

# Code Reviewer

This skill guides the agent in conducting professional and thorough code reviews for both local development and remote Pull Requests.

Severity, proof and refute follow [contract.md](contract.md): read it before step 3. A finding without proof is not a finding.

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
2.  **Preflight**: Execute the project's standard verification suite to catch automated failures early. **Detect the command** rather than assuming one — look for the conventional entrypoint in this order and run the first that applies:
    *   A `Makefile` target such as `make check` / `make test` / `make lint`.
    *   An npm/pnpm/yarn script (`package.json` → `preflight`, `check`, `test`, `lint`).
    *   A language-native runner (`cargo test`, `pytest`, `go test ./...`, etc.).
    *   For project toolchain commands, follow **nix-develop-shell** (host/Mise/direnv bare; `nix develop -c` only after command-not-found, once per batch — never wrap `git`/`gh`/system tools or every command).

    If no verification command is discoverable, skip this step and note it in the review.
3.  **Context**: Read the PR description, linked issue/plan and any existing comments: that is the **stated intent** every hunk must serve (contract § intent).

#### For Local Changes:
1.  **Identify Changes**:
    *   Check status: `git status`
    *   Read diffs: `git diff` (working tree) and/or `git diff --staged` (staged).
2.  **Preflight (Optional)**: If the changes are substantial, ask the user if they want to run the project's verification suite (detected as in the Remote PR flow above) before reviewing. **Unattended (cda-dev worker):** run the preflight unconditionally, do not prompt to switch branches; ask nothing.

### 3. In-Depth Analysis
The pillars below say **where to look**. What you report, and at which severity, follows [contract.md](contract.md): a Critical or Important carries a class and a proof (reproduced, traced or compared).

*   **Correctness**: Does the code achieve its stated purpose without bugs or logical errors?
*   **Maintainability**: Is the code clean, well-structured, and easy to understand and modify in the future? Check: names say what things do, no function does two jobs, existing repo patterns are followed.
*   **Readability**: Is the code well-commented (where necessary) and consistently formatted according to our project's coding style guidelines?
*   **Efficiency**: Are there any obvious performance bottlenecks or resource inefficiencies introduced by the changes?
*   **Security**: Are there any potential security vulnerabilities or insecure coding practices?
*   **Edge Cases and Error Handling**: Are empty, null and boundary inputs and failing calls handled, with no swallowed errors?
*   **Testability**: Does each new or changed branch have a test that fails without it (even if preflight passes)? Suggest additional test cases that would improve coverage or robustness.
*   **Leanness & reuse** (KISS lens): Could this fully work with less code? Does it reinvent a helper, component, service or pattern that already exists in the repo — or something the standard library, the platform, or an already-installed dependency provides? Flag unrequested abstractions, options, wrappers and "for later" code. **Read beyond the diff**: open the code the change touches *and its callers* — the worst finding is often an untouched file that now breaks. State each finding as: what the code does, what goes wrong, the fix, and the cost of skipping it. Never label removing validation, security, accessibility or error handling as "leaner".

### 4. Refute
Before any verdict, run contract § Refute on every Critical and Important: in a fresh sub-agent (findings + contract only) when the host can start one, otherwise yourself, file re-opened. Unproven findings are dropped, not downgraded.

### 5. Provide Feedback

#### Structure
*   **Summary**: What the change does, what blocks, what must happen to pass (2-4 sentences).
*   **Intent**: ok / missing / mismatch — one line.
*   **Findings**: Critical, then Important, then Minor, in the contract's finding format.
*   **Checks**: preflight commands and results, or "not run".
*   **Refute**: kept / adjusted / dropped counts.
*   **Conclusion**: Request Changes iff a Critical or Important survived the refute; otherwise Approved (Minors listed).

#### Tone
*   Plain and factual: what the code does, what goes wrong, the fix. No praise, no hedging.
*   Explain *why* a change is requested.

### 6. Cleanup (Remote PRs only)
*   After the review, ask the user if they want to switch back to the default branch (e.g., `main` or `master`) (skip when running as a cda-dev worker; see Local preflight above).
