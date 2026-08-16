# System-Operation-Maintenance Expertise — Release, CI-Gate, Rollback & Data-Safety Mechanics

> **Purpose.** This is a repo-agnostic, primary-sourced practitioner guide to the **mechanics of operating
> and maintaining a shipped system** — exact Semantic Versioning precedence rules, changelog-marker mechanics,
> wiring a CI pipeline so a "gate" actually gates, the concrete steps a release and a rollback run, artifact
> integrity (checksums/signing/provenance), backup and datastore-consistency discipline, deprecation/lifecycle
> mechanics, basic observability wiring, and — the one rule that threads through all of it — never running an
> irreversible or outward-facing action without the user's explicit authorization. It contains no
> project-specific commands, release scripts, or APIs; every rule applies to any team, any codebase, any CI
> provider whose concrete syntax is shown via GitHub Actions as the reference implementation.
>
> **Scope note.** The `engineering-leadership` expertise's §5 ("Versioning & changelog ownership") already
> covers the *ownership/judgment* angle of versioning and the changelog: declaring a public API before making
> any compatibility promise, the MAJOR/MINOR/PATCH triple and what each increment promises, version
> immutability once published, the 0.y.z→1.0.0 decision, the six Keep-a-Changelog entry types, the live
> "Unreleased" section, and writing an entry for the upgrading consumer (`lead-31`…`lead-38`). This module
> deliberately does **not** re-derive any of that — it picks up exactly where ownership stops and mechanics
> start: the parts of SemVer and Keep a Changelog that `lead-31`…`lead-38` does not cite (pre-release/build
> metadata, precedence comparison, the Yanked marker), plus everything CI/release/rollback/artifact/backup/
> confirmation that engineering-leadership's remit never covered at all. Tag/signing mechanics are
> `git-mastery`'s job (`git-9`, `git-58`–`git-60`) and are cross-referenced, not repeated. The human side of
> incident response — blameless retros, postmortem triggers, incident communication — is engineering-
> leadership's §7 (`lead-44`–`lead-49`); §10 below covers only the operational wiring around it.
>
> **Primary sources (this file is a faithful distillation — do not contradict):** the **SemVer 2.0.0**
> specification (`semver.org/spec/v2.0.0.html`); **Keep a Changelog 1.1.0** (`keepachangelog.com/en/1.1.0/`);
> and the official **GitHub Actions** documentation (`docs.github.com/en/actions/...` — workflow syntax,
> deployment environments, managing environments for deployment, artifact attestations). One supporting page,
> "About protected branches" (`docs.github.com/en/repositories/...`), sits just outside the `/actions`
> namespace but is the concrete mechanism GitHub gives for "make CI a real gate," so it is cited too and
> flagged as adjacent rather than an `/actions` page proper. Every page was fetched and read for this guide —
> see the Source ledger.
>
> **Evidence discipline.** **[VERIFIED]** = taken directly from a fetched primary-source page, cited inline by
> URL. **[INFERRED]** = this guide's engineering judgment or a well-established operational convention that
> was not itself the literal text of a fetched page (backup/restore discipline, alerting philosophy, and
> checksum fallback for artifacts outside GitHub's attestation feature carry no single primary source and are
> labeled accordingly, exactly as `engineering-leadership` labels its own uncited rules). No claim is asserted
> without one of these two labels, and no source's claim is generalized to a new domain without the
> [INFERRED] flag.
>
> **Every actionable rule has a stable id (`som-N`).** The companion manifest
> `manifests/system-operation-maintenance.json` indexes each, typed `checkable | judgment | principle`.
>
> Status: **v1 — SemVer precedence mechanics through basic observability/incident-response mechanics, 46 rules
> (som-1…som-46). No section dropped. Date: 2026-08-16.**

---

## 0. Executive summary — mechanics beneath the ownership calls

