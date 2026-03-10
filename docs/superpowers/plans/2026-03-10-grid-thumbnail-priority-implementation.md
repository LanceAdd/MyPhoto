# Grid Thumbnail Priority Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make grid first-screen thumbnail rendering hit `<=1s` for visible-90% by prioritizing `grid-256`, then upgrading quality in background, while keeping only `grid` and `preview` persistent categories.

**Architecture:** Introduce a frontend priority thumbnail scheduler and memory-budget LRU for transient fast display, keep persistent cache model with two grid buckets (`256`,`512`), and lower `preview-1600` warmup priority behind active grid work. Add telemetry to prove latency and queue behavior.

**Tech Stack:** Vue 3 + Pinia + TypeScript (frontend), Tauri + Rust (backend), `image` crate, existing invoke/event bridge.

---

## File Structure

- Create: `src/utils/thumb-priority-queue.ts`
- Create: `src/utils/thumb-fast-cache.ts`
- Modify: `src/utils/thumb-cache.ts` (bucket policy `256|512`, memory-budget LRU accounting)
- Modify: `src/utils/thumb-loader.ts` (route generation through priority queue)
- Modify: `src/components/GridView.vue` (visible range reporting)
- Modify: `src/components/PhotoCard.vue` (request priority/context)
- Modify: `src/components/RailThumb.vue` (shared grid policy for cull rail)
- Modify: `src/stores/workspace.ts` (deprioritize preview warmup when grid has active requests)
- Modify: `src-tauri/src/imaging.rs` (timing instrumentation + optional batch telemetry helpers)
- Modify: `src-tauri/src/lib.rs` (emit telemetry event payloads when needed)

## Chunk 1: Grid Bucket And Cache Foundations

### Task 1: Lock `grid` buckets to `256/512`

**Files:**
- Modify: `src/utils/thumb-cache.ts`
- Test: `src/utils/thumb-cache.ts` (temporary inline assertions) and app smoke run

- [ ] **Step 1: Write failing checks for size normalization expectations**

Add temporary assertions (guarded by dev-only block) expecting:
- `normalizeThumbSize(80) === 256`
- `normalizeThumbSize(300) === 256`
- `normalizeThumbSize(600) === 512`

- [ ] **Step 2: Run type/build check to confirm current behavior fails checks**

Run: `cmd /c npm run build`  
Expected: build fails due assertion mismatch or explicit throw.

- [ ] **Step 3: Implement minimal bucket change**

Change `SIZE_PRESETS` to `[256, 512]` and keep nearest-bucket logic unchanged.

- [ ] **Step 4: Remove temporary failing assertions and keep permanent lightweight guard**

Keep a tiny non-throwing dev log or comment documenting bucket policy.

- [ ] **Step 5: Run build to verify pass**

Run: `cmd /c npm run build`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/utils/thumb-cache.ts
git commit -m "feat(grid): reduce persistent thumb buckets to 256 and 512"
```

### Task 2: Add memory-budget LRU for transient fast thumbnails

**Files:**
- Create: `src/utils/thumb-fast-cache.ts`
- Modify: `src/utils/thumb-cache.ts` (integrate or delegate to new module)
- Test: `src/utils/thumb-fast-cache.ts` (pure-function test harness section)

- [ ] **Step 1: Write failing unit-style scenarios in module-local test block**

Define scenarios:
- insert until over budget evicts oldest
- recently accessed entry is retained
- size accounting uses `w*h*4`

- [ ] **Step 2: Run build to verify failures are observed**

Run: `cmd /c npm run build`  
Expected: fail because API not implemented yet.

- [ ] **Step 3: Implement minimal LRU cache with byte cap**

Implement API:
- `createThumbFastCache(maxBytes)`
- `get(key)`
- `put(key, value, estimatedBytes)`
- `stats()`

- [ ] **Step 4: Re-run build**

Run: `cmd /c npm run build`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/utils/thumb-fast-cache.ts src/utils/thumb-cache.ts
git commit -m "feat(grid): add byte-budgeted in-memory fast thumbnail cache"
```

## Chunk 2: Priority Scheduler And Viewport-First Loading

### Task 3: Introduce thumbnail priority queue with dedupe

**Files:**
- Create: `src/utils/thumb-priority-queue.ts`
- Modify: `src/utils/thumb-loader.ts`
- Test: `src/utils/thumb-priority-queue.ts` (module-local scenarios)

- [ ] **Step 1: Write failing queue behavior scenarios**

Scenarios:
- P0 tasks run before P1/P2
- same `(path,size)` joins in-flight promise
- low-priority pending task can be cancelled

- [ ] **Step 2: Run build to confirm red state**

Run: `cmd /c npm run build`  
Expected: fail due missing queue implementation.

- [ ] **Step 3: Implement minimal queue**

Implement:
- fixed concurrency (`2` default)
- priority buckets (`P0..P4`)
- dedupe map for in-flight keys
- cancellation for pending tasks

- [ ] **Step 4: Integrate queue into loader**

Route `ensureGridThumbSrc` through queue submission with priority input.

- [ ] **Step 5: Run build to verify pass**

