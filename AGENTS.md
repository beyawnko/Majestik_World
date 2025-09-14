# AGENTS.md

**Purpose:**  
This guide formalizes standards and decision-making procedures for AI and human engineering agents contributing to this Veloren Fork project. It ensures all automated and human contributions are safe, predictable, spec-driven, reviewable, and composable in a rich, multi-agent Rust game development environment.

---

## 0. Canonical References

- **Specs:** [./SPECS.md](./SPECS.md)
- **Veloren Book:** https://book.veloren.net/
- **Project Structure:** `/client/`, `/server/`, `/common/`, `/plugin/`, `/voxygen/`, `/assets/`, `/nix/`
- **Contribution/Docs:** `/README.md`, `/CONTRIBUTING.md`, `/docs/`, `/CHANGELOG.md`
- **CI:** `.github/workflows/`
- **Upstream Repo:** https://github.com/veloren/veloren

---

## 1. Senior-Level Principles

**A. Spec and Safety First:**  
- *Never* accept silent ambiguity. Every action must cite the exact section(s) of SPECS.md, plan.md, or issue tracker that authorizes or requires it.
- No agent, human or machine, may “go by feel” or guess; when intent or context is unclear, escalate or open a clarifying issue.
- Modifications must be *minimally invasive* and *maximally documented* at each boundary.

**B. Invariant Protection:**
- Builds must always succeed on all supported targets.
- No step or code change may break, degrade, or bypass:  
  - CI (format, lint, test, build)—fail closed and escalate  
  - Rust formatter & clippy—configurable only with review  
  - Error and result handling guidelines in full—see SPECS.md §11  
  - Data migration on asset, protocol, or schema changes  
  - Public API and plugin stability—see SPECS.md §13  

**C. Composability as Default:**  
- All features are implemented as modular ECS systems, plugins, or config/data extension unless explicitly noted in SPECS.md or reviewed.
- Favor additive and extension-based change models; never monolithically mutate core logic or data flows without both strong precedent and review.

---

## 2. The Senior Agent Workflow

### 2.1 Task Discovery

- All new work is sourced from:  
  - Plan/backlog (plan.md, issues, TODOs in codebase)  
  - Explicit gaps in SPECS.md  
  - CI or doc gaps (test or doc coverage shortfall)
- The agent links the *origin* of the requirement in each commit/PR body.

### 2.2 Mapping & Preflight

- For each action, explicitly map:  
  - The target crate(s), module(s), ECS system(s), plugin(s), or asset dirs  
  - Dependencies (code/data/workflow) that may be affected
- Run a dry-run diff (e.g., `git diff`, or code generation preview) and validate that all changes are strictly limited to the mapped set.

### 2.3 Implementation Norms

- **Rust:**  
  - Idiomatic, readable Rust—favor iterator, match, Result/Early return patterns  
  - Exhaustive error handling, always propagating or explicitly logging errors with context
  - No `.unwrap()`, `.expect()`, or panic except in proven unreachable branches (with reason documented)
- **ECS/Plugins:**  
  - All new features enter via systems, events, or plugin extension points
  - Tests cover default, edge, and failure cases for each system and event
  - Never create ECS-type, resource, or event singletons unless required by design (SPECS.md, or explicit review)
- **Assets/Data:**  
  - Asset and config additions/changes must provide or update reference manifests  
  - Each new asset addition/test is scriptable and revertible (`test_asset_load()` or equivalent)
- **CI:**  
  - Conformance to rustfmt, clippy, and test coverage gates is *non-negotiable*
  - All agent contributors must run local/test CI against the complete workspace prior to PR

### 2.4 Review and Traceability

- All PRs must summarize:
  - Task origin and mapping (see above)
  - Detailed rationale for any deviation from SPECS.md or precedent
  - Impacted files and modules, with diff summary
  - Specific example/test plan repeated in the PR body
  - Checklist that tests/docs/build pass
  - Full attribution (machine/human agent, approval lineage if any escalation)
- PRs affecting public APIs, ECS system registration ordering, or network protocols *require* an explicit migration/test plan.
- Use semantic, atomic commit messages (`[AGENT/PLUGIN] Add spell system per SPECS.md §13`, etc.)

---

## 3. Code and System Boundary Table