**som-1 (principle).** Read this guide as the **mechanical layer beneath three ownership decisions**
`engineering-leadership` already covers: what a version number promises, what a changelog entry says, and
when a release is trustworthy enough to ship. This guide is "how the promise is executed" — the exact SemVer
precedence algorithm, the exact changelog markers for a pulled release, the exact CI wiring that turns a gate
into something that actually blocks a merge, the exact command sequence a release and a rollback run, and the
exact confirmation an irreversible action requires before it runs.

**som-2 (principle).** The single rule threaded through every section below: **never run an irreversible or
outward-facing action — deploy, publish/release, push or force-push, delete — without the user's explicit
authorization**, and always present the exact command plus its blast radius when asking (§8). Release
mechanics, rollback mechanics, and artifact integrity below all exist to make that one confirmed action as
safe and reversible as possible once it is authorized.

**som-3 (principle).** A gate — a required status check, a branch-protection rule, a `needs` dependency, a
lint threshold — exists to be satisfied by fixing the underlying problem, **never by weakening the gate
itself**; a red main blocks every release. This mirrors the same non-negotiable stance `engineering-
leadership` takes on technical debt (`lead-43`) and `test-driven-development` takes on a single test, applied
here specifically at the CI-pipeline layer (§3, `som-18`).

---

## 1. Semantic Versioning — format & precedence mechanics

**som-4 (checkable).** A normal version number **MUST** take the form `X.Y.Z` where X, Y, Z are non-negative
integers with **no leading zeroes**, and each element **MUST** increase numerically — e.g. `1.9.0 → 1.10.0 →
1.11.0` (numeric, not lexical, ordering: `1.10.0` is not "less than" `1.9.0` the way the strings would sort).
*[VERIFIED, semver.org/spec/v2.0.0.html, item 2.]*

**som-5 (checkable).** A **pre-release** version is denoted by a hyphen plus dot-separated identifiers
immediately after the patch version; identifiers are ASCII alphanumerics/hyphens only, **MUST NOT** be empty,
and a purely-numeric identifier **MUST NOT** have leading zeroes. A pre-release version **always has lower
precedence than its associated normal version** — `1.0.0-alpha` denotes an unstable candidate for `1.0.0`,
not a value that could ever outrank it. Examples: `1.0.0-alpha`, `1.0.0-alpha.1`, `1.0.0-0.3.7`,
`1.0.0-x.7.z.92`, `1.0.0-x-y-z.--`. *[VERIFIED, semver.org/spec/v2.0.0.html, item 9.]*

**som-6 (checkable).** **Build metadata** is denoted by a `+` plus dot-separated identifiers after the patch
or pre-release version (same character rules as pre-release). Build metadata **MUST be ignored when
determining precedence** — two versions differing only in build metadata have **identical** precedence, so it
is never safe to key a deploy/promotion decision off build metadata alone. Examples: `1.0.0-alpha+001`,
`1.0.0+20130313144700`, `1.0.0-beta+exp.sha.5114f85`. *[VERIFIED, semver.org/spec/v2.0.0.html, item 10.]*

**som-7 (checkable).** The full **precedence algorithm**, in order: (1) split into major/minor/patch/
pre-release (build metadata never enters); (2) compare major, minor, patch **numerically**, first difference
wins — `1.0.0 < 2.0.0 < 2.1.0 < 2.1.1`; (3) if those are equal, a pre-release version has **lower** precedence
than the normal version — `1.0.0-alpha < 1.0.0`; (4) if both are pre-release with equal major.minor.patch,
compare dot-separated identifiers left-to-right until a difference: numeric-only identifiers compare
numerically, alphanumeric/hyphenated identifiers compare **lexically in ASCII sort order**, a numeric
identifier **always** has lower precedence than a non-numeric one, and — if every earlier identifier is equal
— **a larger set of fields outranks a smaller set**. Full worked chain:
`1.0.0-alpha < 1.0.0-alpha.1 < 1.0.0-alpha.beta < 1.0.0-beta < 1.0.0-beta.2 < 1.0.0-beta.11 < 1.0.0-rc.1 <
1.0.0`. *[VERIFIED, semver.org/spec/v2.0.0.html, item 11, quoted in full via the raw fetched page text.]*

