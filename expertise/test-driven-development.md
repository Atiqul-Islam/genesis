# Test-Driven-Development Expertise — The Discipline, Language-Agnostic

> **Purpose.** This is a repo-agnostic, primary-sourced practitioner guide to **test-driven development
> (TDD)** and the testing craft around it — the red-green-refactor cycle, Kent Beck's three laws, test-first
> as a design technique, the test pyramid, test doubles (dummy/fake/stub/spy/mock), arrange-act-assert,
> one-assertion-per-behavior, coverage as a tool, property-based and snapshot testing, test determinism, and
> regression-test discipline. Every rule applies to any codebase, in any language, using any xUnit-style or
> property-based test runner; where a concrete illustration helps (§12's snapshot-matcher syntax, §11's
> Hypothesis seed flag), the guide names at most one framework as a *labelled example* per rule and states
> explicitly that the underlying technique is not specific to it.
>
> **Primary sources (this file is a faithful distillation — do not contradict):** Martin Fowler's testing
> articles and bliki entries (`martinfowler.com`), Gerard Meszaros's **xUnit Test Patterns** reference site
> (`xunitpatterns.com`), Robert C. Martin's **Three Laws of TDD** — the verbatim wording is his original
> statement of the rule, `butunclebob.com/ArticleS.UncleBob.TheThreeRulesOfTdd` ("The Three Laws of TDD"),
> distinct from his later reworded retelling of the same three rules in *The Cycles of TDD*
> (`blog.cleancoder.com`, itself citing `97 Things Every Programmer Should Know`) — Kent Beck's own **Test
> Desiderata** essay, the **Hypothesis** property-based testing docs (including its seed/replay mechanism),
> and snapshot/golden-testing tooling verified across ecosystems (Jest for JS as this guide's one labelled
> example; `insta.rs` for Rust and `approvaltests.com` as a multi-language family, both fetched and read to
> confirm the technique is not Jest-specific) — all fetched and read in full for this guide, 2026-08-16.
> Wikipedia's Test-Driven Development article is used only where it directly quotes Kent Beck or cites
> *Test-Driven Development by Example* / peer-reviewed meta-analyses, and is flagged as a secondary source
> each time.
>
> **Evidence discipline.** **[VERIFIED]** = taken directly from a fetched primary- or reputable
> practitioner-source page, cited inline by URL. **[INFERRED]** = this guide's synthesis across two or more
> verified sources, or a well-established convention not itself the literal text of a fetched page. No claim
> is asserted without one of these two labels; a generalization of a source's claim to a new context is
> always tagged **[INFERRED]**, never passed off as directly verified.
>
> **Every actionable rule has a stable id (`tdd-N`).** The companion manifest
> `manifests/test-driven-development.json` indexes each, typed `checkable | judgment | principle`.
>
> Status: **v1.1 — red-green-refactor through refactor-under-green, 71 rules (tdd-1…tdd-71). No section
> dropped. Date: 2026-08-16 (review-fix pass applied same date).**

---

## 0. Executive summary — the discipline, then the load-bearing rules

TDD is not "write tests." It is a **discipline of sequencing**: a tiny, disciplined loop that forces
correctness and design to be handled one at a time instead of simultaneously, backed by a suite that only
means something if it can be trusted completely. The three truths below dominate everything else in this
guide, so they lead.

**tdd-1 (principle).** **Never write production code without a failing test that demands it.** The
mechanism is the **red-green-refactor cycle**: write a test for the next bit of behavior, write just enough
code to pass it, then refactor — repeated in a tight loop, driven from a test list written up front.
*[VERIFIED, martinfowler.com/bliki/TestDrivenDevelopment.html.]*

**tdd-2 (principle).** **Test-first is a design technique, not verification bolted on afterward.** Writing
the test before the code forces you to consider the desired behavior and interface before implementation —
this tends to produce smaller units, looser coupling, and clearer interfaces, especially when tests are
written against public behavior rather than implementation details. *[VERIFIED, Wikipedia
Test-driven_development "Potential benefits", citing Beck (2003) — secondary source summarizing Beck's own
claim.]*

**tdd-3 (principle).** **Determinism is non-negotiable — a suite that lies sometimes is worse than no suite.**
A test that passes sometimes and fails sometimes "without any noticeable change in the code, tests, or
environment" destroys the value of the whole regression suite, because people stop trusting any of it, not
just the flaky test. *[VERIFIED, martinfowler.com/articles/nonDeterminism.html.]*

The rest of this guide is the mechanism behind those three: the red-green-refactor cycle in full (§1), Kent
Beck's three laws — TDD's nano-cycle (§2), test-first as design (§3), arrange-act-assert / the four-phase
test (§4), one assertion per behavior (§5), the test pyramid (§6), test doubles (§7), classical vs. mockist
TDD (§8), determinism (§9), coverage as a tool (§10), property-based testing (§11), snapshot/golden testing
(§12), regression tests (§13), keeping the suite fast and pristine (§14), and refactoring safely under green
(§15).

---

## 1. The Red-Green-Refactor cycle

**tdd-4 (checkable).** The macro loop has exactly three steps, repeated: **(1)** write a test for the next
bit of functionality you want to add; **(2)** write the functional code until that test passes; **(3)**
refactor both new and old code to make it well structured. This is commonly summarized as
**Red → Green → Refactor**. *[VERIFIED, martinfowler.com/bliki/TestDrivenDevelopment.html.]*

**tdd-5 (checkable).** Before the loop starts, write out a **list of test cases** you expect to need. Pick
one, run red-green-refactor on it, then pick the next — adding new items to the list as they occur to you
along the way. Sequencing this list well (picking tests that drive quickly to the salient design points) is
itself a skill. *[VERIFIED, martinfowler.com/bliki/TestDrivenDevelopment.html.]*

