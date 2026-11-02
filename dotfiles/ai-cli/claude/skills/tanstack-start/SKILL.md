---
name: tanstack-start
description: 'TanStack Start, Router and Query API facts for routing and data-loading work: rendering modes, generated route trees, loaders with Query, search params, boundaries and preloading. Use when a repo depends on @tanstack/react-start or @tanstack/react-router. Verify every API against the pinned versions; they move between minor releases.'
---

# TanStack Start, Router and Query

The repo's rules win (for example an SPA-only rule). Use the APIs the pinned versions and the repo's existing code use.

- **Modes**: SPA mode (`spa: { enabled: true }` in the Start plugin) prerenders only the shell, so hosting must rewrite unknown paths to it. SSR is per route (`ssr: true | false | 'data-only'`); a child can only restrict its parent. In an SPA-only repo, verify with dev and build runs, not an SSR smoke test, and report server-only code you find.
- **Route tree**: file routes generate the route tree during dev and build. Regenerate it; never hand-edit it.
- **Loaders and Query**: define `queryOptions` once and share it between loader and component; prefetch in the loader and read with `useSuspenseQuery`; pass the `QueryClient` through router context. Where the pinned Query version deprecates `ensureQueryData`, `fetchQuery` and `prefetchQuery`, use `queryClient.query()` in their place.
- **Search params**: validate with a schema in `validateSearch`, with fallbacks. List only the params the loader uses in `loaderDeps`.
- **Boundaries**: `pendingComponent`, `errorComponent` (reset with the query error-reset boundary and `router.invalidate()`) and `notFoundComponent`. Guards go in `beforeLoad`.
- **Preloading**: `defaultPreload: 'intent'` with `defaultPreloadStaleTime: 0`, so Query's `staleTime` decides freshness.
- **Query keys**: one factory hierarchy including every variable the query function uses, so prefix invalidation after actions reaches related queries.