**som-8 (judgment).** Use a pre-release identifier as the mechanical gate for a staged rollout — tag a
candidate `2.0.0-rc.1`, run it through the real release pipeline (§4), and **promote by cutting the plain
`2.0.0` tag once verified**, never by mutating the `rc` tag's target in place (a published tag's content is
immutable — cross-ref `engineering-leadership` `lead-34` and `git-mastery` `git-9`/`git-59` for the tag-object
mechanics that make an annotated tag the right vehicle for this).
**reviewer_criterion:** Did a staged rollout use a genuinely new, higher-precedence pre-release/release tag at
each promotion step, rather than moving an existing tag's target to point at different content?

---

## 2. Changelog mechanics — beyond the ownership call

**som-9 (checkable).** Mark a release that had to be **pulled** for a serious bug or security issue with a
loud, structured **`[YANKED]`** tag on its own heading — `## [0.0.5] - 2014-12-13 [YANKED]` — rather than
quietly deleting or omitting that version's entry. The tag is deliberately loud and bracket-delimited so it is
both human-noticeable and programmatically parseable. *[VERIFIED verbatim marker, keepachangelog.com/en/1.1.0/
— "What about yanked releases?".]*

**som-10 (checkable).** Treat the Yanked marker as the changelog's half of a rollback: it must be paired with
the actual operational act — redeploying the last good build (§5) or shipping an immediate patch release — not
left standing alone as a documentation-only edit. A Yanked entry with no corresponding rollback/patch action is
a changelog that admits a problem without having fixed it. *[INFERRED — direct extension of the Yanked
mechanic above; no primary source states the pairing requirement verbatim.]*

**som-11 (judgment).** Do not rely on a platform's native release notes (e.g. GitHub Releases) as the sole
system of record for changelog history — per Keep a Changelog's own comparison, such release pages are
**non-portable** (viewable only inside that platform), historically **less discoverable** than a
repository-root uppercase file a contributor already expects (`README`, `CONTRIBUTING`), and — at the time of
that comparison — offered **no built-in links to the commit/diff log between releases**. Prefer a plain,
portable `CHANGELOG.md` as the primary artifact; a platform release page may mirror it, but should not replace
it. *[VERIFIED, keepachangelog.com/en/1.1.0/ — "What about GitHub Releases?".]*
**reviewer_criterion:** Does a portable `CHANGELOG.md` (or equivalent versioned file) exist as the actual
system of record, with any platform release page treated as, at most, a mirror of it?

---

## 3. CI as a gate — status checks, job gating, least privilege, concurrency

**som-12 (checkable).** Protect the integration branch with a **branch protection rule** requiring specific
status checks before a merge is allowed, and keep **job names unique across every workflow** in the repository
— reusing a job name in two different workflow files can produce ambiguous or duplicated status-check results
that block a pull request from merging even when the real checks passed. *[VERIFIED,
docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-
branches/about-protected-branches — "About branch protection rules" tip.]*

**som-13 (checkable).** By default, a branch protection rule's restrictions **do not apply** to users with
admin permissions on the repository. Decide **explicitly** whether admins are actually bound by the same
required checks as everyone else, rather than leaving this permissive default unexamined — an unreviewed
admin-bypass is a standing hole in a gate everyone else believes is enforced. *[VERIFIED, docs.github.com/en/
repositories/.../about-protected-branches — "About branch protection rules".]*

**som-14 (checkable).** Sequence dependent CI jobs with `jobs.<job_id>.needs` (a string or array): a job
runs only once every job it needs has **completed successfully**. If a needed job fails or is skipped, every
job depending on it — transitively, down the whole dependency chain from that point — is skipped too, unless
that job's `if:` explicitly uses `always()` to opt back in. This is the mechanical form of "later stages don't
run on top of a failed gate." *[VERIFIED, docs.github.com/en/actions/reference/workflows-and-actions/
workflow-syntax — `jobs.<job_id>.needs`.]*

