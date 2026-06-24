# Context7 Skill

## Purpose

Fetches **live, version-specific documentation** for external libraries and
frameworks using the Context7 API. Ensures you always get current API patterns
instead of potentially outdated training data.

**Golden Rule**: Always fetch live docs for external libraries — training data
may be outdated.

## Quick Start

Invoke the skill directly via `curl` (no API key required for basic usage):

```bash
# Step 1: Search for the library
curl -s "https://context7.com/api/v2/libs/search?libraryName=LIBRARY&query=TOPIC" | jq '.results[0]'

# Step 2: Fetch documentation
curl -s "https://context7.com/api/v2/context?libraryId=LIBRARY_ID&query=OPTIMIZED_QUERY&type=txt"
```

See `SKILL.md` for the full API reference and examples.

## Supported libraries

See `library-registry.md` for the complete list with aliases and
query-optimization patterns, including:

- **Database & ORM**: Drizzle, Prisma
- **Authentication**: Better Auth, NextAuth.js, Clerk
- **Frontend**: Next.js, React, TanStack Query/Router/Start
- **Infrastructure**: Cloudflare Workers, AWS Lambda, Vercel
- **UI**: Shadcn/ui, Radix UI, Tailwind CSS
- **State**: Zustand, Jotai
- **Validation**: Zod, React Hook Form
- **Testing**: Vitest, Playwright

## Workflow

```
User asks about a library
        ↓
Match it in library-registry.md (aliases + query patterns)
        ↓
Build an optimized Context7 query
        ↓
Fetch live docs via the API (SKILL.md)
        ↓
Return current, actionable documentation
```

## Files

- **`SKILL.md`** — Context7 API documentation and usage
- **`library-registry.md`** — supported libraries, aliases, and query patterns
- **`README.md`** — this file (overview and quick start)

## Adding new libraries

1. Edit `library-registry.md`
2. Add an entry under the appropriate category:

```markdown
#### Library Name
- **Aliases**: `alias1`, `alias2`, `package-name`
- **Docs**: https://example.com/docs
- **Context7**: `use context7 for library-name`
- **Common topics**: topic1, topic2, topic3
```
