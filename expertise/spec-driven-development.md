# Spec-Driven-Development Expertise — Plain-English Specs to Executable Tests

> **Purpose.** This is a repo-agnostic, primary-sourced practitioner guide to **spec-driven development
> (SDD) / behaviour-driven development (BDD)** — turning a plain-English requirement into a human-reviewable
> specification and, from the same artifact, an automated test: the discovery → formulation → automation
> pipeline, Gherkin's Given/When/Then structure, acceptance criteria as the definition of done,
> specification-by-example, and the anti-patterns that let a spec quietly rot. **Cucumber and Gherkin are
> used throughout as one popular, thoroughly-documented labelled EXAMPLE of the pipeline — not the
> definition of it** (§11): the same discovery/formulation/automation discipline runs under SpecFlow,
> JBehave, Behave, RSpec, or a team's own plain-English-to-test convention.
>
> **Primary sources (this file is a faithful distillation — do not contradict):** the official **Cucumber
> docs** (`cucumber.io/docs/gherkin/reference`, `cucumber.io/docs/bdd`, `.../bdd/who-does-what`,
> `.../bdd/example-mapping`) and **Dan North's "Introducing BDD"** (`dannorth.net/blog/introducing-bdd/`,
> the article that coined and first described BDD) — all fetched and read in full for this guide,
> 2026-08-16. **Gojko Adzic's *Specification by Example*** is cited only from Manning's official publisher
> page (`manning.com/books/specification-by-example`), which I fetched and read; the book's own chapter text
> was **not** independently verifiable for this guide (see the Source ledger for exactly what failed and
> why) — every claim about the book's specific patterns is labelled accordingly, never presented as a direct
> quote.
>
> **Evidence discipline.** **[VERIFIED]** = taken directly from a fetched primary-source page, cited inline
> by URL; where the guide quotes text, the quote is exact (modulo straight vs. curly quotation marks) and
> cites the exact page section. **[INFERRED]** = this guide's synthesis across two or more verified sources,
> a well-established convention not itself the literal text of a fetched page, or — explicitly flagged where
> it happens — a claim sourced from secondary aggregation because the primary text could not be fetched. No
> claim is asserted without one of these two labels; generalizing a source's claim to a new context is always
> tagged **[INFERRED]**, never passed off as directly verified.
>
> **Every actionable rule has a stable id (`sdd-N`).** The companion manifest
> `manifests/spec-driven-development.json` indexes each, typed `checkable | judgment | principle`.
>
> Status: **v1 — discovery through anti-patterns and tool-agnosticism, 45 rules (sdd-1…sdd-45). No section
> dropped. Date: 2026-08-16.**

---

## 0. Executive summary — the pipeline, then the load-bearing rules

Spec-driven development is not "write Gherkin." It is a **discipline for closing the gap between what the
business means and what the code does**, using one artifact — the plain-English, then structured, then
automated specification — as spec, documentation, and test at once. The three truths below dominate
everything else in this guide, so they lead.

**sdd-1 (principle).** **A spec-driven example is three things at once: a specification, a piece of
documentation, and a test.** Once formulated and automated, "your examples are an executable specification
of the system" — there is no separate "the real spec" hiding somewhere else that the tests merely check
against; the example IS the spec. *[VERIFIED, cucumber.io/docs/gherkin/reference — "Example".]*

**sdd-2 (principle).** **The pipeline has three practices, run in order: Discovery (what the system COULD
do) → Formulation (what it SHOULD do, written so both humans and computers can read it) → Automation (what
it ACTUALLY does, checked by running code).** Skipping discovery to jump straight to writing Gherkin is the
most common failure mode — "you won't get much joy from the other two practices until you've mastered
discovery." *[VERIFIED, cucumber.io/docs/bdd/ — "Three practices".]*

**sdd-3 (principle).** **A story's behaviour IS its acceptance criteria — not a checklist alongside it.**
"If the system fulfils all the acceptance criteria, it's behaving correctly; if it doesn't, it isn't." Dan
North's contribution was making that criteria **executable**, so "done" is a command's exit code, not an
opinion. *[VERIFIED, dannorth.net/blog/introducing-bdd/.]*

The rest of this guide is the mechanism behind those three: what SDD is and why it exists (§1), the
discovery→formulation→automation pipeline in full (§2), Gherkin's syntax as a labelled example of a
formulated spec (§3), writing Given/When/Then well (§4), acceptance criteria as the definition of done (§5),
collaboration and roles (§6), discovery workshops and Example Mapping (§7), specification by example and
living documentation (§8), keeping spec/code/tests synchronized (§9), anti-patterns (§10), and why the
discipline outlives any one tool (§11).