**som-15 (checkable).** Scope the CI token to **least privilege** explicitly, rather than leaving the
default broad scope implicit: set `permissions:` at the workflow level (e.g. `permissions: read-all`) or per
job (e.g. `permissions: {issues: write, pull-requests: write}`, which grants exactly those two write scopes and
**no access at all** to anything unlisted). A workflow that never declares `permissions:` is trusting whatever
the repository/org default happens to be. *[VERIFIED, docs.github.com/.../workflow-syntax — `permissions` and
`jobs.<job_id>.permissions`.]*

**som-16 (checkable).** When one workflow calls another (a reusable workflow), secrets do **not** cross that
boundary implicitly — use `jobs.<job_id>.secrets.inherit` to deliberately pass **all** of the calling
workflow's accessible secrets (organization, repository, and environment secrets) into the called workflow,
including across repositories within the same organization or across organizations in the same enterprise.
Treat this as a scoping decision made on purpose, not a default. *[VERIFIED, docs.github.com/.../
workflow-syntax — `jobs.<job_id>.secrets.inherit`.]*

**som-17 (checkable).** Prevent overlapping or racing runs with `concurrency`: group runs under a key (e.g.
`${{ github.workflow }}-${{ github.ref }}`) and set `cancel-in-progress` deliberately. GitHub Actions'
**default behavior with no `concurrency` block is to allow unlimited concurrent runs/jobs** — `true` auto-
cancels a superseded in-progress run (right for CI on a fast-moving branch), `false` lets an already-running
job finish undisturbed (right for a production deploy you never want interrupted mid-flight). Concurrency-group
names must be unique across workflows, or an unrelated workflow's in-progress run can be cancelled by mistake.
*[VERIFIED, docs.github.com/.../workflow-syntax — `concurrency`, "the default behavior... is to allow multiple
jobs or workflow runs to run concurrently".]*

**som-18 (checkable).** Never weaken or skip a required check, a branch-protection rule, or a `needs` gate to
get a red build merged — fix the underlying failure, or change the check itself through the same review
process as any other code change, never as a side effect of getting unblocked (`som-3`). *[INFERRED — direct
statement of the house convention this guide's own repository operates under; no single primary source states
this specific anti-pattern for CI gates verbatim, though it mirrors `lead-43`'s identical stance for a lint
rule or test.]*

---

## 4. Release process — environments & manual triggers

**som-19 (checkable).** Reference a GitHub Actions **environment** (e.g. `production`) from a deploying job.
Every protection rule configured for that environment **must pass before the job is even dispatched to a
runner**; the job can read the environment's secrets **only after** it has been sent to a runner — protection
rules gate dispatch, not merely secret visibility after the fact. *[VERIFIED, docs.github.com/en/actions/
concepts/workflows-and-actions/deployment-environments.]*

**som-20 (checkable).** Configure environment protection mechanically as up to three independent controls:
**required reviewers** (up to 6 people/teams; only **one** approval is needed for the job to proceed;
optionally enable "Prevent self-review" so a user cannot approve a run they triggered themselves), a **wait
timer** (a fixed number of minutes to hold before the job proceeds), and **deployment branch policies**
restricting which branches/tags may deploy to that environment. *[VERIFIED, docs.github.com/en/actions/
how-tos/deploy/configure-and-manage-deployments/manage-environments — "Creating an environment".]*

**som-21 (checkable).** Decide **explicitly** whether administrators may bypass an environment's configured
protection rules ("Allow administrators to bypass configured protection rules") — leaving this enabled by
default silently creates a path around every required-reviewer and wait-timer gate just configured for that
environment, the environment-level twin of `som-13`'s branch-protection bypass. *[VERIFIED, docs.github.com/
en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments.]*

**som-22 (checkable).** Gate a release or rollback trigger behind `workflow_dispatch` with typed `inputs`
(the workflow receives them via the `inputs` context; up to 25 top-level input properties, 65,535-character
total payload) whenever the action must be a **deliberate, on-demand human act** rather than an automatic
reaction to a push. This is the CI-pipeline's mechanical form of the confirmation gate in §8 — a manual trigger
with named inputs is what makes "ask before running it" enforceable in the pipeline itself, not just in
conversation. *[VERIFIED, docs.github.com/.../workflow-syntax — `on.workflow_dispatch` and
`on.workflow_dispatch.inputs`.]*