Run: `cmd /c npm run build`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/utils/thumb-priority-queue.ts src/utils/thumb-loader.ts
git commit -m "feat(grid): add priority thumbnail queue with in-flight dedupe"
```

### Task 4: Report visible range and enqueue viewport-first work

**Files:**
- Modify: `src/components/GridView.vue`
- Modify: `src/components/PhotoCard.vue`
- Modify: `src/components/RailThumb.vue`
- Test: manual UI verification + build

- [ ] **Step 1: Add visible-window index computation in `GridView.vue`**

Expose current visible start/end and one-screen neighbor range.

- [ ] **Step 2: Pass request priority context to cards/rail**

Visible cards -> `P0`, neighbor cards -> `P1`, quality upgrade -> `P2/P3`.

- [ ] **Step 3: Update `PhotoCard.vue`/`RailThumb.vue` loader calls**

Pass priority + intent (`fast` vs `upgrade`) to loader API.

- [ ] **Step 4: Run build**

Run: `cmd /c npm run build`  
Expected: PASS.

- [ ] **Step 5: Manual smoke test**

Run app, open large workspace, confirm:
- visible thumbnails populate first
- offscreen thumbnails lag behind
- no obvious flicker on upgrade

- [ ] **Step 6: Commit**

```bash
git add src/components/GridView.vue src/components/PhotoCard.vue src/components/RailThumb.vue
git commit -m "feat(grid): prioritize visible-range thumbnail scheduling"
```

## Chunk 3: Warmup Priority, Telemetry, And Verification

### Task 5: Deprioritize `preview-1600` warmup when grid demand exists

**Files:**
- Modify: `src/stores/workspace.ts`
- Test: build + manual behavior

- [ ] **Step 1: Add grid-demand activity signal**

Define a lightweight shared flag/timestamp when P0/P1 queue has active work.

- [ ] **Step 2: Gate preview warmup loop using demand signal**

If active grid demand exists, pause preview warmup batch dispatch.

- [ ] **Step 3: Run build**

Run: `cmd /c npm run build`  
Expected: PASS.

- [ ] **Step 4: Manual behavior check**

Open grid during startup:
- grid fill remains responsive
- preview warmup resumes after interaction idle

- [ ] **Step 5: Commit**

```bash
git add src/stores/workspace.ts
git commit -m "perf(warmup): pause preview warmup during active grid demand"
```

### Task 6: Add generation and queue telemetry for KPI validation

**Files:**
- Modify: `src-tauri/src/imaging.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/utils/thumb-priority-queue.ts`
- Test: `src-tauri` unit tests + app logs

- [ ] **Step 1: Write failing Rust test for timing payload shape**

Add test asserting telemetry struct fields exist and serialize.

- [ ] **Step 2: Run Rust tests to confirm failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml imaging::tests:: -- --nocapture`  
Expected: FAIL on missing telemetry type/fields.

- [ ] **Step 3: Implement backend timing capture**

Capture per task:
- `decode_ms`
- `resize_ms`
- `encode_ms`
- `io_ms`

Emit via existing event channel or dedicated telemetry event.

- [ ] **Step 4: Add frontend queue wait telemetry**

Record:
- `thumb_queue_wait_ms`
- `thumb_fast_cache_hit_rate`
- `thumb_disk_cache_hit_rate`

- [ ] **Step 5: Re-run Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml imaging::tests:: -- --nocapture`  
Expected: PASS.

- [ ] **Step 6: Re-run frontend build**

Run: `cmd /c npm run build`  
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/imaging.rs src-tauri/src/lib.rs src/utils/thumb-priority-queue.ts
git commit -m "chore(telemetry): add thumbnail pipeline and queue timing metrics"
```

## Chunk 4: Final Validation And Rollback Guard

### Task 7: Verify KPI and add rollback switch

**Files:**
- Modify: `src/utils/thumb-loader.ts` (feature flag check)
- Modify: `src/components/SettingsPanel.vue` (optional debug toggle)
- Test: end-to-end manual benchmark run

- [ ] **Step 1: Add feature flag to enable new scheduler path**

Flag name example: `thumb.scheduler.v2`.

- [ ] **Step 2: Keep legacy path callable when flag off**

Use old direct invoke flow as fallback branch.

- [ ] **Step 3: Build and run smoke tests in both modes**

Run: `cmd /c npm run build`  
Expected: PASS for both flag values.

- [ ] **Step 4: Capture KPI sample**

Collect:
- `thumb_grid_visible_90_ms`
- memory fast-cache peak bytes
- warmup pause/resume behavior

- [ ] **Step 5: Commit**

```bash
git add src/utils/thumb-loader.ts src/components/SettingsPanel.vue
git commit -m "feat(grid): add scheduler v2 rollback flag and validation hooks"
```

## Global Verification Checklist

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture`
- [ ] `cmd /c npm run build`
- [ ] Manual benchmark on large workspace confirms `visible-90 <= 1s` target trend
- [ ] No regression in cull/lightbox main preview loading path
- [ ] Memory fast-cache stays under configured MB cap during sustained scrolling

## Notes For Executors

- Do not introduce new persistent thumbnail categories beyond `grid` and `preview`.
- Keep changes incremental and commit after each task.
- If KPI misses target, tune in order: queue priority mapping -> worker count -> memory cap.