**tdd-6 (principle).** TDD operates at **three nested cycle granularities**: a *macro-cycle* (the test list,
tackled test-by-test), a *milli-cycle* — the minute-by-minute Red/Green/Refactor loop, run once per complete
unit test — and a *nano-cycle*, the second-by-second Three Laws (§2), iterated roughly a dozen times inside
one milli-cycle. *[VERIFIED, blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html — "Minute-by-Minute:
micro-cycle: Red-Green-Refactor" and "Second-by-Second nano-cycle".]*

**tdd-7 (checkable).** The philosophy behind Red-Green-Refactor: a limited mind cannot pursue **correct
behavior** and **correct structure** at the same time, so get behavior right first (green), and only then —
and only then — restructure. Robert C. Martin traces this to what he calls "Kent Beck's original injunction":
**"Make it work. Make it right. Make it fast."** *[VERIFIED, blog.cleancoder.com/uncle-bob/2014/12/17/
TheCyclesOfTDD.html — the quote is Martin's citation of Beck, not Martin's own coinage.]*

---

## 2. Kent Beck's Three Laws of TDD — the nano-cycle

**tdd-8 (checkable).** The **Three Laws of TDD**, verbatim, from Robert C. Martin's original statement of the
rule (titled, in his own words, "The Three Laws of TDD"):
1. "You are not allowed to write any production code unless it is to make a failing unit test pass."
2. "You are not allowed to write any more of a unit test than is sufficient to fail; and compilation failures
   are failures."
3. "You are not allowed to write any more production code than is sufficient to pass the one failing unit
   test."

*[VERIFIED, butunclebob.com/ArticleS.UncleBob.TheThreeRulesOfTdd — "The Three Laws of TDD.", quoted exactly.]*

Martin later retold these same three rules, reworded rather than requoted, in *The Cycles of TDD*: "You must
write a failing test before you write any production code... You must not write more of a test than is
sufficient to fail, or fail to compile... You must not write more production code than is sufficient to make
the currently failing test pass." Treat this second wording as a **paraphrase of the same rule**, not a second
independent verbatim source — the guide previously conflated the two. *[VERIFIED,
blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html, citing
programmer.97things.oreilly.com/wiki/index.php/The_Three_Laws_of_Test-Driven_Development.]*

**tdd-9 (principle).** Attribute precisely: the Three Laws were **codified by Robert C. Martin** as a
distillation of the fine-grained discipline he experienced pair-programming with Kent Beck — they are Martin's
formulation of the practice, not a verbatim quotation from Beck's own book. Do not cite them as Beck's own
words. *[VERIFIED, blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html — "the fine grained structure
... I experienced while working with Kent so long ago".]*

**tdd-10 (checkable).** The Three Laws force **line-by-line granularity**: you iterate them roughly a dozen
times to produce one complete unit test. Breaking law 2 (writing a large, elaborate test before it even
compiles) or law 3 (writing more production code than the current failing test demands) is a violation of TDD
discipline even if the end result is the same code — the *sequencing* is the point, not just the destination.
*[VERIFIED, blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html.]*

---

## 3. Test-first as design, not verification-after-the-fact

**tdd-11 (checkable).** Because the test is written before the code, the developer must consider the desired
**behavior and interface** first. This tends to produce smaller units of code, looser coupling, and clearer
interfaces — especially when the tests exercise public behavior rather than implementation detail.
*[VERIFIED, Wikipedia Test-driven_development "Potential benefits", citing Beck (2003) — secondary source.]*

**tdd-12 (checkable).** Keeping the unit under test small has two claimed, concrete benefits: **reduced
debugging effort** (a failure localizes to a smaller piece of code) and **self-documenting tests** (small test
cases are easier to read and understand). *[VERIFIED, Wikipedia Test-driven_development "Keep the unit small",
citing Pathfinder Solutions — secondary source.]*

**tdd-13 (judgment).** **"Fake it till you make it"** — Kent Beck's own strategy from *Test-Driven Development
by Example* for getting from red to green fast: return a literal, hard-coded value first, then generalize
under the next test that forces you to. This is a legitimate, deliberate TDD move, not a shortcut to be
embarrassed about — it keeps each nano-cycle (§2) small. *[VERIFIED, Wikipedia Test-driven_development
"Development style", naming the principle from Beck's book — secondary source for attribution.]*

**tdd-14 (checkable).** TDD's rapid red-green-refactor cycle relies primarily on **unit tests that avoid
process boundaries, network connections, or external dependencies** so they execute quickly; where the code
under development genuinely depends on something external, TDD encourages substituting a **test double**
(§7) to keep the unit test fast and isolated. *[VERIFIED, Wikipedia Test-driven_development "Test isolation" —
secondary source.]*

**tdd-15 (checkable).** The by-product of doing this well is **self-testing code**: comprehensive automated
tests written alongside the functional software such that a single command executes them all, and passing
gives real confidence that hidden bugs would have been illuminated. You can also reach self-testing code by
writing tests after the code — what matters is that the tests exist and pass, not strictly how you got there;
TDD is simply a very effective way to arrive at it. *[VERIFIED, martinfowler.com/bliki/SelfTestingCode.html.]*

---

## 4. Arrange-Act-Assert and the four-phase test

**tdd-16 (checkable).** Structure every test in **four phases, executed in sequence**: **fixture setup**
(establish the prior state the SUT needs), **exercise SUT** (actually invoke the behavior under test),
**result verification** (check the expected outcome), **fixture teardown** (housekeeping — restore the world).
Clearly separating the four phases makes a test's intent far easier to see. *[VERIFIED,
xunitpatterns.com/Four%20Phase%20Test.html — "How It Works" and "Why We Do This".]*

**tdd-17 (checkable).** A common shorthand for the first three phases is the **"Arrange, Act, Assert"**
mnemonic: set up the test data, call the method under test, assert the expected results. Fowler attributes the
mnemonic to Bill Wake's originating article "3A – Arrange, Act, Assert" (`xp123.com`). A BDD-flavored
alternative is the **given / when / then** triad, where *given* is the setup, *when* is the call, and *then*
is the assertion. Both keep tests short, consistent, and easy to read regardless of test level. *[VERIFIED,
martinfowler.com/articles/practical-test-pyramid.html — "Test Structure", which links the mnemonic directly to
xp123.com/articles/3a-arrange-act-assert/ as Wake's article; xp123.com itself returned a fetch-tool error in
this session (DNS-handling bug in the fetch sandbox, not a dead page), so the Wake/xp123.com attribution is
verified via Fowler's citation of it, not a direct read of Wake's page.]*

**tdd-18 (judgment).** Do not let setup or teardown logic obscure the exercise/verify lines that actually
carry the test's meaning — from a "tests as documentation" standpoint, the housekeeping phases are
deliberately the least important ones for a reader trying to understand *what behavior is being verified*.
*[VERIFIED, xunitpatterns.com/Four%20Phase%20Test.html — "Why We Do This", citing the "Tests as Documentation"
goal.]*

**tdd-19 (checkable).** Because assertion calls can raise exceptions, wrap the exercise+verify block in a
`try`/`finally` (or your language's equivalent) so **fixture teardown still runs even when an assertion
fails** — otherwise one failing assertion silently leaks state into the next test (a lack-of-isolation bug,
§9). *[VERIFIED, xunitpatterns.com/Four%20Phase%20Test.html — "Example: Four Phase Test (Inline)".]*

---

## 5. One assertion per behavior

**tdd-20 (checkable).** **Assertion Roulette** is a named test smell: "it is hard to tell which of several
assertions within the same test method caused a test failure." A test with many assertions and no
distinguishing failure message forces a reader to re-run and single-step just to find out what actually broke.
*[VERIFIED, xunitpatterns.com/Assertion%20Roulette.html.]*

**tdd-21 (checkable).** The **Eager Test** symptom that causes it: a test exercises several methods of the
system under test, or calls the same method several times, interleaved with fixture setup and assertions for
each — effectively several tests welded into one, so a failure anywhere in the middle is ambiguous about which
of the earlier behaviors it belongs to. *[VERIFIED, xunitpatterns.com/Assertion%20Roulette.html — "Cause: Eager
Test".]*

**tdd-22 (judgment).** The rule of thumb this implies: **verify one behavior per test.** Several assertions
that check different facets of a *single* logical outcome (e.g., three fields of one returned object) are
fine — they still fail together, for the same reason. What to avoid is exercising *multiple distinct SUT
behaviors* in one test method, because that is what makes a failure ambiguous about its cause. *[INFERRED,
synthesizing xunitpatterns.com's Assertion Roulette and Eager Test entries — the site does not itself state
this rule-of-thumb in these words.]*

---

## 6. The test pyramid

**tdd-23 (checkable).** The **test pyramid** (Mike Cohn, *Succeeding with Agile*) is a way of thinking about
how different kinds of automated tests should be combined into a balanced portfolio: **many more low-level
unit tests than high-level, broad-stack tests** driven through a GUI. Cohn's original three layers, bottom to
top, are **Unit Tests → Service Tests → User Interface Tests**. *[VERIFIED,
martinfowler.com/bliki/TestPyramid.html and martinfowler.com/articles/practical-test-pyramid.html — "The Test
Pyramid".]*

**tdd-24 (checkable).** Even critics of the exact shape agree on **two takeaways** from Cohn's pyramid: write
tests at **different granularities**, and **write fewer tests as you climb to higher-level, more
integrated levels**. Stated as a habit: write *lots* of small, fast unit tests; *some* more coarse-grained
tests; *very few* high-level tests. *[VERIFIED, martinfowler.com/articles/practical-test-pyramid.html — "The
Test Pyramid".]*

**tdd-25 (checkable).** The **ice-cream-cone anti-pattern** is the inversion: an approach built mainly around
record-and-playback UI automation looks easy at first (no programming knowledge needed to record it), but such
UI-driven tests are slow, often license-locked to particular machines, hard to run "headless," and — layered
on top of that — **more prone to non-determinism** (§9), which further undermines trust in them even when
written with good practice. *[VERIFIED, martinfowler.com/bliki/TestPyramid.html.]*

**tdd-26 (checkable).** Distinguish **solitary unit tests** (every collaborator replaced with a mock/stub, for
perfect isolation) from **sociable unit tests** (collaborators left real unless they're slow or have big side
effects) — terms coined by Jay Fields. Neither is universally correct; using both, situationally, is normal
practice. *[VERIFIED, martinfowler.com/articles/practical-test-pyramid.html — "Sociable and Solitary", citing
Jay Fields' *Working Effectively with Unit Tests*.]*

**tdd-27 (judgment).** Testing terminology is **genuinely contested** — "integration test" means a broad,
whole-system activity to some and a narrow, one-external-dependency-at-a-time activity to others; "component
test," "service test," and "broad-stack test" all overlap differently by team. Don't get hung up on winning
the label debate; agree on team-consistent terms and be explicit about what each level covers, since the
underlying reality is more of a spectrum than discrete buckets anyway. *[VERIFIED,
martinfowler.com/articles/practical-test-pyramid.html — "The Confusion About Testing Terminology".]*

**tdd-28 (checkable).** **End-to-end tests give the highest confidence that the software works, and are also
the most fragile**: they are "notoriously flaky and often fail for unexpected and unforeseeable reasons,"
driven by browser quirks, timing issues, animations, and unexpected popup dialogs — the more sophisticated the
UI, the flakier they tend to get. *[VERIFIED, martinfowler.com/articles/practical-test-pyramid.html —
"End-to-End Tests".]*

**tdd-29 (checkable).** Fowler's Test Pyramid bliki explicitly points readers, in its own "Further Reading"
section, to the **Google Testing Blog's** "Just Say No to More End-to-End Tests" (Mike Wacker, 2015) as
further support for the pyramid's shape. That cross-reference itself — title, author, and Fowler's one-line
summary — is what this guide directly verified on Fowler's page; the Google post's own extended body text was
not independently fetched and re-verified in this session, so it is not presented as a directly-verified
argument here. *[VERIFIED, martinfowler.com/bliki/TestPyramid.html — "Further Reading": "The Google Testing
Blog expands on why you shouldn't rely on end-to-end tests," linking to
testing.googleblog.com/2015/04/just-say-no-to-more-end-to-end-tests.html by Mike Wacker.]*

The stronger characterization sometimes attached to this reference — that fewer, cheaper, faster low-level
tests catch more, sooner, more reliably than a large end-to-end layer can — is this guide's own synthesis of
Fowler's stated rationale *elsewhere* in his testing corpus (Fast Feedback as a deployment-pipeline value, and
"push your tests as far down the test pyramid as you can" from the duplication-avoidance rule of thumb), not a
verified quotation of the Google Testing Blog post's own argument. *[INFERRED, synthesizing
martinfowler.com/articles/practical-test-pyramid.html — "Putting Tests Into Your Deployment Pipeline" and
"Avoid Test Duplication" — explicitly not the Google post's own verified body text.]*

---

## 7. Test doubles — dummy, fake, stub, spy, mock

**tdd-30 (checkable).** **Test Double** is the generic umbrella term (from Gerard Meszaros's *xUnit Test
Patterns*) for any kind of pretend object substituted for a real one during testing — the name is drawn from
the film industry's "stunt double." The specific vocabulary — dummy, fake, stub, spy, mock — is Meszaros's,
adopted because it distinguishes real, useful differences rather than being interchangeable jargon.
*[VERIFIED, martinfowler.com/articles/mocksArentStubs.html — "The Difference Between Mocks and Stubs".]*

**tdd-31 (checkable).** **Dummy Object** — passed only because a method signature requires a parameter that
neither the test nor the SUT actually cares about (as simple as `null` or a bare object instance). A dummy
should **never actually be used** by the receiver, so it needs no real implementation at all.
*[VERIFIED, xunitpatterns.com/Test%20Double.html "Variation: Dummy Object" and xunitpatterns.com/Dummy%20
Object.html.]*

**tdd-32 (checkable).** **Fake Object** — a **working**, but much simpler/lighter implementation than the real
thing it replaces (e.g., an in-memory store standing in for a database), unsuitable for production but
functionally real enough to let the SUT run against it. Unlike a stub or mock, a fake needs no per-test
"canned response" configuration — you install it and let the SUT use it as if it were real.
*[VERIFIED, xunitpatterns.com/Fake%20Object.html and xunitpatterns.com/Test%20Double.html "Variation:
Unconfigurable Test Doubles".]*

**tdd-33 (checkable).** **Test Stub** — a **control point** for the SUT's *indirect inputs*: it returns
pre-programmed ("canned") answers so you can drive the SUT through behaviors that would otherwise be hard to
trigger, or to get past a call to unavailable external software. A stub is the right choice when you need to
control inputs but have **no need to verify indirect outputs**. *[VERIFIED, xunitpatterns.com/Test%20
Stub.html — "When To Use It".]*

**tdd-34 (checkable).** **Test Spy** — an **observation point**: like a stub, it may still supply canned
responses, but it additionally **captures the SUT's indirect outputs** as they occur, for the test to inspect
and verify *afterward*. In effect, a spy is "just a" stub with recording capability, used for the same
underlying purpose as a mock but read and written more like a stub-based test (state-style verification after
the fact). *[VERIFIED, xunitpatterns.com/Test%20Double.html "Variation: Test Spy" and xunitpatterns.com/Test%20
Spy.html.]*

**tdd-35 (checkable).** **Mock Object** — replaces a dependency with a test-specific object whose
**expectations are declared before the SUT is exercised**, and which itself verifies — during or immediately
after the exercise — that it was called correctly. This is the mechanism for **behavior verification**: the
mock, not a later assertion, is what determines pass/fail for the interaction. *[VERIFIED,
xunitpatterns.com/Mock%20Object.html and martinfowler.com/articles/mocksArentStubs.html "Tests with Mock
Objects" worked example.]*

**tdd-36 (checkable).** The real dividing line across all five kinds is **state verification vs. behavior
verification**: a stub-based test checks the **final state** of the SUT after exercising it (extra query
methods may be added to the stub to help); a mock-based test checks that the **right calls happened**, verified
by the double itself. Mock objects always use behavior verification; a stub-style double *can* be extended to
do behavior verification too — Meszaros calls that variant a **Test Spy**. *[VERIFIED,
martinfowler.com/articles/mocksArentStubs.html — "The Difference Between Mocks and Stubs".]*

**tdd-37 (judgment).** Pick the double by what the test actually needs to know: use a **dummy** when a
parameter is structurally required but semantically irrelevant; a **fake** when you need the SUT to interact
with something real-feeling but too heavy/unavailable for the test environment; a **stub** when you need to
control indirect inputs and don't care how the SUT called it; a **spy** or **mock** when the interaction
itself — not just the SUT's resulting state — is the thing under test. *[INFERRED, synthesized across
xunitpatterns.com's Test Double / Dummy Object / Fake Object / Test Stub / Test Spy / Mock Object pages.]*

---

## 8. Classical vs. mockist TDD

**tdd-38 (checkable).** **Classical TDD**: use real collaborator objects wherever practical, and reach for a
test double only when using the real thing would be awkward (slow, non-deterministic, hard to construct).
The specific kind of double matters less to a classicist than simply avoiding the awkward dependency.
*[VERIFIED, martinfowler.com/articles/mocksArentStubs.html — "Classical and Mockist Testing".]*

**tdd-39 (checkable).** **Mockist TDD**: always substitute a mock for **any** collaborator with interesting
behavior, real or awkward, because the interaction itself is treated as part of the specification. **Behavior
Driven Development (BDD)** is an important offshoot of the mockist style, renaming tests as "behaviors" to
foreground TDD's role as a design technique. *[VERIFIED, martinfowler.com/articles/mocksArentStubs.html —
"Classical and Mockist Testing".]*

**tdd-40 (checkable).** Mockist tests are **more coupled to implementation**: they check the SUT's *outbound
calls*, so changing how a method talks to its collaborators — even with identical externally-observed
behavior — tends to break the test. A classic test only cares about the SUT's final state, not how it was
derived, so it survives more refactoring untouched. *[VERIFIED,
martinfowler.com/articles/mocksArentStubs.html — "Coupling Tests to Implementations".]*

**tdd-41 (judgment).** There is **no universally correct answer**; Fowler himself, a self-described
"old-fashioned classic TDDer," presents both fairly rather than declaring mockist testing wrong. A concrete
signal for trying mockist testing: you're losing time debugging tests that fail without clearly indicating
where or why — mockist-style expectations tend to fail closer to the actual point of the bug.
*[VERIFIED, martinfowler.com/articles/mocksArentStubs.html — "So should I be a classicist or a mockist?".]*

---

## 9. Determinism — isolation, seeding, no shared state, no network at test time

**tdd-42 (checkable).** Kent Beck's **Test Desiderata** names the load-bearing properties directly:
**Isolated** — "tests should return the same results regardless of the order in which they are run";
**Composable** — "if tests are isolated, then I can run 1 or 10 or 100 or 1,000,000 and get the same results";
**Deterministic** — "if nothing changes, the test result shouldn't change"; **Automated** — "tests should run
without human intervention." *[VERIFIED, Kent Beck, "Test Desiderata," medium.com/@kentbeck_7670/
test-desiderata-94150638a4b3.]*

**tdd-43 (checkable).** A test is **non-deterministic** exactly when "it passes sometimes and fails sometimes,
without any noticeable change in the code, tests, or environment" — failures for such a test are seemingly
random, and that randomness is what corrodes trust in the entire suite, not just that one test.
*[VERIFIED, martinfowler.com/articles/nonDeterminism.html.]*

**tdd-44 (checkable).** The **first response** to a non-deterministic test is to **quarantine** it into a
separate suite immediately, so it stops eroding confidence in the healthy suite — but "fix quarantined tests
quickly": bound quarantine with a hard limit (e.g., a fixed count of tests, or a time limit such as "no longer
than a week") so it cannot silently become a graveyard for known-broken coverage.
*[VERIFIED, martinfowler.com/articles/nonDeterminism.html — "Quarantine".]*

**tdd-45 (checkable).** **Lack of isolation** is the most common, and most frustrating, cause of
non-determinism — one test leaving data behind (e.g., in a database) can corrupt a later test that assumes a
clean starting state. **Prefer rebuilding starting state from scratch** over relying on each test to clean up
after itself: a rebuild failure localizes cleanly to the offending test, whereas a cleanup failure makes one
test buggy but a *different* test fail, which is much harder to trace. *[VERIFIED,
martinfowler.com/articles/nonDeterminism.html — "Lack of Isolation".]*

**tdd-46 (checkable).** **Always wrap the system clock** behind a seam that tests can substitute — a "clock
stub" set to and frozen at a particular time — so time-dependent behavior becomes controllable and
repeatable instead of "a new result every call." *[VERIFIED, martinfowler.com/articles/nonDeterminism.html —
"Time".]*

**tdd-47 (checkable).** **Remote/network calls at test time are a leading non-determinism source** — the
remote system may have no test instance, or an unstable one. Replace it with a **test double** that mimics
the remote system's behavior under your control; then guard against the double drifting out of sync with
reality by running **contract tests** — the same interaction test run against both the fake and the real
service — to keep the double honest. *[VERIFIED, martinfowler.com/articles/nonDeterminism.html — "Remote
Services", and martinfowler.com/articles/practical-test-pyramid.html — "Integration With Separate Services".]*

**tdd-48 (checkable).** **Resource leaks** cause *other, unrelated* tests to fail intermittently, since it's
essentially arbitrary which test happens to be the one that pushes a leaking resource pool over its limit. A
good tactic: configure the pool to size **1** and make it throw when exhausted — so the *first* test that
requests a resource after the leak fails, loudly and locally, instead of some innocent later test.
*[VERIFIED, martinfowler.com/articles/nonDeterminism.html — "Resource Leaks".]*

**tdd-49 (checkable).** For **asynchronous behavior**, don't guess with fixed sleeps: expose an explicit hook
that the test can use to detect when the async operation has actually completed (the same hook a UI spinner
might use to know when to stop), and always pair any wait with a **timeout**, since the expected event might
simply never arrive. *[VERIFIED, martinfowler.com/articles/nonDeterminism.html — footnotes on async
completion hooks and timeouts.]*

**tdd-71 (checkable).** **Randomized and property-based tests must record — and be able to replay — the seed
that produced a failure.** A failure surfaced by generated/random input is only as trustworthy as the suite's
other guarantees (tdd-3, tdd-43) if it is deterministically reproducible rather than a one-off you can never
see again; a randomized test that fails, is re-run, and passes without anyone capturing what input caused the
failure is a non-determinism problem wearing a different hat. Property-based frameworks provide this directly:
Hypothesis (this rule's labelled example) prints the failing seed and lets you pin it with `@seed(...)` or
pytest's `--hypothesis-seed`, and separately auto-saves failing examples to a local database so they replay on
their own via `@example` / `@reproduce_failure` (see §11) — the same expectation (print/record the failing
seed or example, provide a replay mechanism) generalizes to any other property-based or fuzz-style test
runner, not just Hypothesis. *[VERIFIED, hypothesis.readthedocs.io/en/latest/reference/api.html —
`hypothesis.seed(seed)`: "Seed the randomness for this test... for a fixed seed value Hypothesis will produce
the same test cases... If using pytest, you can alternatively pass --hypothesis-seed on the command line";
VERIFIED, hypothesis.readthedocs.io/en/latest/tutorial/replaying-failures.html — "When a test fails,
Hypothesis automatically saves the failure so it can be replayed later," via its `ExampleDatabase`, `@example`,
and `@reproduce_failure`.]*

---

## 10. Coverage as a tool, not a target

**tdd-50 (checkable).** Test coverage is "a useful tool for finding untested parts of a codebase" — and "of
little use as a numeric statement of how good your tests are." These are two different claims; conflating
them is the mistake. *[VERIFIED, martinfowler.com/bliki/TestCoverage.html.]*

**tdd-51 (checkable).** Brian Marick's distinction, quoted approvingly by Fowler: **"I expect a high level of
coverage. Sometimes managers require one. There's a subtle difference."** — a coverage number you notice is
diagnostic information; a coverage number you're required to hit is a quota, and quotas get gamed.
*[VERIFIED, martinfowler.com/bliki/TestCoverage.html, quoting Brian Marick.]*

**tdd-52 (checkable).** Making coverage a hard **target** invites low-quality tests that only exist to move
the number — Fowler names this failure mode "AssertionFreeTesting": tests that execute code (raising coverage)
without ever actually checking anything. High coverage is trivially reachable with such tests while telling
you nothing about correctness. *[VERIFIED, martinfowler.com/bliki/TestCoverage.html.]*

---

## 11. Property-based testing

**tdd-53 (checkable).** **Property-based testing**: instead of hand-picking example inputs, write a test that
should hold for **all inputs in a described range/strategy**, and let the tool **randomly generate which of
those inputs to actually check — including edge cases you might not have thought of yourself.**
*[VERIFIED, hypothesis.readthedocs.io/en/latest/index.html.]*

**tdd-54 (checkable).** Property-based testing is explicitly **"a powerful addition to unit testing. It is not
always a replacement."** Good candidate properties: **round-trip** behavior (encode/decode,
serialize/deserialize); **equivalence** between an optimized implementation and a slower-but-obviously-correct
reference; **invariants** that must always hold (e.g., "a sequence of transactions always balances — money
never gets lost"); and **no-crash-on-valid-input** for parsers, linters, and compilers.
*[VERIFIED, hypothesis.readthedocs.io/en/latest/tutorial/introduction.html — "When to use Hypothesis and
property-based testing".]*

**tdd-55 (checkable).** When a property-based run finds a counterexample, it reports the **actual failing
input**, which is often already small and pointed (e.g., a documented Hypothesis run against a sort function
over mixed integers/floats reported the counterexample `[1.0, nan, 0]`, immediately exposing that sorting in
the presence of `nan` is ill-defined) — a concrete input the developer can reason about directly, not just
"some case failed." *[VERIFIED, hypothesis.readthedocs.io/en/latest/tutorial/introduction.html — "Preventing
floats() from generating nan".]*

**tdd-56 (judgment).** Property-based testing frameworks (Hypothesis included) are generally understood to
**shrink** a randomly-found failing case down toward the smallest/simplest input that still reproduces the
failure, before reporting it — this is what makes counterexamples like `[1.0, nan, 0]` land as small,
readable inputs rather than large random noise. **[INFERRED]** — this is the framework's well-known general
behavior; the specific fetched pages in this session illustrate a small counterexample but do not themselves
spell out the shrinking algorithm in the text retrieved.

---

## 12. Snapshot and golden testing

**tdd-57 (checkable).** **Snapshot testing** (also called **golden-file** or **approval** testing) captures a
serialized rendering of some output (a component tree, a data structure, a rendered document) the first time
a test runs, storing it as a **snapshot/golden file**; on every later run, the current output is compared
against that stored file and any difference fails the test. This is a general technique implemented across
many ecosystems under compatible names, not a Jest-only idea: **Jest** for JavaScript/TypeScript
(`jestjs.io/docs/snapshot-testing` — used below as this guide's *one labelled example*), **insta** for Rust
(`insta.rs`), pytest snapshot/syntax-tree plugins for Python, and **ApprovalTests** — a multi-language family
(Java, .NET, C++, Python, Ruby, Go, and more) built on exactly this idea. *[VERIFIED, jestjs.io/docs/
snapshot-testing — "a very useful tool whenever you want to make sure your UI [or other output] does not
change unexpectedly"; VERIFIED, insta.rs — "Snapshots tests (also sometimes called approval tests) are tests
that assert values against a reference value (the snapshot)"; VERIFIED, approvaltests.com — "Approvaltests is
in many languages," listing Java/C#/C++/PHP/Python/Swift/JS/Perl/Go/Lua/Objective-C/Ruby/LabVIEW/Dart/Elixir
implementations.]*

**tdd-58 (checkable).** Treat the snapshot/golden artifact as code: **commit it alongside the change that
produced it, and review it as part of code review.** Good snapshot tooling deliberately renders the stored
value in a human-readable format so a reviewer can read the diff, not just trust that "the test still
passes" — Jest's version of this is `pretty-format` (this rule's labelled example); the same
human-readable-diff discipline is not Jest-specific — insta, for instance, independently documents rendering
"beautiful snapshot diffs right in your terminal." *[VERIFIED, jestjs.io/docs/snapshot-testing, as the
labelled example; VERIFIED, insta.rs — "Pretty Diffs: insta renders beautiful snapshot diffs right in your
terminal," confirming the discipline generalizes beyond Jest.]*

**tdd-59 (checkable).** Any **non-deterministic field** in the snapshotted value (generated ids, timestamps)
will fail the snapshot on every run unless normalized — snapshot tooling generally provides some form of
placeholder/redaction mechanism so a matcher, not the literal generated value, is what gets checked and
stored. Jest's version is an asymmetric property matcher (e.g. `expect.any(Date)`, this rule's labelled
example); insta calls the equivalent mechanism a **redaction**. *[VERIFIED, jestjs.io/docs/snapshot-testing —
"Property Matchers", as the labelled example; VERIFIED, insta.rs — "Redactions: if you have output which can
change between test runs (such as random identifiers, timestamps or others) you can instruct insta to redact
these parts," confirming the same normalization need recurs outside Jest.]*

**tdd-60 (judgment).** A snapshot test verifies **"did the output change,"** not **"is the output correct."**
Because tooling commonly offers an interactive mode to step through and accept/reject each failed snapshot
(Jest's Interactive Snapshot Mode is this rule's labelled example; insta's `cargo-insta` review flow is the
equivalent in Rust), it is easy to rubber-stamp every diff without reading it — which defeats the entire
purpose. A human must judge each diff at the moment it's created, not merely at the moment the snapshot was
first captured. *[INFERRED, drawn from Jest's and insta's own documented review workflows (commit +
code-review + interactive per-snapshot accept/reject) — the docs describe the mechanism; the discipline
required to use it correctly is this guide's synthesis.]*

---

## 13. Regression tests — every bug earns one

**tdd-61 (checkable).** The standard reaction to a production bug in a team practicing self-testing code:
**first write a test that exposes the bug, and only then try to fix it.** Writing that test may itself be a
series of tests that progressively narrow the scope down to a single unit test that reliably triggers the
bug. *[VERIFIED, martinfowler.com/bliki/SelfTestingCode.html.]*

**tdd-62 (checkable).** **"Any bug isn't just a failure in the code, it's equally a failure in the testing
screen."** Writing the reproducing test first is not only a debugging technique — it is what guarantees the
bug **stays fixed**, since a later regression will now be caught automatically instead of silently
reintroduced. *[VERIFIED, martinfowler.com/bliki/SelfTestingCode.html, verbatim.]*

**tdd-63 (judgment).** Use a fixed bug as a **trigger to audit for sibling untested cases nearby** — a team
practicing this well treats one discovered bug as "inspiration to look for similar missing tests," not just an
isolated data point to patch and move past. *[VERIFIED, martinfowler.com/bliki/SelfTestingCode.html.]*

---

## 14. Keeping the suite fast and pristine

**tdd-64 (checkable).** An uncontrolled non-deterministic test doesn't just cost the one flaky result — it
degrades the whole suite's signal, until "people don't pay much attention to whether \[tests\] pass or fail."
A suite nobody trusts has already failed at its job, whatever its pass/fail count says.
*[VERIFIED, martinfowler.com/articles/nonDeterminism.html.]*

**tdd-65 (checkable).** Kent Beck's Test Desiderata names both the speed and the confidence properties
directly: **Fast** — "tests should run quickly"; **Inspiring** — "passing the tests should inspire
confidence." A suite that is slow enough to be skipped, or unreliable enough to be ignored, satisfies neither.
*[VERIFIED, Kent Beck, "Test Desiderata," medium.com/@kentbeck_7670/test-desiderata-94150638a4b3.]*

**tdd-66 (judgment).** Generalize "pristine" beyond flakiness to **any accumulating source of noise** a team
learns to tune out — stale skipped tests, ignored warnings, tests nobody remembers the purpose of. Anything a
developer has learned to routinely disregard in the test output has, functionally, stopped being part of the
suite's signal even while it's still nominally "passing." **[INFERRED]** — a generalization of Fowler's
quarantine/trust argument (tdd-44, tdd-64) beyond the specific non-determinism case it was made for.

**tdd-67 (checkable).** Quarantine (tdd-44) is a tool for **preserving trust in the healthy suite while you
fix the broken part** — it is not a place to let a test live forever. Bound it explicitly (a count limit,
e.g. 8 tests, or a time limit, e.g. one week) so hitting the bound forces the cleanup work rather than letting
quarantine quietly grow. *[VERIFIED, martinfowler.com/articles/nonDeterminism.html — "Quarantine".]*

---

## 15. Refactor safely, only under green

**tdd-68 (checkable).** Refactoring is a **named, bounded step of the cycle** — "refactor both new and old
code to make it well structured" — not an ongoing background activity you do whenever you feel like it
mid-implementation. It happens after green, as its own deliberate pass. *[VERIFIED,
martinfowler.com/bliki/TestDrivenDevelopment.html.]*

**tdd-69 (checkable).** The reason refactoring is sequenced strictly *after* green: correct behavior and
correct structure are two goals a limited mind cannot optimize simultaneously, so the discipline is to nail
behavior first (with the safety net of a passing test), and only then restructure with that safety net intact.
*[VERIFIED, blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html.]*

**tdd-70 (judgment).** **Never expand behavior during a refactor step.** If you notice new behavior is needed
while refactoring, that observation belongs on the test list (tdd-5) as the seed of the *next* red-green
cycle — not folded into the refactor you're already mid-way through. Refactoring that quietly adds behavior
loses the safety property the whole cycle is built on: a refactor is only provably safe if the tests it runs
under didn't change what they were checking. **[INFERRED]** — the direct corollary of tdd-68/tdd-69's
behavior-then-structure sequencing.

---

## Defaults / quick-reference table

Cycle nesting: **macro** (test list) → **milli** (Red-Green-Refactor, per test) → **nano** (Three Laws, per
line, ~dozen iterations per milli-cycle) (tdd-6) · Three Laws = write failing test → don't over-write the test
→ don't over-write the code (tdd-8) · Four-Phase Test = setup → exercise → verify → teardown, teardown in
`finally` (tdd-16, tdd-19) · AAA = Arrange/Act/Assert ≡ Given/When/Then (tdd-17) · test doubles: **dummy**
(never used) · **fake** (working, lighter) · **stub** (canned inputs, no output check) · **spy** (records
outputs, verified after) · **mock** (expectations set before exercise, verifies itself) (tdd-31–tdd-35) ·
state verification (classical/stub) vs. behavior verification (mockist/mock) (tdd-36) · pyramid shape = many
unit → some service/integration → few end-to-end/UI (tdd-23–tdd-24) · anti-shape = ice-cream cone (tdd-25) ·
dedup thresholds n/a — determinism gates: isolate (rebuild > cleanup), stub the clock, double the network +
contract-test the double, pool-limit-1 for leak detection, hook + timeout for async (tdd-45–tdd-49) · seeded
randomness — record/print the failing seed or example, replay it, never just re-run until green (tdd-71) ·
quarantine bound = count or time limit, never unbounded (tdd-44, tdd-67) · coverage = diagnostic tool, never a
pass/fail gate number (tdd-50–tdd-52) · property-based testing = addition not replacement; round-trip /
equivalence / invariant / no-crash properties (tdd-54) · snapshot = commit + human-reviewed diff + normalize
generated fields (tdd-58–tdd-59) · every bug → reproducing test first, then fix, then audit siblings
(tdd-61–tdd-63) · refactor only after green, never mid-refactor behavior growth (tdd-68–tdd-70).

---

## Source ledger

**Primary sources (fetched and read in full for this guide, 2026-08-16):**

- Martin Fowler, `martinfowler.com`: *Test Pyramid* (bliki); *The Practical Test Pyramid* (article, Ham
  Vocke); *Mocks Aren't Stubs* (article); *Eradicating Non-Determinism in Tests* (article); *Test Coverage*
  (bliki); *Test Driven Development* (bliki); *Self Testing Code* (bliki).
- Gerard Meszaros, *xUnit Test Patterns*, hosted at `xunitpatterns.com`: *Test Double*; *Test Double
  Patterns*; *Four-Phase Test*; *Assertion Roulette*; *Dummy Object*; *Fake Object*; *Test Stub*; *Mock
  Object*; *Test Spy*.
- Robert C. Martin, *The Three Laws of TDD*, `butunclebob.com/ArticleS.UncleBob.TheThreeRulesOfTdd` — the
  verbatim source of tdd-8's quoted rule text.
- Robert C. Martin, *The Cycles of TDD*, `blog.cleancoder.com/uncle-bob/2014/12/17/TheCyclesOfTDD.html` — his
  later, reworded retelling of the same three rules (used for tdd-6/7/9/10/69, not treated as a second
  verbatim source of the rule text itself); citing *97 Things Every Programmer Should Know*
  (`programmer.97things.oreilly.com`).
- Kent Beck, *Test Desiderata*, `medium.com/@kentbeck_7670/test-desiderata-94150638a4b3`.
- Hypothesis documentation, `hypothesis.readthedocs.io/en/latest/` — index page, *Introduction to Hypothesis*
  tutorial page, the API Reference (`reference/api.html`, for `hypothesis.seed`/`--hypothesis-seed`), and
  *Replaying failed tests* (`tutorial/replaying-failures.html`, for the `ExampleDatabase`/`@example`/
  `@reproduce_failure` mechanism) — all read for tdd-53–tdd-56 and the new tdd-71.
- Jest documentation, `jestjs.io/docs/snapshot-testing` — used as §12's one labelled framework example.
- insta documentation, `insta.rs`, and ApprovalTests, `approvaltests.com` — fetched and read to confirm
  snapshot/golden/approval testing is a cross-ecosystem technique (Rust and a 15-language family,
  respectively), not Jest-specific; used to de-Jest-lock §12.
- Google Testing Blog, *Just Say No to More End-to-End Tests* (Mike Wacker, 2015-04-22,
  `testing.googleblog.com`) — cited only via Fowler's own summary of its argument (title, author, and thrust
  verified directly on the page; the article's own extended body text was not independently re-verified
  beyond that in this session — re-confirmed on this review-fix pass, where the fetched page returned only
  the post's header/comments, not its body).

**Secondary source used carefully, always flagged:** Wikipedia, *Test-driven development*
(`en.wikipedia.org/wiki/Test-driven_development`) — used only for passages that themselves directly quote Kent
Beck (the "rediscovery" quote) or cite *Test-Driven Development by Example* (Beck, 2002) and named empirical
studies; every rule sourced from it says "Wikipedia" and names what it is citing, never presented as if
independently verified from Beck's book text.

**Evidence flags carried in this guide:** tdd-8's Three Laws quote is sourced verbatim from
`butunclebob.com/ArticleS.UncleBob.TheThreeRulesOfTdd`, not from the reworded `blog.cleancoder.com` retelling
that an earlier pass of this guide mislabeled as the verbatim text; tdd-9 corrects a common misattribution (the
Three Laws are Martin's formulation of Beck's practice, not Beck's own words) rather than silently repeating
the popular shorthand; tdd-29 is split into a directly-verified clause (Fowler's own cross-reference to the
Google Testing Blog post) and a separately-flagged [INFERRED] clause (the "fewer, cheaper, faster" practical
characterization, synthesized from Fowler's own stated rationale elsewhere, not the Google post's body); tdd-56
and tdd-60 are explicitly labeled [INFERRED] rather than presented as directly quoted, because the specific
pages fetched this session illustrate but do not spell out those claims in the retrieved text; tdd-71 (seeded/
replayable randomness) is fully [VERIFIED] against current Hypothesis docs.

*Colophon: v1.1, 2026-08-16. Distilled with zero shortcuts from Fowler's testing corpus, Meszaros's xUnit Test
Patterns, Robert C. Martin's Three Laws (verbatim from butunclebob.com), Kent Beck's Test Desiderata, the
Hypothesis practitioner docs (including seed/replay), and snapshot-testing docs verified across Jest, insta,
and ApprovalTests, fetched and read in full; 71 rules (tdd-1…tdd-71); no section dropped; verified vs.
inference labeling carried throughout. This v1.1 pass corrected a mislabeled paraphrase (tdd-8), re-scoped an
oversold citation (tdd-29), de-Jest-locked snapshot testing (§12), added seeded-determinism coverage (tdd-71),
and sourced the AAA mnemonic's originator (tdd-17).*
