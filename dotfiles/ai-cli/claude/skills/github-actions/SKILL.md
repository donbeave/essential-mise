---
name: github-actions
description: GitHub Actions hardening, release signing and mise toolchain pinning for writing, changing or reviewing workflows, runners, releases and tool pins. Use when a repo has .github/workflows or mise.toml.
---

# GitHub Actions, releases and mise

## Workflow hardening
- Default `GITHUB_TOKEN` to `permissions: contents: read`; raise permissions per job.
- Pin third-party actions to full commit SHAs from the action's own repo, and container images by digest. Use workflow lockfiles where the repo has adopted them.
- No `pull_request_target` or `workflow_run` on fork code, and never check out fork code inside them.
- Never interpolate `${{ github.event.* }}` into `run:`; pass untrusted values through `env:`.
- OIDC for cloud and registry access, with `id-token: write` only on the job that needs it. Trusted publishing for package registries.
- Write caches only from trusted events; never cache secrets; never restore caches in release jobs.
- Self-hosted runners: ephemeral or JIT, in separate runner groups, never on public repos.
- Lint with `actionlint` and `zizmor`.
- Verify runner and protocol behavior against upstream source (`actions/runner`, `actions/scaleset`), not memory.

## Releases
Sign keylessly with Sigstore, attach artifact attestations (reusable workflows reach SLSA Build L3) and an SBOM, and gate releases on `gh attestation verify`.

## mise
Pin tools by exact version in `mise.toml` with `mise.lock`, and install with `mise install --locked`. Mirror `rust-toolchain.toml` in mise when the repo does.