---

## 1. What spec-driven development is, and why

**sdd-4 (principle).** **The hardest problem in building software is deciding precisely what to build** —
Cucumber's own docs open their case for BDD with Fred Brooks's line from *The Mythical Man-Month*: "The
hardest single part of building a software system is deciding precisely what to build." SDD's primary
output is therefore not tests or docs — it is **the right conversations at the right time**; documentation
and automated tests are "nice side-effects" of a team reaching shared understanding, not the goal itself.
*[VERIFIED, cucumber.io/docs/bdd/ — "Discovery: What it could do", quoting Fred Brooks.]*

**sdd-5 (checkable).** BDD closes the business/technical gap through three concrete, named mechanisms:
**(1)** encouraging collaboration across roles to build shared understanding of the problem, **(2)** working
in rapid, small iterations to increase feedback and the flow of value, **(3)** producing system documentation
that is **automatically checked against the system's actual behaviour**. It does this "by focusing
collaborative work around concrete, real-world examples that illustrate how we want the system to behave."
*[VERIFIED, cucumber.io/docs/bdd/ — "What is BDD?".]*

**sdd-6 (principle).** **SDD/BDD is not a rival to agile — it assumes one.** It "does not replace your
existing agile process, it enhances it"; think of it as "a set of plugins" for a team already planning work
in small increments (e.g. user stories) that make the team more able to deliver "timely, reliable releases
of working software." *[VERIFIED, cucumber.io/docs/bdd/ — "BDD and agile".]*

---

## 2. The pipeline: plain-English discovery → structured formulation → automated tests

**sdd-7 (checkable).** Run the three practices **in this order, never skipped**: **Discovery** — structured
conversations ("discovery workshops," §7) around real-world examples from the user's perspective, which grow
shared understanding, surface gaps, and often reveal low-priority scope that can be deferred; **Formulation**
— once at least one valuable example is found, write it as structured documentation in a medium **both
humans and computers can read**, so the whole team can give feedback on the shared vision and the same
artifact can drive automation; **Automation** — wire the formulated examples to executable checks against
real system behaviour. *[VERIFIED, cucumber.io/docs/bdd/ — "Three practices".]*
<br>*predicate: a spec-driven change shows evidence of a discovery conversation (workshop notes, example
mapping cards, or equivalent) before a formulated scenario exists for it, and the formulated scenario exists
before automation is written against it.*

