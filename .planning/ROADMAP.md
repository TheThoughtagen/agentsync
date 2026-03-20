# Roadmap: aisync

## Milestones

- ✅ **v1.0 aisync** — Phases 1-5 (shipped 2026-03-07)
- ✅ **v1.1 Adapter Expansion & Plugin SDK** — Phases 6-11 (shipped 2026-03-09)
- ✅ **v1.2 Real-World Hardening** — Phases 12-16 (shipped 2026-03-09)

## Phases

<details>
<summary>✅ v1.0 aisync (Phases 1-5) — SHIPPED 2026-03-07</summary>

- [x] Phase 1: Foundation and Data Model (2/2 plans) — completed 2026-03-05
- [x] Phase 2: Core Sync Loop MVP (5/5 plans) — completed 2026-03-05
- [x] Phase 3: Memory and Hooks (5/5 plans) — completed 2026-03-06
- [x] Phase 4: Watch Mode and Bidirectional Sync (5/5 plans) — completed 2026-03-06
- [x] Phase 5: Polish and Distribution (3/3 plans) — completed 2026-03-06

</details>

<details>
<summary>✅ v1.1 Adapter Expansion & Plugin SDK (Phases 6-11) — SHIPPED 2026-03-09</summary>

- [x] Phase 6: Core Refactoring (3/3 plans) — completed 2026-03-08
- [x] Phase 7: Windsurf & Codex Adapters (2/2 plans) — completed 2026-03-08
- [x] Phase 8: Add-Tool Command (2/2 plans) — completed 2026-03-09
- [x] Phase 9: Plugin SDK Crate Extraction (2/2 plans) — completed 2026-03-09
- [x] Phase 10: Declarative TOML Adapters (2/2 plans) — completed 2026-03-09
- [x] Phase 11: Compile-Time Registration (2/2 plans) — completed 2026-03-09

</details>

<details>
<summary>✅ v1.2 Real-World Hardening (Phases 12-16) — SHIPPED 2026-03-09</summary>

- [x] Phase 12: Types & Trait Foundation (1/1 plan) — completed 2026-03-09
- [x] Phase 13: Multi-File Rule Sync (2/2 plans) — completed 2026-03-09
- [x] Phase 14: MCP Server Config & Security (2/2 plans) — completed 2026-03-09
- [x] Phase 15: Command Sync (2/2 plans) — completed 2026-03-09
- [x] Phase 16: Init Completeness (2/2 plans) — completed 2026-03-09

</details>

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation and Data Model | 2/3 | In Progress|  | 2026-03-05 |
| 2. Core Sync Loop MVP | v1.0 | 5/5 | Complete | 2026-03-05 |
| 3. Memory and Hooks | v1.0 | 5/5 | Complete | 2026-03-06 |
| 4. Watch Mode and Bidirectional Sync | v1.0 | 5/5 | Complete | 2026-03-06 |
| 5. Polish and Distribution | v1.0 | 3/3 | Complete | 2026-03-06 |
| 6. Core Refactoring | v1.1 | 3/3 | Complete | 2026-03-08 |
| 7. Windsurf & Codex Adapters | v1.1 | 2/2 | Complete | 2026-03-08 |
| 8. Add-Tool Command | v1.1 | 2/2 | Complete | 2026-03-09 |
| 9. Plugin SDK Crate Extraction | v1.1 | 2/2 | Complete | 2026-03-09 |
| 10. Declarative TOML Adapters | v1.1 | 2/2 | Complete | 2026-03-09 |
| 11. Compile-Time Registration | v1.1 | 2/2 | Complete | 2026-03-09 |
| 12. Types & Trait Foundation | v1.2 | 1/1 | Complete | 2026-03-09 |
| 13. Multi-File Rule Sync | v1.2 | 2/2 | Complete | 2026-03-09 |
| 14. MCP Server Config & Security | v1.2 | 2/2 | Complete | 2026-03-09 |
| 15. Command Sync | v1.2 | 2/2 | Complete | 2026-03-09 |
| 16. Init Completeness | v1.2 | 2/2 | Complete | 2026-03-09 |

### Phase 1: add cursor plugin ecosystem support

**Goal:** Add hooks translation, skills sync, and agents sync for Cursor's plugin ecosystem — fix translate_hooks from Unsupported to Supported, add SkillEngine/AgentEngine loaders, wire into SyncEngine
**Requirements**: TBD
**Depends on:** Phase 0
**Plans:** 2/3 plans executed

Plans:
- [ ] 01-01-PLAN.md — Foundation types, trait methods, and SkillEngine/AgentEngine loaders
- [ ] 01-02-PLAN.md — CursorAdapter hooks fix, skills sync, agents sync, AnyAdapter dispatch
- [ ] 01-03-PLAN.md — SyncEngine wiring, hook path routing, action execution

---
*Roadmap created: 2026-03-08*
*v1.0 milestone shipped: 2026-03-07*
*v1.1 milestone shipped: 2026-03-09*
*v1.2 milestone shipped: 2026-03-09*