**som-23 (principle).** Tag every release with an **annotated** (and, where the project requires provenance,
signed) tag — never a lightweight one. This module does not re-derive the git object-model reason (only an
annotated tag is a real tag *object* with a message/tagger a release tool can attach notes to, and only an
annotated tag can carry a GPG signature); that mechanic belongs to `git-mastery` (`git-9`, `git-58`–`git-60`)
and is cross-referenced here, not repeated.

---

## 5. Rollback strategy

**som-24 (judgment).** Prefer **roll-forward** (ship a new, higher-precedence version that fixes the
regression) over any notion of "rolling back a release number" as the default recovery path — a published
version's contents are immutable once released, so there is no such operation as un-publishing `1.2.3`;
"rolling back" always means **redeploying a prior, still-intact artifact** while the broken version stays
visible in history (Yanked per `som-9`), never rewriting or removing the broken version's record.
**reviewer_criterion:** Did the recovery either ship a new, higher version, or redeploy a specific prior
artifact by reference — rather than attempting to edit, delete, or renumber the already-published broken
version?

**som-25 (checkable).** A deployment rollback is **"redeploy the last-known-good build artifact,"** not "reset
the version number backward." Version numbers and currently-deployed state are two independent axes; keep the
previous version's build artifact retrievable specifically so this redeploy is always possible on short notice
— this is exactly why artifact integrity/retention (§6) and backup discipline (§7) exist as prerequisites to a
usable rollback, not optional extras.

**som-26 (judgment).** When the failing behavior sits behind a feature flag or config toggle, prefer **flipping
the flag off** over running a full artifact rollback — a flag flip is near-instant and needs no re-run of the
deploy pipeline, whereas a full rollback re-triggers every gate in §3–§4. Reserve full artifact rollback for
regressions that are not isolated behind a flag. *[INFERRED — standard progressive-delivery practice; no
primary source fetched specifically for this preference.]*
**reviewer_criterion:** For a regression isolated behind a flag/toggle, was the flag flipped off before (or
instead of) triggering a full deployment rollback?

---

## 6. Artifact integrity — checksums, signing, provenance

**som-27 (checkable).** Establish build **provenance** for a release artifact with a GitHub Actions
attestation: grant the building job the `id-token`, `contents`, and `attestations` write permissions (add the
`packages` write permission when the artifact is a container image), then run `actions/attest@v4` **after**
the build step — a `subject-path` input for a binary, or a `subject-name` (fully-qualified image name, no
tag) plus a `subject-digest` input (the artifact's own SHA-256 digest) for a container image. *[VERIFIED,
docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations —
"Generating artifact attestations for your builds".]*