**sdd-8 (checkable).** Formulating examples **collaboratively**, in a medium readable by both humans and
computers, does double duty: the whole team gives feedback on the shared vision, **and** by writing it
together the team "establish[es] a shared language for talking about the system," which "helps us to use
problem-domain terminology all the way down into the code." A specification formulated solely by one role
after the fact forfeits this second effect. *[VERIFIED, cucumber.io/docs/bdd/ — "Formulation: What it should
do".]*

---

## 3. Gherkin: the executable-specification syntax (a labelled example, not the definition)

This section describes **Gherkin's** concrete syntax as the primary worked example of what a "formulated"
spec-driven artifact looks like (§2's middle stage). The mechanics are Gherkin-specific; the underlying
discipline — one plain-English artifact serving as spec, documentation, and test — is not (§11).

**sdd-9 (checkable).** A Gherkin document's **primary keywords** are `Feature`, `Rule` (Gherkin 6+),
`Example` (or `Scenario`), the step keywords `Given`/`When`/`Then`/`And`/`But` (or `*`), `Background`, and
`Scenario Outline` (or `Scenario Template`) with its `Examples` (or `Scenarios`) table; the **secondary
keywords** are Doc Strings (`"""`), Data Tables (`|`), Tags (`@`), and Comments (`#`). Every non-blank line
must start with one of these, except the free-form description lines under `Example`/`Background`/`Scenario
Outline`/`Rule`. *[VERIFIED, cucumber.io/docs/gherkin/reference — "Keywords".]*
<br>*predicate: the formulated spec file uses only these primary/secondary keywords to structure content;
free text appears only where the reference permits it.*

**sdd-10 (checkable).** `Feature` gives a high-level description and **groups related scenarios**; free-form
text beneath it is ignored by the runner but preserved for reporting (e.g. an HTML formatter), and a single
file holds **exactly one** `Feature`. If a Feature accumulates "lots and lots" of scenarios, that is a signal
it is actually describing more than one feature and should be split. *[VERIFIED, cucumber.io/docs/gherkin/
reference — "Feature"; scenario-count guidance from `.../who-does-what` — "Scenarios".]*
<br>*predicate: each `.feature` file (or equivalent) contains exactly one `Feature:` line, and a feature
whose scenario count has grown very large has been reviewed for a split.*

**sdd-11 (checkable).** `Rule` (added in Gherkin 6) represents **one business rule**; it groups the one-or-
more scenarios that illustrate that rule, making "this set of examples is all illustrating the SAME rule"
explicit in the document structure instead of left implicit. *[VERIFIED, cucumber.io/docs/gherkin/reference
— "Rule".]*
<br>*predicate: where two or more scenarios in a feature illustrate the same underlying business rule, they
are grouped under a shared `Rule:` block rather than left as unrelated top-level scenarios.*

**sdd-12 (checkable).** Each `Example`/`Scenario` is a **concrete example that illustrates a business rule**,
built from Given (context) / When (event) / Then (outcome) steps; the reference recommends **3-5 steps**,
because too many erodes "its expressive power as a specification and documentation." Critically: "In
addition to being a specification and documentation, an example is also a *test*. As a whole, your examples
are an *executable specification* of the system." *[VERIFIED, cucumber.io/docs/gherkin/reference —
"Example".]*
<br>*predicate: each scenario has roughly 3-5 steps and follows the Given(context)→When(event)→Then(outcome)
ordering; scenarios well outside that step count have been reviewed for a split (§10, sdd-41).*

**sdd-13 (checkable).** `Background` hoists `Given` steps that repeat across **every** scenario in a Feature
— repetition is the signal that the steps are "**incidental details**," not essential to the scenario being
read. It runs before each scenario (after any Before hooks). Guidance for using it well: keep it **short**
(rule of thumb: if it's over ~4 lines, push irrelevant detail into a higher-level step); make it **vivid**
(memorable names, not `"User A"`/`"Site 1"`); and never use it to bury **complicated state the reader
actually needs to see**. *[VERIFIED, cucumber.io/docs/gherkin/reference — "Background" and "Tips for using
Background".]*
<br>*predicate: `Background` contains only Given steps common to every scenario in the feature, stays within
roughly 4 lines, and does not hide state a reader needs to evaluate the scenario's correctness.*

**sdd-14 (checkable).** `Scenario Outline` (alias `Scenario Template`) collapses near-duplicate scenarios
into one template using `<param>`-delimited placeholders, **run once per data row** in its `Examples`/
`Scenarios` table beneath it (the header row is not itself run); placeholders may also appear in the
outline's own description and in multiline step arguments (Doc Strings/Data Tables). *[VERIFIED,
cucumber.io/docs/gherkin/reference — "Scenario Outline" and "Examples".]*
<br>*predicate: scenarios that differ only in data values are expressed as one `Scenario Outline` with an
`Examples` table, not copy-pasted per value combination.*

**sdd-15 (checkable).** `Doc Strings` (delimited by `"""` on their own lines, or — in modern Cucumber — by
triple backticks) pass a larger block of text as a step's last argument, with indentation dedented relative
to the opening delimiter; `Data Tables` (`|`-delimited) pass structured tabular data to a step, escaping a
literal `|` as `\|`, a newline as `\n`, and a backslash as `\\` inside a cell. *[VERIFIED, cucumber.io/docs/
gherkin/reference — "Doc Strings", "Data Tables", "Table Cell Escaping".]*
<br>*predicate: multi-line text passed to a step uses a Doc String, and structured/tabular step data uses a
Data Table with correctly escaped `|`, `\n`, `\\` inside cells — not ad-hoc inline formatting.*

**sdd-16 (checkable).** A Gherkin file's spoken language must match **the language your users and domain
experts actually use when they talk about the domain** — "translating between two languages should be
avoided." Gherkin ships localized keyword sets for 70+ spoken languages, selected per-file with a leading
`# language: <code>` comment (default: English, `en`); some implementations also allow a project-wide
default so the header need not repeat in every file. *[VERIFIED, cucumber.io/docs/gherkin/reference —
"Spoken Languages".]*
<br>*predicate: a spec's keyword language matches the language domain experts use, declared via `# language:`
when it is not the tool's default.*

**sdd-17 (checkable).** `@Tag`s group Features/Scenarios (e.g. for selective execution) **independently of
file/directory layout** — they are placed above the element they tag. Comments (`#`) are free text ignored
by the parser, with one reserved exception: a `# language:` header as the very first line of a file sets
that file's spoken language (sdd-16). *[VERIFIED, cucumber.io/docs/gherkin/reference — "Feature" (tags note)
and "Spoken Languages" (`# language` header).]*
<br>*predicate: cross-cutting grouping (e.g. "run only smoke scenarios") is expressed via tags, not via
directory naming conventions that the runner has no notion of.*

---

## 4. Writing Given/When/Then well

**sdd-18 (checkable).** `Given` steps describe the **initial context** — "the scene of the scenario,"
typically something that happened in the **past** — and put the system in a **well-defined state** (e.g.
creating/configuring objects, seeding test data). Avoid describing user interaction in a `Given`; if you were
writing use cases, `Given`s are your preconditions. Multiple givens are fine (chain with `And`/`But`).
*[VERIFIED, cucumber.io/docs/gherkin/reference — "Given".]*
<br>*predicate: `Given` steps set up state only (no simulated user action) and read as something already
true before the scenario's event occurs.*

**sdd-19 (checkable).** `When` steps describe **one event or action** — a person interacting with the system,
or an event triggered by another system. Apply the "**imagine it's 1922**" test: could a person do this
without a computer? If a step reads like a UI mechanic instead of a domain action, it has smuggled
implementation/technology assumptions into the spec — those belong in the step definition, never the Gherkin
text. *[VERIFIED, cucumber.io/docs/gherkin/reference — "When".]*
<br>*predicate: each `When` step names exactly one domain-level action free of UI/technology detail (e.g.
"Withdraw money," not "click the withdraw button and enter 20 in the amount field").*

**sdd-20 (checkable).** `Then` steps assert an **expected, observable outcome**: the step definition should
use an assertion comparing the actual outcome to the expected one, and the outcome "should be on an
**observable** output" — something that comes *out* of the system (report, UI, message) — "and not a
behaviour deeply buried inside the system (like a record in a database)." The reference explicitly warns:
"While it might be tempting to implement `Then` steps to look in the database — resist that temptation!"
*[VERIFIED, cucumber.io/docs/gherkin/reference — "Then".]*
<br>*predicate: `Then` steps assert on a user/external-system-observable output, never directly on internal
storage state (§10, sdd-38).*

**sdd-21 (checkable — the mechanism behind "unambiguous").** `And`/`But` are pure readability sugar for
consecutive same-kind steps — Cucumber's matcher **ignores the leading keyword entirely** when resolving a
step to its definition. This means a `Given` and a `Then` with byte-identical text are treated as **the same
step** — a duplicate. This "might seem like a limitation, but it forces you to come up with a less ambiguous,
more clear domain language": e.g. `Given there is money in my account` / `Then there is money in my account`
collide, while `Given my account has a balance of £430` / `Then my account should have a balance of £430` do
not. *[VERIFIED, cucumber.io/docs/gherkin/reference — "Steps" and "And, But".]*
<br>*predicate: no two steps of different intent share identical wording once the leading Given/When/Then/
And/But keyword is stripped; a collision is resolved by making the domain language more precise, not by
special-casing the automation layer.*

**sdd-22 (judgment).** Treat "can this be phrased as a Given(context)/When(event)/Then(observable-outcome)
triple" as the **operational test for whether prose is actually a specification yet** (not merely informally
"clear"). Prose with no discernible prior context, no single event, or a `Then` that isn't phrasable as an
assertion on an observable (sdd-20) is untestable by construction — it needs another discovery/formulation
pass (§2), not a more strongly-worded comment.
<br>*reviewer_criterion: for any requirement claimed as "specified," can a reviewer point to its Given
(context), When (event), and Then (observable assertion) — and if not, has it correctly been treated as not
yet spec-ready rather than shipped as prose?*

