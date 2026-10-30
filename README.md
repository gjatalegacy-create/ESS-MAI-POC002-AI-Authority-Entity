---
license: apache-2.0
language:
- en
- sq
tags:
- rust
- reproducibility
- governance-first
- ai-authority
- software
pretty_name: ESS-MAI POC002 AI Authority Entity
---

# ESS-MAI POC002 — AI Authority Entity

**A native hierarchy experiment with five user turns, three operational HOLDs and two durable SHADOW verdicts.** This package exposes the existing LIGHT → QUANTUM → SHADOW runtime, its source contracts and the recorded evidence. It preserves the native paired verdicts **(1,1)** and **(0,0)** and demonstrates request-bound replay without another Vault transaction in the tested cases.

Bledar Gjata proposes **AI Authority Entity** as a paradigm: an independent executive competence that fulfils a defined duty, under the applicable law, within its jurisdiction and constitutional hierarchy. Authority includes useful positive work—coordination, selection, computation and verification—and is not defined merely as permission or a blocking filter. In the proposed ESS-MAI organisation, GCL is the constitution; Supreme, laws and hierarchical authorities organise execution. “Law”, “jurisdiction” and “authority” describe computational contracts, not legal power over people.

The experiment makes this proposal inspectable through existing code. It does not establish universal security, academic novelty or complete materialisation of the paradigm.

**Current validation status:** The complete prepublication command `cargo test --offline --locked --workspace --no-fail-fast -- --test-threads=1` returned **exit 101: 1,821 passed (including 4 doctests), 1 failed, 0 ignored**. The only failing test was `blocked_stdin_obeys_deadline_and_retains_output`, at `light/tests/process_deadline_contracts.rs:35`; the isolated rerun also failed. Native builds had no warnings. The release build, 20 focused tests and five-turn continuity experiment passed; those focused tests are subsets, not an extra 20 unique full-suite tests. See [full execution receipt](evidence/prepublication/complete/workspace-complete.json), [full log](evidence/prepublication/complete/workspace-complete.log), and [diagnostic findings](evidence/prepublication/transport_probe/FINDINGS.txt).

A separate diagnostic probe reproduced a cold-start timing mechanism without changing native code: fresh executable spawn calls took 1,049 ms and 1,149 ms, exceeding the test's 750 ms budget before a main-entry marker was recorded. Warm starts of the same binaries took 11.5 ms and 9.4 ms and retained the 21 stdout and 19 stderr bytes. This supports startup latency as a reproduced mechanism, not an attribution to antivirus or proof of the exact chronology of the original failed run. The failing regression remains reported; no timeout, test or runtime rule was relaxed.

## Public source scope

This release includes **all 719 files of the native runtime source and its vendored build dependency closure**, including LIGHT, QUANTUM, SHADOW, GCL, shared contracts, zero-noi, science-domains, test support, the workspace manifest and lockfile. This is a full native runtime source disclosure, not a small hierarchy-only extract. Each copied file is recorded in [SOURCE_MANIFEST.json](provenance/SOURCE_MANIFEST.json) and is byte-for-byte unchanged from the identified AIGovern-Cyber source snapshot.

The package adds documentation, a Python experiment driver and evidence verifiers. It adds no entity wrapper, replacement authority, global duplicate-execution registry or changes to native rules, thresholds, tests or paired judgments. Original product build outputs and private product state are excluded. The included Vault was created for this isolated experiment; native test fixtures remain identifiable as fixtures.

## Hierarchy and distinct competences

```text
GCL constitutional phase/seal contracts
  LIGHT: coordinate the case and its continuation
  QUANTUM / UCL: order and commission distinct reasoning competences
    ASHT: Historical scope and KiE scope
    PRO: original operators and the KiS scientific endpoint
    NPRO: original negative operators and the KiC scientific endpoint
  SHADOW: verify the dossier and own durable Vault writes
```

This diagram summarises the exercised path, not every native component. ASHT produces scoped material and directives; the QUANTUM caller invokes PRO/NPRO. ASHT does not directly execute those operators. GCL is the constitutional order of the contracts, not an additional voter.

A single authority can perform several **different competences** during a cycle. Repeated function calls are therefore not, by themselves, evidence of duplicated competent work. UCL's `command_asht_routes` moves one instance from `XiYiLoaded` to `AshtScoped`, issuing Historical and KiE commands with distinct scopes. The native state transition rejects repeating that commissioning operation on that same instance. A new cycle may perform its own work.

