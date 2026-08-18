# Engineering-Leadership Expertise — Senior-Developer Practice & Project-Management Discipline

> **Purpose.** This is a repo-agnostic, primary-sourced practitioner guide to **engineering leadership as a
> practice, not a title** — how a senior engineer decomposes and scopes work, reviews code as both author and
> reviewer, manages risk and reversibility, owns versioning and the changelog, tracks and pays down technical
> debt, runs a calm and blameless incident response, and communicates and mentors through writing and decision
> records. It contains no project-specific commands or APIs — every rule applies to any team, any codebase,
> any language.
>
> **Primary sources (this file is a faithful distillation — do not contradict):** the **SemVer 2.0.0**
> specification (`semver.org/spec/v2.0.0.html`); **Keep a Changelog 1.1.0** (`keepachangelog.com/en/1.1.0/`);
> Google's **eng-practices** code-review guides (`google.github.io/eng-practices/review/...`); **Conventional
> Comments** (`conventionalcomments.org`); Martin Fowler's **Yagni** and **Technical Debt Quadrant** bliki
> posts (`martinfowler.com/bliki/...`); the **Google SRE Book**, Chapter 15, "Postmortem Culture: Learning
> from Failure" (`sre.google/sre-book/postmortem-culture/`); Michael Nygard's **"Documenting Architecture
> Decisions"** (`cognitect.com/blog/2011/11/15/documenting-architecture-decisions`); Jeff Bezos's **2016
> Letter to Amazon Shareholders**, "High-Velocity Decision Making" (`aboutamazon.com/news/company-news/
> 2016-letter-to-shareholders`); and the **2020 Scrum Guide** (`scrumguides.org/scrum-guide.html`). Every
> page was fetched and read in full (or its full raw text pulled directly) for this guide — see the Source
> ledger.
>
> **Scope note.** §5 (Versioning & changelog ownership) covers the *leadership/ownership* angle — the
> promises a version number and a changelog entry make, and whose judgment call each is. It deliberately
> does **not** duplicate release-pipeline mechanics (tagging, publishing, CI gating) — that is the
> `system-operation-maintenance` expertise's job.
>
> **Evidence discipline.** **[VERIFIED]** = taken directly from a fetched primary-source page, cited inline
> by URL, with genuinely verbatim text held to quotation marks only where the exact string was confirmed
> against the raw fetched page (not a chunked preview). **[INFERRED]** = this guide's engineering judgment
> or a well-established practitioner convention that was not itself the literal text of a fetched page. No
> claim is asserted without one of these two labels.
>
> **Every actionable rule has a stable id (`lead-N`).** The companion manifest
> `manifests/engineering-leadership.json` indexes each, typed `checkable | judgment | principle`.
>
> Status: **v1 — work decomposition through communication & mentoring, 56 rules (lead-1…lead-56). No section
> dropped. Date: 2026-08-16.**

---

## 0. Executive summary — the practices, then the load-bearing rules

Engineering leadership, in this guide, is not a title, a headcount, or an org-chart box. It is a small set
of **practices that reduce everyone else's uncertainty** — about what "done" means, about whether this
change is safe to merge, about whether this decision can be undone, about what actually happened during the
outage, about why the code is shaped the way it is six months from now. Three truths dominate the rest of
this guide, so they lead.

**lead-1 (principle).** Model engineering leadership as three intertwined disciplines, each covered below:
**decompose & scope** (turn ambiguous work into small, reviewable, boundable units — §1–§2), **verify &
respond** (code review, risk, incidents — §3–§4, §7), and **own the record** (versioning, changelog,
technical debt, decisions — §5–§6, §8). None of it depends on authority over other people; all of it depends
on making the system, and the team's shared understanding of it, legible.

**lead-2 (principle).** Two axes recur in nearly every section below and should be checked explicitly on any
non-trivial call: **reversibility** — can this be cheaply undone, and therefore made fast and delegated, or
is it a one-way door that deserves slower, more careful deliberation (§4)? — and **legibility** — will
someone who wasn't in the room be able to reconstruct *why* this was done, from what got written down (a PR
description, a changelog entry, a postmortem, an ADR — §3, §5, §7–§8)?

**lead-3 (principle).** **"Better, not perfect" is the load-bearing standard threaded through this entire
guide** — through code review (§3), scope control (§2), and technical-debt tradeoffs (§6) alike. Google's own
code-review documentation states the underlying reason directly: **"there is no such thing as 'perfect'
code—there is only better code. Reviewers should not require the author to polish every tiny piece of a CL
before granting approval."** The goal everywhere below is not a flawless artifact; it is a codebase, a
system, and a team that are *measurably improving* over time, in small, reviewable increments.
**[VERIFIED, google.github.io/eng-practices/review/reviewer/standard.html.]**