---

## 5. Acceptance criteria as the definition of done

**sdd-23 (checkable — the founding insight, quoted).** "A story's behaviour is simply its acceptance
criteria: if the system fulfils all the acceptance criteria, it's behaving correctly; if it doesn't, it
isn't." This is why, in SDD, acceptance criteria are not a checklist run *against* the definition of done —
they **are** the definition of done. *[VERIFIED, dannorth.net/blog/introducing-bdd/ — "BDD provides a
'ubiquitous language' for analysis".]*
<br>*predicate: a story/change is declared done only when every one of its stated acceptance-criteria
scenarios passes — not on a separate, informally-judged "looks right" basis.*

**sdd-24 (checkable).** The story template **"As a [X], I want [Y], so that [Z]"** (X = the beneficiary role,
Y = the feature, Z = its value/benefit) forces the value to be named at definition time: "its strength is
that it forces you to identify the value of delivering a story when you first define it." When a story
degrades to "... I want [some feature] so that [I just do, ok?]," that is a legible signal to descope it.
*[VERIFIED, dannorth.net/blog/introducing-bdd/ — "BDD provides a 'ubiquitous language' for analysis".]*
<br>*predicate: every accepted story states an X/Y/Z triple, and a story whose "so that" is vacuous or
unnamed is flagged for descoping rather than accepted as-is.*

**sdd-25 (checkable — the origin of Given/When/Then).** Dan North's acceptance-criteria scenario template —
**"Given some initial context (the givens), When an event occurs, Then ensure some outcomes"** — was
deliberately built "loose enough that it wouldn't feel artificial or constraining to analysts but structured
enough that we could break the story into its constituent fragments and automate them." This is the
documented origin of the Given/When/Then structure Gherkin later standardized (§3). *[VERIFIED,
dannorth.net/blog/introducing-bdd/ — "BDD provides a 'ubiquitous language' for analysis".]*

**sdd-26 (checkable).** "**Acceptance criteria should be executable**": the given/event/outcome fragments are
"fine-grained enough to be represented directly in code." North's original JBehave mapped each Given to its
own class, one class for the Event, and classes for the Outcomes, all wired against a shared "world" object
that the givens populate and the event acts on — making the fragments individually **reusable** across
scenarios and stories, not just readable. *[VERIFIED, dannorth.net/blog/introducing-bdd/ — "Acceptance
criteria should be executable".]*
<br>*predicate: automated acceptance criteria are composed of reusable, independently-testable
context/event/outcome fragments, not one monolithic script per scenario.*

---

## 6. Collaboration & roles — who writes the spec

**sdd-27 (checkable).** "Who should be writing Gherkin documents... The answer depends on several factors,
such as team structure, skills, culture, process and more" — there is no single universal answer. A durable
default the docs give: while the team's language/style is still being established, **the whole team**
collaborates on writing it; later it can be done efficiently by **a pair** (a developer/automation-owner and
a tester/quality-owner), as long as their output is **actively reviewed by the product owner** (or business
representative). *[VERIFIED, cucumber.io/docs/bdd/who-does-what/ — "Writing Gherkin".]*
<br>*predicate: no scenario is formulated by a single role in isolation without product-owner/business-rep
review before it is treated as agreed.*

**sdd-28 (checkable).** The **"Three Amigos"** meeting turns a user story into clean, thorough Gherkin
scenarios by bringing at least three voices: **the product owner** (scope — deciding what's in/out as edge
cases surface), **the tester**, and **the developer** (implementation detail — what will actually execute,
what roadblocks exist). "It is essential that all of these roles have conversations to discover examples
*together*" because each amigo sees the product from a different perspective. It is **not** a one-time,
exactly-three-person, project-kickoff-only ritual — hold it repeatedly, with whoever the conversation needs,
to continually refine features. *[VERIFIED, cucumber.io/docs/bdd/who-does-what/ — "The Three Amigos".]*
<br>*predicate: a nontrivial new scenario set has evidence of at least a product/business, a
quality/testing, and a development perspective having been consulted — not authored by one role alone.*

**sdd-29 (judgment).** BDD's ubiquitous-language move — Dan North and Chris Matts recognized they were
"trying to define a ubiquitous language for *the analysis process itself*," borrowing the term from Eric
Evans's *Domain-Driven Design* (modelling a system so business vocabulary "permeates right into the
codebase") — means the **same words** a domain expert uses in conversation should appear in the Given/When/
Then text AND in the code. *[VERIFIED, dannorth.net/blog/introducing-bdd/ — "BDD provides a 'ubiquitous
language' for analysis".]*
<br>*reviewer_criterion: can a domain expert with no engineering background read the scenario's nouns and
verbs and recognize their own vocabulary — or does it lean on internal/technical terms the business side
doesn't use?*

---

## 7. Discovery: workshops and Example Mapping

**sdd-30 (checkable).** A **discovery workshop** is a structured conversation, held *before* formulation
(§2), centered on real-world examples of the system from the user's perspective; it grows the team's shared
understanding of user needs, governing rules, and scope, and it may reveal gaps needing more information.
"The scrutiny of a discovery session often reveals low-priority functionality that can be deferred from the
scope of a user story" — a direct mechanism for working in smaller increments. *[VERIFIED, cucumber.io/docs/
bdd/ — "Discovery: What it could do".]*

**sdd-31 (checkable).** **Example Mapping** runs the discovery conversation on four card colours arranged as
a map: **yellow** = the story, on top; **blue** = each acceptance-criterion/rule beneath it; **green** =
concrete examples illustrating a rule, under that rule; **red** = a question that can't be answered in the
session (or an assumption made), captured so the group can keep moving instead of stalling. Continue "until
the group is satisfied that the scope of the story is clear, or we run out of time." *[VERIFIED, cucumber.io/
docs/bdd/example-mapping/ — "How it works".]*
<br>*predicate: a formulated scenario traces back to a blue rule card that traces back to a yellow story card
from an actual mapping/discovery session — not a scenario invented without that traceable chain (§10, sdd-43).*

---

## 8. Specification by example: living documentation

**sdd-32 (checkable).** Gojko Adzic's *Specification by Example* distills interviews/case-studies from
successful delivery teams into **seven collaborative patterns** ("Seven patterns, fully explored in this
book, are key to making the method effective"). Per the method's own publisher description, it has **four
main benefits**: "it produces living, reliable documentation; it defines expectations clearly and makes
validation efficient; it reduces rework; and, above all, it assures delivery teams and business stakeholders
that the software that's built is right for its purpose." *[VERIFIED, manning.com/books/
specification-by-example.]*

**sdd-33 (principle — disclosed limitation, read before citing a specific pattern).** This guide could
**not** independently verify the seven patterns' individual names/mechanics against the book's own text: the
Manning-hosted sample chapter is a **scanned-image PDF with no extractable text** (confirmed by attempting
extraction — only embedded-image and XMP-metadata binary streams came back, no prose), and the book's former
companion site `specificationbyexample.com` **no longer resolves** (DNS failure). Independent secondary
summaries of the book converge on the same seven names — *deriving scope from goals, specifying
collaboratively, illustrating using examples, refining the specification, automating validation without
changing specifications, validating frequently, evolving a documentation system* — but this list is
**[INFERRED]** from secondary aggregation, not a verbatim quote from a fetched primary page; verify against
the book directly before treating any one pattern's detailed mechanics as settled fact.

**sdd-34 (checkable).** **"Living documentation"** is the convergence point of specification-by-example and
BDD's own stated goal: documentation "**automatically checked against the system's behaviour**." Unlike a
wiki page or a design doc, a living-documentation scenario cannot silently drift out of date — if the system
stops matching it, the scenario **fails** (a red test) rather than quietly becoming wrong. *[VERIFIED,
cucumber.io/docs/bdd/ — "What is BDD?"; corroborated by manning.com/books/specification-by-example's "living,
reliable documentation" framing.]*
<br>*predicate: the formulated specs are wired to automation such that a behaviour change with no matching
spec update produces a failing (red) run — not merely an unreviewed diff.*

---

## 9. Keeping spec, code, and tests synchronized — drift is the enemy

**sdd-35 (checkable).** Because automation (§2) ties the formulated spec **directly** to live system
behaviour, an out-of-date scenario doesn't rot silently the way a comment or a wiki page does — it goes
**red**. This is the concrete mechanism that makes "keep the spec current" enforceable rather than aspirational:
the same executable-specification property that makes an example a test (sdd-1, sdd-12) is what makes drift
detectable at all. *[VERIFIED, cucumber.io/docs/bdd/ — "Producing system documentation that is automatically
checked against the system's behaviour" (§0 mechanism), applied to the drift question.]*
<br>*predicate: CI (or an equivalent gate) runs the formulated specs on every change, so a spec/behaviour
mismatch is caught as a failing run, not discovered later by inspection.*

**sdd-36 (judgment).** When behaviour intentionally changes, update the **scenario** as part of the same
change — not the code first with the scenario patched up afterward as an afterthought. Because the scenario
is plain-English and business-readable (§3), a deliberate behaviour change surfaces in review as a **visible,
readable scenario edit**, giving the business side the same visibility into "what changed" that the code
diff gives engineers; a scenario edited only to make a stale test pass, with no accompanying explanation of
why the behaviour changed, is a smell.
<br>*reviewer_criterion: for a change that alters observable behaviour, does the reviewable diff include the
scenario edit alongside the code edit, in a form a non-engineer reviewer could read and understand what
changed and why?*

**sdd-37 (judgment).** Collaborative formulation (§2 sdd-8, §6 sdd-27) is itself a synchronization mechanism,
not just a quality-of-first-draft one: a spec authored by one role in isolation, after the code already
exists, tends toward restating the implementation (§10, sdd-44) rather than the business intent — and a spec
that restates the code can never usefully diverge from it, so it stops being able to *catch* drift between
intent and implementation even while technically staying "in sync."
<br>*reviewer_criterion: was this scenario set formulated collaboratively (§6) before or alongside the
implementation, rather than derived from the finished code by a single author afterward?*

---

## 10. Anti-patterns

**sdd-38 (checkable — gotcha).** A `Then` step that queries the database (or other internal storage) instead
of asserting an observable output is an explicit, named anti-pattern: "resist that temptation." It looks
precise but binds the spec to an **implementation detail** (the schema) rather than behaviour, so a pure
refactor of the storage layer — no behaviour change at all — breaks specs that were never describing behaviour
in the first place. *[VERIFIED, cucumber.io/docs/gherkin/reference — "Then".]* (Mechanism: sdd-20.)

**sdd-39 (checkable — gotcha).** A `When` step written as a UI script ("click the blue button," "fill in the
Name field then the Address field") rather than a domain action ("Withdraw money") fails the **"imagine it's
1922"** test and smuggles technology/implementation assumptions into the plain-English layer, where they
don't belong (they belong in the step definition). The reference gives the concrete fix for a step that
already reads as two actions joined by "and": split it into two `When`/`And` steps, one action each.
*[VERIFIED, cucumber.io/docs/gherkin/reference — "When" and "Scenarios" (who-does-what) example of splitting
a multi-field step.]* (Mechanism: sdd-19.)

**sdd-40 (checkable — gotcha).** Two steps whose wording collides once the leading keyword is stripped (§4,
sdd-21) — most often a `Given` and a `Then` that describe "the same fact" in different tenses, e.g. "there is
money in my account" used both ways — are a **vague-domain-language** anti-pattern, not a tooling problem;
the fix is more precise, business-specific phrasing ("my account has a balance of £430"), never a workaround
bolted onto the automation layer to force two different meanings out of identical text. *[VERIFIED,
cucumber.io/docs/gherkin/reference — "Steps".]*

**sdd-41 (judgment — scenario bloat).** A scenario padded well past the recommended 3-5 steps (sdd-12) has
usually stopped being *one example of one rule* and started narrating a procedure — split it into multiple
scenarios, or extract genuinely incidental setup into `Background` (sdd-13). A scenario is only useful as
living documentation if a reader can hold the whole thing in mind at once; that property is exactly what
excess steps destroy.
<br>*reviewer_criterion: does this scenario stay close to 3-5 steps and illustrate one rule — and if not, has
it been reviewed for a split or a `Background` extraction rather than left to grow?*

**sdd-42 (checkable — gotcha).** Using `Background` to set up **complicated state the reader actually needs
to evaluate the scenario** is a misuse of the keyword — the explicit guidance is "don't use Background to set
up complicated states, unless that state is actually something the client needs to know," preferring a
higher-level step ("Given I am logged in as a site owner") that hides irrelevant detail instead. A
`Background` that has "scrolled off the screen" defeats its own purpose: the reader loses the overview it was
meant to provide. *[VERIFIED, cucumber.io/docs/gherkin/reference — "Tips for using Background".]* (Rule:
sdd-13.)

**sdd-43 (judgment — gold-plating).** Example Mapping's own discipline (§7, sdd-31) is the countermeasure to
gold-plating: any scenario that cannot be traced back to a blue rule card that itself traces back to the
yellow story card the group actually discussed does not belong in the spec yet. Anything that can't answer
"which rule, from which story, does this illustrate?" is speculative scope — capture it on a red question
card (or its equivalent) instead of formulating it prematurely.
<br>*reviewer_criterion: can every scenario in this change be traced to a discussed rule and story — or does
at least one exist only because it seemed like a reasonable thing to also support?*

**sdd-44 (judgment — specs that restate the code).** A spec whose steps are really the implementation
reworded (e.g. "the `save()` method returns true," "the `UserRepository` throws `NotFoundException`") fails
both the ubiquitous-language test (§6, sdd-29) and the "imagine it's 1922" test (§4, sdd-19/§10 sdd-39) at
once. The operational check: if a domain expert with no engineering background cannot read the scenario and
confirm it's right, it is not a specification — it is a disguised implementation note wearing Given/When/
Then formatting.
<br>*reviewer_criterion: strip the scenario of Gherkin formatting — does the remaining prose read as a
business rule a domain expert could confirm, or as a description of a function/class/method?*

---

## 11. Repo-agnostic: the discipline outlives any one tool

**sdd-45 (principle).** BDD predates and outlives Cucumber. Dan North built the **first** implementation,
**JBehave**, specifically to give the Given/When/Then acceptance-criteria template an executable "story
runner"; the same discipline subsequently produced **RSpec** in the Ruby community (via Dave Astels's
promotion of BDD techniques) and North's own planned **rbehave**. The concrete keyword syntax in §3
(Gherkin/Cucumber) is one thoroughly-documented instance of "plain-English discovery → structured, readable
formulation → automated check" (§2) — the pipeline and its acceptance-criteria discipline (§5) are what
transfers to SpecFlow, Behave, JBehave, or a team's own convention; the exact keyword grammar does not have
to. *[VERIFIED, dannorth.net/blog/introducing-bdd/ — "The present and future of BDD".]*

---

## Defaults / quick-reference table

Pipeline order: **Discovery → Formulation → Automation** (sdd-2, sdd-7), never skipped, never reordered ·
example size: **3-5 steps** per scenario (sdd-12) · scenario shape: **Given (context, past) → When (one
event) → Then (observable outcome)** (sdd-18–20) · step-matching: **by text only, keyword-blind** — identical
wording across `Given`/`When`/`Then` is one step, a collision to resolve with more precise language (sdd-21,
sdd-40) · story template: **As a [X], I want [Y], so that [Z]** (sdd-24) · acceptance-criteria template:
**Given some initial context, When an event occurs, Then ensure some outcomes** (sdd-25) · roles: whole team
early, developer+tester pair later, **always reviewed by the product owner** (sdd-27) · discovery cards:
**yellow = story, blue = rule, green = example, red = question** (sdd-31) · drift defense: automation makes
an out-of-sync spec **fail red**, not rot silently (sdd-35).

---

## Source ledger

**Primary sources (fetched and read in full for this guide, 2026-08-16):**

- Cucumber official docs (`cucumber.io/docs/...`): `/gherkin/reference` (Keywords: Feature, Rule, Example,
  Steps [Given/When/Then/And/But], Background + tips, Scenario Outline, Examples, Doc Strings, Data Tables +
  escaping, Spoken Languages); `/bdd/` (What is BDD?, BDD and agile, Rapid iterations, Three practices —
  Discovery/Formulation/Automation); `/bdd/who-does-what/` (Scenarios, The Three Amigos, Writing Gherkin);
  `/bdd/example-mapping/` (How it works).
- Dan North, **"Introducing BDD"** (`dannorth.net/blog/introducing-bdd/`; the article's own canonical URL
  redirects there from `dannorth.net/introducing-bdd/`; first published in *Better Software* magazine, March
  2006) — the primary source for BDD's origin, the ubiquitous-language rationale, the As-a/I-want/so-that and
  Given/When/Then templates, "acceptance criteria should be executable" and the JBehave object-model example,
  and BDD's spread to RSpec/rbehave.
- Manning Publications' official book page for Gojko Adzic, *Specification by Example* (`manning.com/books/
  specification-by-example`; published 2011, ISBN 978-1617290084) — publisher's "about the technology"/"team"
  copy, used for the seven-patterns claim and the four stated benefits.

**Evidence flags carried from the research (read before relying on §8):**

- **sdd-32** is a direct, exact quote from Manning's official book-page copy — **[VERIFIED]**.
- **sdd-33** discloses a genuine research dead-end, not a shortcut taken silently: the Manning sample-chapter
  PDF (`manning-content.s3.amazonaws.com/.../adzic_ch01.pdf`, linked from the book page) was fetched but is a
  **scanned-image PDF** — extraction returned only embedded-image binary streams and Adobe XMP metadata, zero
  extractable prose; the book's former companion site `specificationbyexample.com` returned a **DNS
  resolution failure** (domain no longer live) on the date of research. The seven pattern *names* given in
  sdd-33 are corroborated across independent secondary summaries but are explicitly **[INFERRED]** from
  secondary aggregation, not verified against the book's primary text.
- No other rule in this guide relies on an unreachable source; every rule outside §8 cites Cucumber's docs or
  Dan North's article directly.

*Colophon: v1, 2026-08-16. Distilled from the official Cucumber documentation, Dan North's "Introducing BDD,"
and Manning's official Specification by Example book page, fetched and read for this guide; 45 rules
(sdd-1…sdd-45); no section dropped; verified vs. inferred labelling — including one disclosed source
dead-end — carried throughout.*
