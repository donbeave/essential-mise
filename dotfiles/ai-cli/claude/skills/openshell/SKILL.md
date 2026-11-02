---
name: openshell
description: NVIDIA OpenShell facts and verification method for repos that build on OpenShell sandboxes, gateways, policies, providers, exec and port forwarding, or its gRPC API and SDKs. Use before answering how OpenShell behaves or implementing an OpenShell integration.
---

# OpenShell

OpenShell is NVIDIA's runtime for autonomous AI agents (github.com/NVIDIA/OpenShell, Apache-2.0, pre-1.0). Facts below were checked on 2026-11-01 against `main`; re-verify them at the pinned tag.

## Parts
- **Sandboxes**: isolated runtimes created from images; a **supervisor** runs inside each.
- **Gateway**: the control plane and authentication boundary.
- **Policies**: network is an OPA-evaluated CONNECT proxy, deny-by-default, in a separate network namespace; filesystem is Landlock; process is seccomp plus a privilege drop. Network policy and providers can change at runtime; filesystem and process policy are fixed at sandbox creation.
- **Providers**: hold credentials agents never see, injected only at approved endpoints.

Surfaces: the `openshell sandbox create|connect|exec` and `policy set|update` CLI; the gRPC `OpenShell` service in `proto/openshell.proto` (`CreateSandbox`, `ExecSandbox`, `ExecSandboxInteractive`, `ForwardTcp`, policy, provider and lifecycle RPCs); SDKs for Python, TypeScript, Go and Rust.

## Verifying behavior
1. Use the pinned tag, never `main`: `gh api repos/NVIDIA/OpenShell/contents/proto/openshell.proto?ref=<tag>`.
2. Read `proto/README.md` (canonical for API design) and the policy schema and security docs at the same ref.
3. Read the crate that implements the behavior, and run `openshell --help` from the pinned binary.
4. Cite `NVIDIA/OpenShell@<tag>:path:line`.

## Rules
- Pass explicit gateway and workspace identity with every operation; never rely on a global "active" setting for concurrent work.
- A missing upstream capability gets a blocker report, not a fallback or shim.