The rest of this guide is the mechanism behind those three: work decomposition and estimation (§1),
definition of done / acceptance criteria / scope control (§2), code review discipline as author and reviewer
(§3), risk management and reversibility (§4), versioning and changelog ownership (§5), technical-debt
management (§6), incident basics (§7), and communication & mentoring including decision records (§8).

---

## 1. Work decomposition & estimation

**lead-4 (checkable).** Decompose work into units that are **independently reviewable and independently
revertible** — the same logic that governs code review (§3) governs task breakdown. A unit of work should be
small enough that one reviewer can hold its entire diff in their head, and rejecting or rolling it back
doesn't take unrelated work down with it. *[VERIFIED, google.github.io/eng-practices/review/developer/
small-cls.html — "Why Write Small CLs?"]*

**lead-5 (checkable).** When a piece of work seems to require one large, indivisible change, treat that as a
signal to look harder for a decomposition — a refactor-first unit of work that paves the way for a cleaner,
smaller implementation — before accepting the large unit as necessary. Per Google's own guidance, genuinely
needing a large, unsplittable CL is **"very rarely"** true. *[VERIFIED, google.github.io/eng-practices/
review/developer/small-cls.html — "Can't Make it Small Enough".]*

**lead-6 (judgment).** Prefer **stacking** small increments — sending one small unit for review, then
immediately starting the next one based on it — over batching all work into one large handoff. This keeps
reviewers unblocked, avoids wasted work if an early direction is rejected, and keeps you from being blocked
waiting on review. *[VERIFIED, google.github.io/eng-practices/review/developer/small-cls.html — "Why Write
Small CLs?" and "Writing Small CLs Efficiently".]*
**reviewer_criterion:** Is work being planned/sent as a sequence of small, independently reviewable
increments (stacked or otherwise), rather than one large batched handoff?

**lead-7 (judgment).** Treat every estimate as a **range with a confidence level**, not a single point
number, and re-estimate openly when new information changes the actual shape of the work rather than
defending the original guess. An estimate made before the work is understood is necessarily less accurate
than one made partway through it — communicate that uncertainty instead of hiding it behind false precision.
*[INFERRED — standard practitioner convention; no single primary source was fetched for this rule, and it is
labeled accordingly rather than attributed to a specific citation.]*
**reviewer_criterion:** Was the estimate communicated as a range/confidence rather than a bare point number,
and was it revisited when the shape of the work materially changed?

**lead-8 (judgment).** For novel or uncertain work, prefer sizing **relative to a known reference unit**
("about as involved as X") over an absolute hours/days figure — relative sizing is easier to calibrate over
time than absolute estimates, which compound error under uncertainty. *[INFERRED — standard practitioner
convention, no primary citation fetched.]*
**reviewer_criterion:** For uncertain work, is the size expressed relative to a known comparable unit rather
than as an unqualified absolute-time figure?

---

## 2. Definition of done, acceptance criteria & scope control

**lead-9 (checkable).** Work from an explicit, **written** Definition of Done as the quality bar for every
unit of work, not an implicit or personal one. Per the Scrum Guide: **"The Definition of Done is a formal
description of the state of the Increment when it meets the quality measures required for the product... If
a Product Backlog item does not meet the Definition of Done, it cannot be released or even presented."** Work
is not done because it compiles or because the person who wrote it believes it's finished — it is done when
it meets the agreed, written bar. *[VERIFIED, scrumguides.org/scrum-guide.html — "Commitment: Definition of
Done".]*

**lead-10 (checkable).** Treat adherence to the Definition of Done as a **standing accountability of whoever
is doing the work**, not a gate imposed afterward by someone else — the Scrum Guide lists "instilling quality
by adhering to a Definition of Done" alongside planning and adapting as a core Developer accountability.
*[VERIFIED, scrumguides.org/scrum-guide.html — "Developers".]*

**lead-11 (checkable — YAGNI).** Do not build a capability merely because you presume the software will need
it in the future — "You Aren't Gonna Need It." Build the problem known to need solving *now*; the
future problem, once it actually arrives, can be solved with its real, observed shape and requirements, which
speculative up-front design cannot correctly anticipate. *[VERIFIED, martinfowler.com/bliki/Yagni.html.]*

**lead-12 (checkable — gold-plating).** Treat **over-engineering** — code made more generic than it needs to
be, or functionality added that isn't presently needed — as a defect a reviewer should flag, the same as any
other correctness or complexity problem. Per Google's review guidance: **"Encourage developers to solve the
problem they know needs to be solved now, not the problem that the developer speculates might need to be
solved in the future."** *[VERIFIED, google.github.io/eng-practices/review/reviewer/looking-for.html —
"Complexity".]*

**lead-13 (judgment).** Apply scope control per-decision, not as a blanket refusal to ever design for
extension. YAGNI does not mean "never generalize" — it means the **burden of proof sits with the speculative
feature**, and the default answer without that proof is "not yet, build it when the real need is visible."
*[INFERRED — synthesis of lead-11 (Fowler's Yagni) and lead-12 (Google's over-engineering guidance); no
single primary source states this framing verbatim.]*
**reviewer_criterion:** For any generalization or "just in case" capability, has its need been demonstrated
by a concrete, present requirement — rather than assumed for a hypothetical future one?

**lead-14 (judgment).** Write acceptance criteria **before** implementation begins, as concrete, testable,
falsifiable statements — a criterion that nothing could fail to satisfy is not a criterion. This is what
turns a Definition of Done (lead-9) from a general quality bar into a specific, checkable bar for one piece
of work. *[INFERRED — standard BDD/agile convention; no primary source fetched specifically for acceptance-
criteria phrasing.]*
**reviewer_criterion:** Do the acceptance criteria for this unit of work exist before the implementation, and
is each one concrete enough that a reviewer could point to evidence proving it was or wasn't met?

---

## 3. Code review discipline — author and reviewer

**lead-15 (checkable).** Default to **small CLs/PRs**: "the right size for a CL is one self-contained
change" that "makes a minimal change that addresses just one thing," includes its own related test code, and
errs toward too-small over too-large. Small changes are reviewed more quickly and more thoroughly, are less
likely to hide an introduced bug, waste less work if their direction is rejected, are easier to merge, and
are simpler to roll back cleanly. *[VERIFIED, google.github.io/eng-practices/review/developer/small-cls.html
— "Why Write Small CLs?" and "What is Small?".]*

**lead-16 (checkable).** When a change genuinely cannot be made small, that is a documented **exception
path**, not silent noncompliance: get the reviewer's advance consent, warn them what is coming, and be extra
diligent about tests — expect the review itself to take longer as the tradeoff. *[VERIFIED,
google.github.io/eng-practices/review/developer/small-cls.html — "Can't Make it Small Enough".]*

**lead-17 (checkable — the reviewer's standard).** As reviewer, favor approving a CL once it is in a state
that **definitely improves the overall code health of the system**, even if it isn't perfect — "there is no
such thing as 'perfect' code—there is only better code." Do not withhold approval to force polish on every
detail; balance the duty to protect code health against developers' need to make forward progress.
*[VERIFIED verbatim, google.github.io/eng-practices/review/reviewer/standard.html — "The Standard of Code
Review".]*

**lead-18 (checkable).** Run a fixed rubric on every review, not a mood: **design, functionality
(including any UI and any parallel/concurrent code), complexity (including over-engineering, lead-12), tests
(present and well-designed), naming, comments (explaining *why*, not *what* — see lead-54), documentation,
and conformance to house style.** *[VERIFIED, google.github.io/eng-practices/review/reviewer/looking-for.html
— "Summary".]*

**lead-19 (checkable).** Review **every line** you were asked to review and understand what it does — don't
skim a human-written block and assume its contents are fine. If the code is too hard to follow, that itself
is reviewable feedback: ask the author to clarify rather than approving what you don't understand.
*[VERIFIED, google.github.io/eng-practices/review/reviewer/looking-for.html — "Every Line".]* Also look at
the CL in its wider file/system **context** — four new lines can be the tipping point that makes a 50-line
method need splitting — and never accept a CL that degrades the code health of the system as a whole, even by
a small amount; most systems become complex through the accumulation of many small, individually-accepted
complexities. *[VERIFIED, google.github.io/eng-practices/review/reviewer/looking-for.html — "Context".]*

**lead-20 (checkable).** **Label the severity of review comments** so the author can triage them correctly:
distinguish a required change from a suggestion from pure information, e.g. **"Nit:"** (minor, technically
correct but low-impact), **"Optional"/"Consider:"** (a good idea, not required), **"FYI:"** (not expected in
this change at all). Unlabeled comments default to being read as mandatory, which either blocks unnecessarily
or trains authors to start ignoring feedback altogether. *[VERIFIED, google.github.io/eng-practices/review/
reviewer/comments.html — "Label comment severity".]*

**lead-21 (checkable).** Structure each comment as **label [+ optional decoration]: subject**, with the
reasoning ("why," and "next steps") in the body — e.g. `suggestion (non-blocking): rename this for clarity`.
Recommended labels include `praise`, `nitpick`, `suggestion`, `issue`, `todo`, `question`, and `thought`;
recommended decorations include `(blocking)`, `(non-blocking)`, and `(if-minor)`. This is strictly more
actionable than an unlabeled remark, and is machine-parseable besides. *[VERIFIED,
conventionalcomments.org — "Format", "Labels", "Decorations".]*

**lead-22 (checkable).** Comment on the **code**, never the **developer**, especially when the point is
contentious — Google's own before/after example: *bad* — "Why did **you** use threads here when there's
obviously no benefit..."; *good* — "The concurrency model here is adding complexity to the system without
any actual performance benefit that I can see." Always explain your reasoning rather than issuing a bare
verdict, and balance giving an explicit instruction against simply naming the problem and letting the
developer choose the fix. *[VERIFIED, google.github.io/eng-practices/review/reviewer/comments.html —
"Courtesy" and "Summary".]*

**lead-23 (checkable).** Actively look for something to **sincerely** praise in every review, and never leave
false praise — false praise is actively damaging, not neutral. *[VERIFIED, conventionalcomments.org —
"Labels", the `praise` row.]*

**lead-24 (checkable).** Respond to a review request within **one business day** at the latest, even if the
full, in-depth review must follow later — it is the silence, not a review that takes a few rounds, that
blocks a team. *[VERIFIED, google.github.io/eng-practices/review/reviewer/speed.html — "How Fast Should Code
Reviews Be?".]*

**lead-25 (checkable).** When author and reviewer cannot reach consensus through comments, escalate
deliberately instead of letting the review stall: first try a **synchronous conversation** (video call or
face to face), then **record its outcome back onto the CL** as a comment for future readers, and if that
still doesn't resolve it, follow a defined **escalation path** — broader team discussion, a Technical Lead, a
maintainer of the code, or an Engineering Manager. **"Don't let a CL sit around because the author and the
reviewer can't come to an agreement."** *[VERIFIED verbatim, google.github.io/eng-practices/review/reviewer/
standard.html — "Resolving Conflicts".]*

---

## 4. Risk management & reversibility

**lead-26 (checkable).** Sort every non-trivial decision by **reversibility first**, and let that dictate how
much process it deserves: **"Many decisions are reversible, two-way doors. Those decisions can use a
light-weight process."** A cheaply reversible decision should be made fast, with a lightweight process,
often delegated to whoever is closest to the work — applying a slow, heavyweight process uniformly to every
decision is itself a cost, not a safety measure. *[VERIFIED, aboutamazon.com/news/company-news/
2016-letter-to-shareholders — "High-Velocity Decision Making".]*

**lead-27 (checkable).** For a reversible decision, don't wait for near-certainty before deciding —
**"most decisions should probably be made with somewhere around 70% of the information you wish you had. If
you wait for 90%, in most cases, you're probably being slow."** Pair this with being good at quickly
recognizing and correcting a decision that turns out to be wrong; being slow is a cost you always pay, being
wrong is a cost you often don't have to. *[VERIFIED, aboutamazon.com/news/company-news/
2016-letter-to-shareholders — "High-Velocity Decision Making".]*

**lead-28 (judgment).** When you have conviction on a direction but the team hasn't reached consensus, say so
explicitly and ask collaborators to **"disagree and commit"** rather than either silently overriding them or
letting the disagreement stall the decision indefinitely. This tool is for unblocking a decision that is
reversible enough to gamble on — reserve slower, consensus-seeking process for genuinely irreversible or
safety-critical calls (lead-26). *[VERIFIED, aboutamazon.com/news/company-news/2016-letter-to-shareholders —
"High-Velocity Decision Making".]*
**reviewer_criterion:** Was "disagree and commit" invoked only to unblock a reversible decision under genuine
disagreement (not to silence dissent, and not on an irreversible/safety-critical call)?

**lead-29 (judgment).** Rank identified risks by **likelihood × impact**, then apply **reversibility as a
third multiplier**: a low-likelihood, high-impact, *irreversible* risk (data loss, a public release with no
rollback, a security exposure) should outrank a higher-likelihood but cheaply-reversible one, even though a
naive likelihood-times-impact score alone might not show it. *[INFERRED — standard risk-management practice,
combined with lead-26's reversibility axis; no single primary source was fetched specifically for this
three-factor framing.]*
**reviewer_criterion:** Does the risk ranking explicitly account for reversibility, not just likelihood and
impact — i.e. would a low-likelihood but irreversible risk be caught even if its naive score looks small?

**lead-30 (checkable).** Escalate a stuck decision or an unresolved risk rather than letting it sit
unaddressed — the same discipline as code-review conflict resolution (lead-25) generalizes to any stalled
call: attempt consensus, hold a recorded synchronous conversation if that fails, then follow a defined
escalation path (team discussion, technical lead, code/system owner, or manager). Don't let indecision itself
become the risk. *[VERIFIED, google.github.io/eng-practices/review/reviewer/standard.html — "Resolving
Conflicts" (same citation as lead-25, generalized here to the risk/decision case rather than code review
specifically).]*

---

## 5. Versioning & changelog ownership

**lead-31 (checkable).** **Declare a public API explicitly** — in code or in documentation — before making
any SemVer compatibility promise about it. SemVer's entire contract only covers what has been declared
public; nothing else is bound by it. *[VERIFIED, semver.org/spec/v2.0.0.html, item 1.]*

**lead-32 (checkable).** Own the triple that a version number promises: **increment MAJOR when you make
incompatible API changes, MINOR when you add functionality in a backward-compatible manner (also required
whenever public API functionality is marked deprecated), PATCH when you make backward-compatible bug fixes
only.** Reset MINOR and PATCH to 0 on a MAJOR bump; reset PATCH to 0 on a MINOR bump. That triple is the
entire promise a version number makes to its consumers — nothing more, nothing less. *[VERIFIED,
semver.org/spec/v2.0.0.html — "Summary" and items 6–8.]*

**lead-33 (judgment).** Treat a MAJOR bump as a deliberate cost/benefit decision, not a number to avoid
incrementing. SemVer's own FAQ frames the requirement as a forcing function: **"Having to bump major versions
to release incompatible changes means you'll think through the impact of your changes, and evaluate the
cost/benefit ratio involved"** — not a scoreboard to keep low by avoiding honest major bumps. *[VERIFIED,
semver.org/spec/v2.0.0.html — FAQ, "If even the tiniest backward incompatible changes... won't I end up at
version 42.0.0 very rapidly?".]*
**reviewer_criterion:** Was a backward-incompatible change actually shipped as a MAJOR bump (rather than
disguised as MINOR/PATCH to avoid a "big number"), with the cost/benefit of the break considered first?

**lead-34 (checkable).** Once a version is published, its contents are **immutable** — any correction, no
matter how small, ships as a new version number; a released version is never silently edited in place.
*[VERIFIED, semver.org/spec/v2.0.0.html, item 3.]*

**lead-35 (checkable).** Own the decision of when to leave **0.y.z "initial development"** (where "anything
MAY change at any time" and the API is not considered stable) and commit to **1.0.0**: do it once the
software is used in production, has an API users depend on, or you find yourself worrying about breaking
that API. Staying below 1.0.0 indefinitely is itself a decision — it tells every consumer "nothing here is
stable yet." *[VERIFIED, semver.org/spec/v2.0.0.html, item 4, and FAQ "How do I know when to release
1.0.0?".]*

**lead-36 (checkable).** Own the changelog as a document **for humans, not a git-log dump** — "Don't let your
friends dump git logs into changelogs." Group entries under the six standard types (**Added, Changed,
Deprecated, Removed, Fixed, Security**), list the latest version first, date every release, and state whether
the project follows Semantic Versioning. *[VERIFIED, keepachangelog.com/en/1.1.0/ — tagline, "Guiding
Principles", "Types of changes".]*

**lead-37 (checkable).** Maintain a live **"Unreleased"** section at the top of the changelog so upcoming
changes are visible before they ship; at release time, move its contents under the new version heading. This
is the documented, low-effort way to keep the changelog honest without an end-of-cycle scramble to reconstruct
what happened. *[VERIFIED, keepachangelog.com/en/1.1.0/ — "How can I reduce the effort required to maintain a
changelog?".]*

**lead-38 (judgment).** Write each changelog entry for the **consumer deciding whether to upgrade**, not for
yourself remembering what you did — state the user-visible effect, not the internal diff. A changelog and a
commit log serve different readers and should not be conflated; per Keep a Changelog's own framing,
changelogs are "for humans, not machines." *[VERIFIED, keepachangelog.com/en/1.1.0/ — tagline and "Guiding
Principles" ("Changelogs are for humans, not machines").]*
**reviewer_criterion:** Does the changelog entry describe the user-visible effect of the change (what a
consumer needs to know before upgrading), rather than restating the commit/diff itself?

---

## 6. Technical-debt management

**lead-39 (checkable).** Reserve the **technical-debt metaphor** for a considered, short-term-beneficial
decision to adopt a design strategy that isn't sustainable long-term (e.g. to make a release) — not for
"a mess," code that is merely poorly made by people unaware of better practice. Conflating the two blunts the
metaphor's real value: communicating a genuine tradeoff to non-technical stakeholders. *[VERIFIED,
martinfowler.com/bliki/TechnicalDebtQuadrant.html, citing Robert C. Martin's "a mess is not a technical
debt".]*

**lead-40 (checkable).** Classify debt on **two independent axes**: **reckless vs. prudent** — was the
tradeoff a considered, informed choice, weighing whether the payoff is worth the eventual cost? — and
**deliberate vs. inadvertent** — was it taken on knowingly, or only visible in hindsight, once the team
understood what the design should have been? The response should differ by quadrant: prudent-deliberate debt
("we must ship now, and know exactly what we're deferring") is a legitimate, trackable tradeoff; reckless
debt taken on knowingly, choosing "quick and dirty" because the team believes it can't afford clean code, is
close to negligence — "people underestimate where the [design payoff line] is." *[VERIFIED,
martinfowler.com/bliki/TechnicalDebtQuadrant.html.]*

**lead-41 (checkable).** Expect **prudent-inadvertent** debt as a normal cost of learning while building — it
is common for a team to understand, partway through a project, what the design should have been from the
start; the moment that realization lands, the team has debt it did not choose but must still account for.
This kind of debt "is inevitable and thus should be expected," even from the best teams — it is not evidence
of a mistake to assign blame for. *[VERIFIED, martinfowler.com/bliki/TechnicalDebtQuadrant.html.]*

**lead-42 (checkable).** For any debt you knowingly carry, decide explicitly whether you are paying ongoing
**interest** or scheduling a payoff of the **principal** — the same choice a real loan presents. An unpaid
debt keeps costing (interest); refactoring toward the better design pays down the principal. A prudent debt
with genuinely small interest payments (e.g. in a rarely touched part of the codebase) may not be worth
paying down at all — that is itself a legitimate call, but it should be made explicitly, not by default.
*[VERIFIED, martinfowler.com/bliki/TechnicalDebtQuadrant.html.]*

**lead-43 (checkable).** Never pay down debt, or hit a deadline, by **weakening the gate that measures code
health** — a lint rule, a test, a coverage threshold, an acceptance criterion. That converts visible,
trackable debt into invisible risk, which is the opposite of what the debt metaphor is for: making a tradeoff
legible enough to manage deliberately (lead-39–lead-42). *[INFERRED — direct extension of the debt-tracking
discipline in lead-39–lead-42; no primary source states this specific anti-pattern verbatim, though it
follows directly from treating debt as something to track rather than hide.]*

---

## 7. Incident basics — triage, blameless retro, communication

**lead-44 (checkable).** Define incident/postmortem **triggers before an incident happens**, not during one.
Common triggers: user-visible downtime or degradation past a threshold, any data loss, on-call engineer
intervention (a release rollback, rerouting traffic), a resolution time above some threshold, or a monitoring
failure that meant the incident was discovered manually. Pre-defining the bar means nobody argues, mid-
incident, about whether "this counts." *[VERIFIED, sre.google/sre-book/postmortem-culture/ — "Google's
Postmortem Philosophy".]*

**lead-45 (checkable).** Write every qualifying incident up with the goal of **documentation, root-cause
understanding, and concrete preventive action** — never as punishment. **"Writing a postmortem is not
punishment—it is a learning opportunity for the entire company."** A write-up produced under blame becomes a
document people route around, not one people actually learn from. *[VERIFIED, sre.google/sre-book/
postmortem-culture/ — "Google's Postmortem Philosophy".]*

**lead-46 (checkable).** Keep the retro **blameless** — critique the actions and the system that allowed
them, never the person: **"Removing blame from a postmortem gives people the confidence to escalate issues
without fear."** A blame culture teaches people to hide incidents rather than surface them, which is strictly
worse for the organization than the incident itself. *[VERIFIED verbatim, sre.google/sre-book/
postmortem-culture/ — "Google's Postmortem Philosophy".]*

**lead-47 (checkable).** Do not stigmatize a person or team that produces incident write-ups **often** — a
team surfacing many incidents is doing the visible, correct thing; punishing that visibility "risks creating
a culture in which incidents and issues are swept under the rug," which raises organizational risk rather
than lowering it. *[VERIFIED, sre.google/sre-book/postmortem-culture/ — "Google's Postmortem Philosophy".]*

**lead-48 (judgment).** During an active incident, separate **"make it stop"** from **"understand why."**
Mitigate first (rollback, reroute, fail over — exactly the kind of on-call intervention lead-44 treats as a
trigger worth logging) and defer full root-cause analysis to the calm, blameless postmortem process
afterward, rather than attempting deep RCA while the system is still actively degraded. *[INFERRED — standard
incident-response practice; consistent with, but not a verbatim quote from, the SRE postmortem-triggers
material in lead-44.]*
**reviewer_criterion:** During the incident itself, was effort spent on mitigation/rollback first, with deep
root-cause analysis explicitly deferred to the postmortem rather than attempted mid-incident?

**lead-49 (judgment).** Communicate incident status in **plain, factual, blame-free language** while it's
still ongoing — what's affected, what's being done, what isn't yet known. The same "comment on the system,
not the person" discipline that governs code review (lead-22) and the blameless retro (lead-46) applies to
real-time incident communication as well. *[INFERRED — synthesis of lead-22 and lead-46; no single primary
source was fetched specifically for real-time incident communication wording.]*
**reviewer_criterion:** Was incident status communicated in plain, factual terms (impact, current action,
known unknowns) without assigning blame to an individual, while the incident was still active?

---

## 8. Communication & mentoring — writing, teaching, decision records (ADRs)

**lead-50 (checkable).** Write an **Architecture Decision Record (ADR)** for every "architecturally
significant" decision — one that affects structure, non-functional characteristics, dependencies,
interfaces, or construction techniques. Keep ADRs as short text files, in a lightweight markup language
(Markdown or Textile), inside the project repository, **numbered sequentially and monotonically** with
numbers never reused. *[VERIFIED, cognitect.com/blog/2011/11/15/documenting-architecture-decisions.]*

**lead-51 (checkable).** Structure each ADR with exactly five parts, kept small enough that the whole
document is one or two pages: **Title** (a short noun phrase, e.g. "ADR 9: LDAP for Multitenant
Integration"); **Context** (the forces at play — technological, political, social, project-local, often in
tension — described in value-neutral, factual language); **Decision** (the response to those forces, stated
in full sentences and active voice, e.g. "We will..."); **Status** ("proposed" before stakeholders agree,
"accepted" once they do, later "deprecated" or "superseded" with a reference to its replacement); and
**Consequences** (*all* resulting effects — positive, negative, and neutral — not just the favorable ones).
*[VERIFIED, cognitect.com/blog/2011/11/15/documenting-architecture-decisions.]*

**lead-52 (checkable).** When a decision is reversed, **keep the old ADR and mark it superseded** rather than
deleting or silently editing it — "It's still relevant to know that it *was* the decision, but is *no
longer* the decision." This is the same supersede-don't-delete discipline appropriate to any durable,
audited record. *[VERIFIED, cognitect.com/blog/2011/11/15/documenting-architecture-decisions.]*

**lead-53 (checkable).** Write the ADR's Context so a future team member who wasn't in the room has a real
**third option** beyond the two dangerous defaults: **blindly accepting** a decision (fine if it's still
valid, corrosive if the context changed and nobody notices) or **blindly changing** it (fine if it needed
reversing, dangerous if it silently reintroduces the exact problem the original decision solved). A recorded
rationale is what makes "understand, then decide" possible instead. *[VERIFIED,
cognitect.com/blog/2011/11/15/documenting-architecture-decisions — "Context".]*

**lead-54 (checkable).** In review comments, ADRs, and postmortems alike, write to **explain *why*, not
restate *what***. Google's review guidance states this directly for comments — "comments are useful when
they explain why some code exists, and should not be explaining what some code is doing... if the code isn't
clear enough to explain itself, the code should be made simpler" — and it is exactly the reasoning an ADR's
Context section exists to capture (lead-53) that can't be recovered by reading the artifact alone.
*[VERIFIED, google.github.io/eng-practices/review/reviewer/looking-for.html — "Comments".]*

**lead-55 (judgment).** Treat every review comment and every piece of feedback as a **teaching opportunity**,
not only a gate: **"Code review can have an important function of teaching developers something new about a
language, a framework, or general software design principles... if your comment is purely educational, but
not critical..., prefix it with 'Nit:' or otherwise indicate that it's not mandatory."** Balance giving an
explicit instruction against simply naming the problem and letting the other engineer reach the fix
themselves — the latter is what actually builds their judgment over time. *[VERIFIED,
google.github.io/eng-practices/review/reviewer/standard.html — "Mentoring", and google.github.io/
eng-practices/review/reviewer/comments.html — "Summary".]*
**reviewer_criterion:** Is purely educational feedback clearly marked as non-mandatory (e.g. "Nit:"), and
does at least some feedback point out the problem and let the author choose the fix, rather than dictating
every fix explicitly?

**lead-56 (judgment).** Write for the reader with the **least** context, not the one with the most — a
decision record, a postmortem, or a review comment that only makes sense to someone already in the
conversation has failed at the one thing it exists to do. *[INFERRED — synthesis drawn from Nygard's ADR
rationale (lead-53) and the Scrum Guide's framing of the Definition of Done as creating "a shared
understanding"; no single primary source was fetched specifically for this "least-context reader" framing.]*
**reviewer_criterion:** Could someone with no prior context on this decision/incident/change understand it
from the written record alone, without needing to ask the author for missing background?

---

## Defaults / quick-reference table

SemVer triple: **MAJOR** = incompatible API change · **MINOR** = compatible new functionality / any
deprecation · **PATCH** = compatible bug fix only; reset lower components to 0 on a bump (lead-32) ·
Keep a Changelog types: **Added / Changed / Deprecated / Removed / Fixed / Security**, latest-first, dated,
with a live **Unreleased** section (lead-36–37) · code-review response SLA: **one business day** (lead-24) ·
comment severity vocabulary: **Nit / Optional (Consider) / FYI** (Google) or **label [+ blocking /
non-blocking / if-minor]: subject** (Conventional Comments) (lead-20–21) · postmortem triggers: **user-
visible downtime/degradation past threshold · any data loss · on-call intervention · resolution time past
threshold · monitoring-failure discovery** (lead-44) · ADR fields: **Title / Context / Decision / Status /
Consequences**, one or two pages (lead-51) · decision-speed heuristic: **~70% of desired information +
reversible ⇒ fast, delegated, "two-way door"; irreversible ⇒ slower, more careful, "one-way door"**
(lead-26–27) · technical-debt quadrant axes: **reckless ↔ prudent** crossed with **deliberate ↔
inadvertent** (lead-40).

---

## Source ledger

**Primary sources (fetched and read in full, or pulled from raw page text and directly verified, for this
guide, 2026-08-16):**

- **SemVer 2.0.0** — `semver.org/spec/v2.0.0.html`: the eleven-item Specification (items 1, 3, 4, 6, 7, 8
  quoted/cited directly) and the FAQ ("won't I end up at version 42.0.0", "How do I know when to release
  1.0.0?").
- **Keep a Changelog 1.1.0** — `keepachangelog.com/en/1.1.0/`: tagline, "Guiding Principles", "Types of
  changes", and "How can I reduce the effort required to maintain a changelog?".
- **Google eng-practices** — `google.github.io/eng-practices/review/...`: `reviewer/standard.html` ("The
  Standard of Code Review", "Resolving Conflicts", "Mentoring" — verbatim quote independently re-verified
  against raw page text); `reviewer/looking-for.html` ("Complexity", "Summary", "Every Line", "Context",
  "Comments"); `developer/small-cls.html` ("Why Write Small CLs?", "What is Small?", "Can't Make it Small
  Enough"); `reviewer/speed.html` ("How Fast Should Code Reviews Be?"); `reviewer/comments.html` ("Summary",
  "Label comment severity", "Courtesy").
- **Conventional Comments** — `conventionalcomments.org`: "Format", "Labels" (praise / nitpick / suggestion /
  issue / todo / question / thought), "Decorations" (blocking / non-blocking / if-minor).
- **Martin Fowler, "Yagni"** — `martinfowler.com/bliki/Yagni.html` (26 May 2015).
- **Martin Fowler, "Technical Debt Quadrant"** — `martinfowler.com/bliki/TechnicalDebtQuadrant.html`
  (14 Oct 2009, reposted 19 Nov 2014), citing Robert C. Martin's "A Mess is not a Technical Debt".
- **Google SRE Book, Ch. 15** — `sre.google/sre-book/postmortem-culture/`, "Postmortem Culture: Learning from
  Failure" (John Lunney & Sue Lueder) — "Google's Postmortem Philosophy".
- **Michael Nygard, "Documenting Architecture Decisions"** —
  `cognitect.com/blog/2011/11/15/documenting-architecture-decisions` (15 Nov 2011) — "Context", "Decision",
  "Status", "Consequences".
- **Jeff Bezos, 2016 Letter to Amazon Shareholders** —
  `aboutamazon.com/news/company-news/2016-letter-to-shareholders` — "High-Velocity Decision Making".
- **2020 Scrum Guide** — `scrumguides.org/scrum-guide.html` — "Commitment: Definition of Done", "Developers".

**Evidence flags carried from the research:** the exact verbatim sentence for lead-3/lead-17 ("there is no
such thing as 'perfect' code—there is only better code...") and the ADR field list for lead-51 (including the
**Status** field, easy to miss in a truncated read) were both independently re-verified by pulling the raw
page text directly, because the first-pass indexed preview had truncated mid-sentence — flagged here so the
correction is traceable. Rules marked **[INFERRED]** in full are lead-7, lead-8, lead-13, lead-14, lead-29,
lead-43, lead-48, lead-49, and lead-56 — standard practitioner convention or direct synthesis of two cited
rules, not the literal text of any single fetched page, flagged rather than presented as sourced.

*Colophon: v1, 2026-08-16. Distilled with zero shortcuts from SemVer 2.0.0, Keep a Changelog 1.1.0, Google's
eng-practices, Conventional Comments, Fowler's Yagni and Technical Debt Quadrant, the Google SRE Book, Michael
Nygard's ADR post, Jeff Bezos's 2016 shareholder letter, and the 2020 Scrum Guide, fetched and read in full;
56 rules (lead-1…lead-56); no section dropped; verified vs. inference labeling carried throughout.*