| Area                      | Location(s)  | Change Protocol                                     |
|---------------------------|--------------|-----------------------------------------------------|
| Rendering/Graphics        | `/voxygen/`  | Add only modular render stages, docs, tests         |
| ECS Systems/Components    | `/common/`   | Always as additional (never mutative) ECS pieces    |
| Assets, Asset Pipeline    | `/assets/`   | Manifest + test update; never remove w/out review   |
| Plugins/Modding           | `/plugin/`   | Public API events, docs, test plugin included       |
| Server/Networking         | `/server/`, `/common/network/` | Versioned protocol changes only, migration path     |
| Docs/Workflow/CI          | `/docs/`, `.github/` | Only forward-compatible, peer-reviewed changes      |

---

## 4. Escalation, Stoppage, and Human Oversight

- Pause and submit a "blocker" tag on PR/issues if:
  - Any automated inference would cause cross-module/cross-crate breakage
  - The code affects serialization schema, network protocol, or persistent asset/reference files
  - Agent cannot guarantee revertibility
- No silent auto-resume; blocks stay up until reviewed and dispatched
- Human owners may review and merge, but only after full checklists and mapping are present

---

## 5. Deep Codebase Nuances and Expert Guidance

- **Crate structure:**  
  - Treat `/voxygen/` as the graphics/engine client crate  
  - `/common/` for all cross-shared types, ECS, basic protocols  
  - `/plugin/` is the exclusive extension mechanism—never duplicate extension surfaces
- **Upgrades and dependency patching:**  
  - Use `[patch.crates-io]` as the only legal workaround for dependency replacement; otherwise submit upstream first  
  - Bump versions in a separate, documented PR
- **Data-driven/Hot-reload:**  
  - Asset, shader, and config reloading must be—unless impractical—non-blocking and reversible at runtime in dev builds
  - Agents modifying hot-reloadable code paths must write a test and usage snippet

---

## 6. Example Agent-Driven Workflows

**A. Add a new modular armor asset**  
- Place `.vox` model in `/assets/models/armor/`  
- Register reference in manifest  
- Update or add customizable entry in char config  
- Provide load/equip test and doc addition (`/docs/CHARACTERS.md`)

**B. Add plugin event (e.g., for new NPC action)**  
- Extend plugin event API (with version bump, if necessary)  
- Document in `/docs/PLUGIN_API.md`
- Write a stub/example plugin to demonstrate new event/behavior
- Add and reference test coverage in PR body

**C. Modify or extend pipeline/CI**  
- Always as a standalone change, never batched with logic/features  
- Link to CI logs in PR description  
- Document in `/docs/WORKFLOW.md` as appropriate

---

## 7. Agent Update, Self-Reflection, and Process Evolution

- Any agent proposing an update to AGENTS.md, SPECS.md, or major workflow files must make a standalone PR detailing:
  - Justification  
  - Procedure changes  
  - Expected review/test/rollback procedure  
- Major agent-introduced workflow/process upgrades require two independent peer or owner approvals

---

## 8. Template: Agent PR/Commit Message

```

[AGENT] <concise title of action>:

- **Spec(s):** SPECS.md §X.Y[, plan.md, issue \#____, etc.]
- **Target:** <crate>/module/asset_path, ECS system/Component
- **Summary:** <1-2 lines of change, method, rationale>
- **Tests:** <files/commands/tests shown passing>
- **Docs:** <if required, which doc files were updated>
- **Reviewer checklist:**
    - [ ] Build/test on all platforms
    - [ ] Coverage unchanged or increased
    - [ ] Spec and code are in sync
- **Notes:** <escalation, migration, or known gotchas>

```

---

## 9. Appendix: Quick Decision Rules

- **When in doubt, STOP and ask.**  
- **Never remove, always add or extend unless authorized.**
- **Prefer explicitness over brevity in all automated code.**
- **If multiple plausible entry points exist, ask for or propose a review of the extension point first.**
- **Data/configuration is primary; code is secondary except for ECS/events.**
- **No code or system change is above peer and trace review.**

---

*End of file. Last major revision: 2025-09-13*
```

<span style="display:none">[^1]</span>

<div style="text-align: center">⁂</div>

[^1]: SPECS.md