The [source map](SOURCE_MAP.txt) identifies the actual owners and call sites. Particularly relevant are [UCL](native/runtime/quantum/src/ultimatum_collapse_law.rs), [ASHT](native/runtime/quantum/src/asht_quantum.rs), the [scientific endpoint boundary](native/runtime/quantum/src/kie_measurement.rs), [SHADOW gateway](native/runtime/shadow/src/shadow_gateway.rs) and [Vault](native/runtime/shadow/src/knowledge_vault.rs).

## Recorded experiment

All five inputs concern one continuing case. The model is `era00` at UT1 Julian-date parts `2400000.5 + 54388.0`, with tolerance `1e-12` radians.

| User turn | Input/evidence state | Native result |
|---|---|---|
| 1 | Earth-rotation question; required candidates missing | HOLD / `OperationalNoVerdict`; `knowledge_write=0` |
| 2 | Follow-up still lacks the numeric claim/evidence | HOLD / `OperationalNoVerdict`; `knowledge_write=0` |
| 3 | Claim `0.40228372400281387` radians, with the declared model inputs | KiS=1, KiC=0; SHADOW **(1,1)**; durable T1 and T2 |
| 4 | Follow-up requests an opposing claim without yet supplying it | HOLD / `OperationalNoVerdict`; linked to turn 3's T2 |
| 5 | Claim `0.0` radians, with the same model inputs | KiS=0, KiC=1; SHADOW **(0,0)**; durable T1 and T2 |

These are **five user turns, not five complete native judgment cycles**. The checkpoint contains seven entries because two canonical KiE model renderings are also recorded. Turn 5's native predecessor is turn 3's native identity.

The numerical comparison is an executed, bounded model test. KiS and KiC are predicates over the circular angular residual; they are not probabilities or observations of Earth. A scientific counterexample within that model is distinguished from absent evidence. Endpoint contracts also distinguish invalid scope/proof (`Err`), insufficient capacity (`Held`) and an admitted measurement. A measured zero is not automatically a HOLD.

The native LGC C-kernel diagnostic reported `backend=C_KERNEL; sealed=PASS; stripped=BLOCK; freeze=BLOCK; recovery=PASS`. Hardware checks used the current laptop state without changing their thresholds. Logs distinguish Windows-reported battery information from a mathematical thermal estimate. This experiment measures neither a new physical effect nor energy savings.

### Why other native paths remain open

The historical ASHT route still records **`MISSING_EXACT_POSITIVE`** in the model-supported flow. KiE's `era00` evidence supports its declared model scope; it does not manufacture the historical evidence or discharge obligations belonging to another route. The native caller permits the documented continuation only when the historical absence is reproduced from the exact request/response material, not by treating any error as permission. The remaining gaps stay in the continuation question and evidence.

Turns 1, 2 and 4 have no final knowledge verdict because required candidates or evidence are missing. They preserve the case and request further input. They are not scientific refutations, and the later model result does not retrospectively turn them into completed judgments. Conversely, turn 5 supplies a model counterexample; its negative result must not be described as mere absence or an invalid experiment.

Including the entire native source makes other routes available for scrutiny; it does not validate every route, model, hardware state or input. The demonstrated UCL state guard and SHADOW replay protocol do not globally prevent arbitrary direct PRO/NPRO calls from recomputing work outside that protocol. Interoperation with an external goal-directed AI agent is not implemented or demonstrated here. These remain explicit boundaries rather than conclusions inferred from a successful build.

The process-deadline regression is separate from the ASHT evidence gap. The diagnostic record identifies a reproducible startup-delay mechanism, while the underlying host cause remains unknown. A transport timeout conveys uncertainty; it does not establish rollback or permission to retry.

## Durable replay and conflict evidence

After all five turns, the driver resubmitted both exact native T1 requests and retrieved each T2 twice. Returned bytes matched the saved responses. The Vault stayed at **123,245 bytes**, with SHA-256:

```text
bd8f1169dedc33b2d1542669754f05877714c91d36431abbabf0da5b893c3147
```

For each native cycle, changing one bit of the request SHA in a copied PDL2 lookup while keeping its identity produced exit 70 with **`PD_T1_REQUEST_CONFLICT`**. No conflict response was created and Vault bytes remained unchanged. Replay may write an exported response file; the demonstrated property is **no additional durable Vault transaction for those retries**, not absence of all filesystem writes.

