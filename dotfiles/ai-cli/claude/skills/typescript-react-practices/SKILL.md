---
name: typescript-react-practices
description: Version-specific TypeScript 7 and React 19 rules for implementing or reviewing web UI code. Use when working in a repo with tsconfig.json and React.
---

# TypeScript and React practices

Check the repo's pinned versions first; these rules apply when it uses TypeScript 7 and React 19.

## TypeScript 7
- Native `tsc`; tools that need the compiler API may use the TypeScript 6 compatibility package.
- Removed options are hard errors. Never add `ignoreDeprecations` to get past them.
- `import type` under `verbatimModuleSyntax`. Under `erasableSyntaxOnly`, no enums, namespaces or parameter properties.
- Exhaustive checks with `x satisfies never`, in place of the `const _exhaustive: never = x` form in `pstack:typescript-best-practices`.

## React 19
- Mutations as Actions (`useActionState`, `useOptimistic`, `<form action>`); read promises and context with `use()`.
- `ref` is a plain prop; no `forwardRef`.
- With the React Compiler enabled, do not hand-add `useMemo` or `useCallback` unless an effect needs a stable identity. Follow the Rules of React.