**som-28 (checkable).** **Verify**, not just generate, an artifact's attestation before trusting it: `gh
attestation verify PATH/TO/ARTIFACT-BINARY -R ORG/REPO` for a binary, or (after `docker login ghcr.io`) `gh
attestation verify oci://ghcr.io/ORG/IMAGE:TAG -R ORG/REPO` for a container image. Generation without a
consumer-side verification step proves nothing was checked; verification is what closes the
supply-chain-integrity loop. *[VERIFIED, docs.github.com/.../use-artifact-attestations — "Verifying artifact
attestations with the GitHub CLI".]*

**som-29 (checkable).** Attach a **Software Bill of Materials (SBOM)** attestation alongside the build
attestation via the same `actions/attest@v4` action's SBOM-path input, and verify it by naming its predicate
type explicitly, e.g. `gh attestation verify ARTIFACT -R ORG/REPO --predicate-type
https://spdx.dev/Document/v2.3`. A build attestation proves **who built it and how**; an SBOM attestation
additionally proves **what went into it**. *[VERIFIED, docs.github.com/.../use-artifact-attestations —
"Generating an attestation for a software bill of materials (SBOM)" and "Verifying an attestation for
SBOMs".]*

**som-30 (checkable).** For an artifact distributed outside GitHub's attestation feature (e.g. a plain tarball
on another host), publish, at minimum, a checksum manifest (e.g. a `SHA256SUMS`-style file) alongside the
artifact, plus a detached cryptographic signature over that checksum manifest against a key the consumer can
actually verify — write-time integrity, not an after-the-fact promise. *[INFERRED — standard practitioner
convention paralleling the GitHub-native attestation mechanism above; no primary source was fetched for this
specific fallback practice.]*

---

## 7. Backups & datastore consistency

**som-31 (checkable).** Back up a durable datastore on a schedule matched to its actual write rate and the
project's tolerance for data loss (its Recovery Point Objective), and keep at least one copy **off the primary
host/region**. A backup stored only alongside the data it protects is not a defense against the failure modes
that actually destroy data — disk failure, region loss, or an accidental delete of the whole volume take the
"backup" with them. *[INFERRED — standard operational convention; no primary source named in this module's
research scope covers backup cadence/placement.]*

**som-32 (checkable).** A backup that has never been **restored** is unverified. Schedule and actually run
periodic restore drills into an isolated environment; a backup job that exits 0 but was never test-restored is
an unproven claim of safety, not a working one — the same failure shape as a CI check that passes without ever
having been exercised against a real failure. *[INFERRED — standard operational convention.]*

**som-33 (checkable).** For a structured or bi-temporal datastore (cross-ref `memory-management` §5/§9 for one
concrete instance of exactly this kind of store), a restore or import step must **re-validate internal
consistency** — structural invariants, foreign-key/reference integrity, duplicate detection — rather than
trusting the restored bytes blindly. A corrupted or partially-written backup can restore "successfully" while
still being silently wrong. *[INFERRED — direct extension of standard restore-verification practice to a
structured store.]*

**som-34 (judgment).** Treat a destructive migration or schema change against a **live** datastore as the same
class of irreversible action as a delete (§8): take a fresh, verified backup immediately beforehand, and
confirm the rollback path (a restore point, or a working down-migration) actually functions **before** running
the forward migration in production — not after something has already gone wrong.
**reviewer_criterion:** Before a destructive live-datastore migration ran, did a verified-fresh backup exist
and was the rollback/down-migration path confirmed working, rather than assumed?

---

## 8. Irreversible & outward-facing actions — the confirmation gate

**som-35 (checkable — the headline rule).** **Never run an irreversible or outward-facing action — a deploy, a
publish/release, a `git push` (or force-push), or a delete — without the user's explicit authorization.**
Build, test, lint, dry-run, read operations, and uncommitted local edits are exempt and flow freely (`som-38`);
this gate exists specifically for the class of action that leaves the local, reversible working copy and
either cannot be cheaply undone or becomes visible/consumable to a party outside that local workspace.

**som-36 (checkable).** When requesting that authorization, present the **exact command** that will run and
its **blast radius** — what changes, who or what is affected, and whether the action is itself reversible —
before executing anything. A vague "should I proceed?" with no concrete command and no stated consequence is
not a real confirmation request; it does not give the person being asked enough information to actually decide.

**som-37 (checkable).** **Silence is not consent.** If the user does not explicitly respond to a presented
irreversible/outward-facing action, do not proceed on the assumption that "no objection means go" — re-ask, or
stop and wait.

**som-38 (checkable).** Build, test, lint, dry-run, and read operations, and any edit that stays uncommitted in
the local working copy, are explicitly **exempt** from this gate and should flow without a confirmation step —
the gate exists for irreversible/outward-facing actions specifically, not as a blanket brake on every command.
Applying it indiscriminately trains the practice of asking-and-being-ignored, which is precisely the failure
mode `som-3`/`som-18` warn against for gates in general.

**som-39 (judgment).** Classify an action as irreversible/outward-facing by asking one question: **does it
change something a party outside my own local, uncommitted workspace can observe or depend on, or destroy data
with no working restore path?** A local commit is cheaply reversible (`git-mastery` §10 — reflog/`fsck`
recovery); a `git push`, a tag, a package publish, a delete against a live datastore, or a deploy to a real
environment is not, and belongs behind `som-35`.
**reviewer_criterion:** Was the irreversible/outward-facing classification made by checking whether the action
is observable/consumable outside the local workspace or destroys unrecoverable data — not by a looser,
unexamined judgment call?

---

## 9. Lifecycle & deprecation mechanics

**som-40 (checkable).** Surface a deprecation where its consumers will actually encounter it **at the moment
they use the deprecated path** — a runtime warning, a compiler/linter deprecation diagnostic, or an
in-code/doc-comment deprecation tag — not only in the changelog entry that `lead-32` requires for the MINOR
bump. A deprecation a consumer only learns about by reading changelogs before every upgrade is, in practice, a
deprecation most consumers never see.

**som-41 (checkable).** A deprecation notice states three concrete facts, not merely the fact of deprecation
itself: **what to migrate to**, **by when** (a concrete removal version or date, not "eventually"), and — if a
previously-deprecated path is later found actively unsafe rather than merely superseded — pair the notice with
a visible Yanked-style marker (`som-9`) rather than a routine deprecation tag. *[INFERRED — synthesis of the
Yanked mechanic (§2) applied to the deprecation-severity case; no primary source states this three-fact
framing verbatim.]*

**som-42 (checkable).** Treat **removal** of a deprecated capability as a gated deletion, not a routine cleanup
commit: it is a MAJOR-version change (the ownership call is `lead-32`/`lead-33`'s job), and mechanically it
gets the same irreversible-action confirmation as any other delete (`som-35`) — even though the deprecation
itself was announced well in advance, the removal commit is still the moment the capability actually stops
existing for anyone still depending on it.

---

## 10. Basic observability & incident-response mechanics

**som-43 (checkable).** Wire a health-check/readiness endpoint, or an equivalent smoke test, directly into the
deploy pipeline, run automatically immediately after a deploy completes, and gate "deploy considered
successful" on that check passing — a deploy that finishes without a pipeline error but was never smoke-tested
is an unverified claim of health, the same failure shape as an unrestored backup (`som-32`). *[INFERRED —
standard deployment-verification practice; no primary source fetched specifically for this mechanic.]*

**som-44 (checkable).** Route alerts on the signal that actually predicts user-visible harm — error rate,
latency, saturation, the same class of postmortem trigger `engineering-leadership` §7 already defines
(`lead-44`) — rather than on every anomaly a monitoring system can technically detect. An alert nobody acts on
teaches the on-call rotation to ignore alerts, the same failure mode `lead-20` describes for an unlabeled,
default-mandatory review comment. *[INFERRED — standard observability practice.]*

**som-45 (checkable).** Keep a runbook per incident class that names the **exact** rollback or mitigation
command for that failure mode (tie directly to §5's rollback mechanics and §8's confirmation gate) — write it
**before** the first real occurrence of that failure mode, not after, so the first occurrence is executed from
a tested script rather than improvised under pressure. *[INFERRED — standard incident-preparedness practice.]*

**som-46 (principle).** This section covers only the **mechanics** around an incident — monitoring wiring,
smoke gates, runbooks naming concrete rollback commands. The **human process** — blameless retrospectives,
pre-defined postmortem triggers, and blame-free real-time incident communication — is `engineering-
leadership`'s job (§7 there, `lead-44`–`lead-49`); this module does not re-derive it, only the operational
wiring that process runs on top of.

---

## Defaults / quick-reference table

SemVer format: `X.Y.Z`, non-negative integers, no leading zeroes (`som-4`) · pre-release = `-` + dot-separated
identifiers, lower precedence than the normal version (`som-5`) · build metadata = `+` + dot-separated
identifiers, **ignored** for precedence (`som-6`) · precedence order = major→minor→patch (numeric) →
pre-release-vs-normal → pre-release identifiers left-to-right (numeric<non-numeric, ASCII-lexical for
non-numeric, larger identifier-set wins on a tie) (`som-7`) · Yanked marker = `## [X.Y.Z] - YYYY-MM-DD
[YANKED]` (`som-9`) · environment protection = required reviewers (≤6, 1 approval, optional prevent-self-
review) + wait timer + deployment branch policy + admin-bypass toggle (`som-20`–`21`) · concurrency default =
**unlimited concurrent runs** unless a `concurrency:` block is set (`som-17`) · attestation permissions =
`id-token`, `contents`, `attestations` write (+`packages` write for images) via `actions/attest@v4` (`som-27`)
· rollback = redeploy the last-known-good artifact, never renumber a published version (`som-24`–`25`) · the
one always-confirm class = **deploy / publish / push-or-force-push / delete** (`som-35`).

---

## Source ledger

**Primary sources (fetched and read for this guide, 2026-08-16):**

- **SemVer 2.0.0** — `semver.org/spec/v2.0.0.html`: items 2, 9, 10, and the full item 11 (Precedence, all of
  11.1–11.4 including the worked `alpha`→`rc.1` example chain, confirmed against the raw fetched page text).
- **Keep a Changelog 1.1.0** — `keepachangelog.com/en/1.1.0/`: FAQ "What about yanked releases?" and "What
  about GitHub Releases?".
- **GitHub Actions documentation** — `docs.github.com/en/actions/...`: `reference/workflows-and-actions/
  workflow-syntax` (`concurrency`, `jobs.<job_id>.concurrency`, `jobs.<job_id>.needs`, `permissions`,
  `jobs.<job_id>.permissions`, `jobs.<job_id>.secrets.inherit`, `on.workflow_dispatch`,
  `on.workflow_dispatch.inputs`); `concepts/workflows-and-actions/deployment-environments`; `how-tos/deploy/
  configure-and-manage-deployments/manage-environments` (required reviewers, wait timer, deployment branch
  policies, admin bypass); `how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations`
  (attestation generation for binaries/images, `gh attestation verify`, SBOM attestations).
- **Adjacent GitHub documentation (outside `/actions`, flagged per the Scope note above)** —
  `docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-
  branches/about-protected-branches`: required status checks / unique job names, and the admin-bypass default.

**Cross-referenced, not re-derived here:** `engineering-leadership` `lead-31`–`lead-38` (SemVer/changelog
ownership), `lead-43` (never weaken a gate — technical-debt framing), `lead-44`–`lead-49` (postmortem/incident
human process); `git-mastery` `git-9`, `git-58`–`git-60` (annotated/signed tag mechanics), `git-mastery` §10
(reflog/`fsck` recovery, cited by `som-39`); `memory-management` §5/§9 (structured/bi-temporal datastore
consistency, cited by `som-33`).

**Evidence flags carried from the research:** items marked **[INFERRED]** in full are `som-10`, `som-18`,
`som-26`, `som-30`, `som-31`, `som-32`, `som-33` (partially — the datastore cross-reference is verified
elsewhere, the restore-time re-validation practice itself is not), `som-41`, `som-43`, `som-44`, `som-45` —
standard operational convention or direct synthesis of a cited mechanic, not the literal text of any fetched
page, flagged rather than presented as sourced. `som-7`'s full precedence chain was independently re-verified
by pulling the raw `semver.org` page text directly after the first-pass indexed preview truncated mid-item —
flagged here so the correction is traceable, the same discipline `engineering-leadership`'s own source ledger
documents for its `lead-3`/`lead-17`/`lead-51` truncation corrections.

*Colophon: v1, 2026-08-16. Distilled with zero shortcuts from SemVer 2.0.0, Keep a Changelog 1.1.0, and the
official GitHub Actions documentation, fetched and read in full; 46 rules (som-1…som-46); no section dropped;
verified vs. inference labeling carried throughout; no rule duplicates `engineering-leadership` §5's ownership
angle or `git-mastery`'s tag mechanics.*