This is request-bound protocol idempotence, together with a same-instance UCL guard. It is not a universal exactly-once guarantee for every native API, arbitrary new instances, or hostile host control. SHA-256 checks content integrity; it does not itself authenticate a producer or confer competence. In particular, [the native LGC algorithm](native/runtime/light/src/lgc_algorithm.rs) contains a source-embedded HMAC key. It is retained unchanged and cannot provide a secret-key authenticity guarantee against someone with this source. No general authentication or security certification is claimed. The experiment also does not establish protection against rollback to a complete valid older store without an external trusted anchor.

## Reproduce and inspect

Start with the read-only checks; they need Python 3 and its standard library:

```text
python verify_sources.py
python verify_evidence.py
```

The recorded native run used Windows, Rust/Cargo **1.98.0**, the GNU target and MinGW GCC. The driver currently names Windows `.exe` binaries. No Linux or macOS execution result is claimed. For a Windows MinGW installation at the path below, run from the package root in PowerShell:

```powershell
$env:PATH = 'C:\msys64\mingw64\bin;' + $env:PATH
$env:CC = 'C:\msys64\mingw64\bin\gcc.exe'
$env:AR = 'C:\msys64\mingw64\bin\ar.exe'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $env:CC
Set-Location native/runtime
cargo build --locked --release --workspace
cargo build --locked --release -p quantum-platform --example pd_iz_input --example pd_recovery_inspect
Set-Location ../..
python run_continuity.py --build native/runtime/target/release --output experiment-new
python verify_evidence.py --evidence experiment-new
```

`experiment-new` must not already exist. It receives an isolated Vault and handoff directory; do not point the experiment at private product state. Cargo can use `--offline` when the locked dependencies are already cached. Default native features, including C kernels and hardware constraints, remain active. A different real hardware state may legitimately produce HOLD; do not relax a constraint to force the recorded outcome.

The documented focused checks cover **20 selected native tests**: UCL (5), KiS/KiC (3), ASHT (6), action order/replay binding (1), reproduced missing evidence (1) and SHADOW protocol/storage (4). These checks and their fixtures are separate from the real isolated Vault demonstration. See [RESULTS.txt](RESULTS.txt), [test execution](evidence/EXECUTION.json), [protocol tests](evidence/protocol_tests/EXECUTION.json), [build evidence](evidence/complete_build/) and [continuity evidence](evidence/continuity/). The complete suite result and its single failing regression are disclosed at the top of this document.

## Review, attribution and reuse

The [review protocol](REVIEW_PROTOCOL.txt) gives precise questions, commands and a reporting template. Counterexamples, failed reproductions and justified corrections are welcome. Neither praise nor a review is required to use the licensed code. Contributions should receive accurate attribution for what they establish; a passing test is not an independent peer review.

**Bledar Gjata** is the author of the proposed paradigm and architecture and directed this work. Implementation, analysis and documentation involved AI assistance, with human architectural direction and review, including shell inspection and recorded reproduction. Those activities are disclosed as the working method, not independent validation. Rust, C, hashes, HMAC, state machines, transactional storage and numerical reference models are established techniques; this package does not claim their invention.

The authored ESS-MAI/AIGovern-Cyber source and capsule are released under [Apache-2.0](LICENSE). The vendored `sofars` code retains its [MIT license](native/vendor/sofars-0.6.1/LICENSE); other dependencies retain their respective licenses. See [NOTICE](NOTICE).

- [POC002 GitHub repository](https://github.com/gjatalegacy-create/ESS-MAI-POC002-AI-Authority-Entity)
- [POC002 Hugging Face dataset](https://huggingface.co/datasets/gjata-legacy/ess-mai-poc-002-ai-authority-entity)
- [Versioned software DOI](https://doi.org/10.5281/zenodo.23813496)
- [Companion preprint DOI](https://doi.org/10.5281/zenodo.23813625)
- [Author LinkedIn](https://www.linkedin.com/in/bledar-gjata-1a527030b/)
- [OpenAIRE discovery route](https://explore.openaire.eu/search/result?pid=10.5281%2Fzenodo.23813496) (new indexing is not confirmed).
- [ESS-MAI parent project](https://github.com/gjatalegacy-create/ESS-MAI) and [parent archive](https://doi.org/10.5281/zenodo.22750188)
- [Bledar Gjata's ORCID](https://orcid.org/0009-0003-7854-266X)

Use [CITATION.cff](CITATION.cff) to cite this software. The parent archive identifies its origin, not independent confirmation of the POC002 results.
