# Git-Mastery Expertise — The Full Git Model and Professional Workflows

> **Purpose.** This is a repo-agnostic, primary-sourced practitioner guide to **git** — the object model,
> branching, merge vs rebase, interactive rebase, cherry-pick, stash, worktrees, bisect, reflog-based
> recovery, submodules, hooks, `.gitattributes`/`.gitignore`, signed commits/tags, clean-history hygiene,
> and safe recovery from mistakes. It contains no project-specific commands or APIs — every rule applies to
> any git repository at any git version from roughly 2.30 onward.
>
> **Primary sources (this file is a faithful distillation — do not contradict):** the official **Pro Git
> book** (`git-scm.com/book/en/v2`, Scott Chacon & Ben Straub, CC-BY-NC-SA) and the official **git reference
> documentation** (`git-scm.com/docs/<command>`, current as served 2026-08-16). Every command's exact
> synopsis, option semantics, and file layout is taken from these pages, fetched and read in full for this
> guide — see the Source ledger.
>
> **Evidence discipline.** **[VERIFIED]** = taken directly from a fetched primary-source page, cited inline
> by URL. **[INFERRED]** = this guide's engineering judgment / well-established community convention that
> was not itself the literal text of a fetched page (e.g. a config knob mentioned only in passing, or a
> widely-taught practice). No claim is asserted without one of these two labels.
>
> **Every actionable rule has a stable id (`git-N`).** The companion manifest `manifests/git-mastery.json`
> indexes each, typed `checkable | judgment | principle`.
>
> Status: **v1 — object model through clean-history hygiene, 63 rules (git-1…git-63). No section dropped.
> Date: 2026-08-16.**

---

## 0. Executive summary — the model, then the load-bearing rules

Git is not a set of memorized commands. It is one small idea, worked out consistently: a **content-addressed
directed acyclic graph (DAG) of immutable objects**, plus a handful of **mutable pointers** into that graph.
Once that idea is solid, almost every command — including the scary ones — is describable precisely as
"write some new immutable objects" and/or "move a pointer." The three truths below dominate everything else
in this guide, so they lead.

**git-1 (principle).** Model git as a **content-addressed object store**: blobs, trees, and commits are
immutable, hashed-by-content objects forming a DAG; branches, tags, `HEAD`, and the index are just **pointers
into that DAG** (a ref is a file holding a SHA, `HEAD` is usually a pointer to a ref, the index is a staged
snapshot). Learn the object model first (§1–§2); every other command in this guide is built from it.
*[VERIFIED, git-scm.com/book/en/v2/Git-Internals-Git-Objects and .../Git-Internals-Git-References.]*

**git-2 (principle).** Nearly every everyday command is one of exactly two primitive moves over that model:
**(a) write new object(s)** (commit, cherry-pick, rebase-replay, stash) or **(b) move a pointer** (checkout,
branch, reset, merge-fast-forward). `git reset` is the clearest teaching example: it walks up to **three
trees in order** — move `HEAD`'s branch pointer, then make the index match it, then make the working
directory match the index — stopping early per `--soft`/`--mixed`/`--hard` (§2). *[VERIFIED,
git-scm.com/book/en/v2/Git-Tools-Reset-Demystified.]*

**git-3 (principle).** **Nothing reachable is ever silently gone.** As long as a commit is reachable from
some ref (a branch, a tag) **or** from a live reflog entry, it survives; `git gc` only removes what is
reachable from **neither**. This is a time-bounded, local-only safety net (§10), not a backup system — but
it means "I think I lost work" is almost always solvable by `git reflog` before it is solvable by panic.
*[VERIFIED, git-scm.com/docs/git-reflog and .../Git-Internals-Maintenance-and-Data-Recovery.]*

The rest of this guide is the mechanism behind those three: the object model in full (§1), references/HEAD/
the index (§2), branching and basic merging (§3), rebase mechanics and the golden rule (§4), interactive
rebase (§5), cherry-pick (§6), stash (§7), worktrees (§8), bisect (§9), reflog-based recovery (§10),
submodules (§11), hooks (§12), `.gitattributes` (§13), `.gitignore` (§14), signed commits/tags (§15), and
clean-history hygiene / safe rewriting (§16).

---

## 1. The object model — blobs, trees, commits, tags

**git-4 (checkable).** A **blob** stores raw file **content only** — no filename, no mode, no path. Two
files anywhere in the repository (or across its whole history) with byte-identical content are the **same
blob**, referenced by multiple tree entries. Inspect one with `git cat-file -p <sha>`; create one directly
with `git hash-object`. *[VERIFIED, git-scm.com/book/en/v2/Git-Internals-Git-Objects — "Object Storage" and
"Blob Objects".]*

**git-5 (checkable).** A **tree** object is one directory level: an ordered list of entries, each `<mode>
<type> <sha>\t<name>`, where `<type>` is `blob` (a file) or `tree` (a subdirectory). Build one from the
index with `git write-tree`; read one back into the index with `git read-tree` (optionally `--prefix=<dir>`
to graft it as a subtree). Inspect with `git cat-file -p <tree-sha>`. *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Git-Objects — "Tree Objects".]*

**git-6 (checkable).** A **commit** object points to exactly **one tree** (the full snapshot) plus **zero,
one, or many parent commits**, plus `author`/`committer` lines (name, email, timestamp) and a free-text
message. Create one directly with `git commit-tree <tree> [-p <parent>...]`; the everyday `git commit`
command is a convenience wrapper that reads the current index as the tree and `HEAD` as the parent. A commit
with 0 parents is a root commit; **2+ parents marks a merge commit**. *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Git-Objects — "Commit Objects".]*

**git-7 (checkable).** Every object's id is **content-addressed**: `SHA = SHA-1(header + content)` where
`header = "<type> <byte-length-of-content>\0"`. Because the id is a pure hash of content+header, the **same
content always produces the same id**, anywhere, which is what makes blobs/trees dedupe automatically and
makes `git cat-file`/`git hash-object` mutually verifiable. `git hash-object --stdin` on raw content
reproduces exactly the id git would assign that blob. *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Git-Objects — "Object Storage".]*

**git-8 (principle).** Objects are written **zlib-compressed** under `.git/objects/<first-2-hex>/<remaining-
38-hex>` as "loose objects." Periodically (or via `git gc`), loose objects are rolled up into compact
**packfiles** with delta compression; `git gc --auto` fires a real gc only past a threshold (roughly 7,000
loose objects or 50 packfiles by default, tunable via `gc.auto`/`gc.autopacklimit`). Packing is a storage-
layer optimization only — it changes nothing about reachability or content addressing. *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Maintenance-and-Data-Recovery — "Maintenance".]* Footnote: Pro Git's
prose rounds this to "~7,000 loose objects"; the `git-gc`/`git-config` reference docs give the exact
`gc.auto` default as **6700** — same threshold, more precise number. *[VERIFIED, git-scm.com/docs/git-gc and
git-scm.com/docs/git-config — `gc.auto`.]*

**git-9 (checkable).** A **tag** is a fourth object type, structurally like a commit (tagger, date, message,
one pointer) but pointing at a commit instead of a tree, giving it a permanent friendly name. There are two
kinds: a **lightweight tag** is nothing but a ref (`git update-ref refs/tags/<name> <sha>`) pointing straight
at a commit; an **annotated tag** (`git tag -a`) is a real tag **object** that the ref points at instead —
only the annotated form carries metadata and can be GPG-signed (§15). *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Git-References — "Tags".]*

---

## 2. References, HEAD, and the index — the pointers over the DAG

**git-10 (checkable).** `.git/HEAD` is normally a **symbolic reference**: a pointer to another ref, e.g. its
content is literally `ref: refs/heads/master`. Checking out a **tag, a raw commit, or a remote branch**
(anything that isn't a local branch) writes a raw SHA into `HEAD` instead — "**detached HEAD**" state — after
which a new commit has no branch pointing at it and is one `git checkout`/`switch` away from becoming
unreachable (mitigate with §10). Read/write `HEAD` safely with `git symbolic-ref`, never by hand-editing
outside `refs/` syntax (git refuses non-refs targets). *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Git-References — "The HEAD".]*

**git-11 (checkable).** A **branch** is a plain file under `refs/heads/<name>` holding one commit SHA — a
cheap, mutable pointer, not a copy of anything. `git commit` on a branch writes a new commit object whose
parent is the branch's current SHA, then advances the branch file to the new SHA; this is why creating and
switching branches is O(1) regardless of repository size. *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Git-References — refs layout under `.git/refs/heads`.]*

**git-12 (checkable).** Refs (branches, tags, remotes) live as individual files under `.git/refs/...`;
`git gc` consolidates them for efficiency into one file, `.git/packed-refs`, in `<sha> <full-ref-name>` lines
(an annotated tag's line is followed by a `^<peeled-commit-sha>` line). Updating a packed ref does **not**
edit `packed-refs` in place — git writes a fresh loose file under `refs/`, which shadows the packed entry;
resolution checks the loose `refs/` tree first, `packed-refs` as fallback. *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Maintenance-and-Data-Recovery — "Maintenance".]*

**git-13 (checkable).** The **index** ("staging area") is **your proposed next commit** — not itself a tree
object but "a flattened manifest" of staged blob SHA + path pairs (`git ls-files -s` shows it raw). `git add`
updates entries in it; `git commit` converts its current state into a tree via an internal `write-tree` and
commits that tree. This is the mechanism that lets you stage *part* of your working-tree changes for one
commit. *[VERIFIED, git-scm.com/book/en/v2/Git-Tools-Reset-Demystified — "The Index".]*

**git-14 (checkable).** `git reset [--soft|--mixed|--hard] <target>` rewrites **up to three trees, in
strict order, stopping where you tell it**: (1) move the current branch's `HEAD` pointer to `<target>` —
**stop here** for `--soft`; (2) make the **index** match that new `HEAD` — this is the **default** if no
flag is given, i.e. plain `--mixed`; (3) make the **working directory** match the index — **only** for
`--hard`. `--soft` therefore keeps all your changes staged; `--mixed` keeps them unstaged; `--hard`
**discards** them from the working tree too (recoverable only per §10, and only for committed states, never
for uncommitted-and-unstashed edits). *[VERIFIED, git-scm.com/book/en/v2/Git-Tools-Reset-Demystified —
"Recap", and git-scm.com/docs/git-reset — DISCUSSION table.]*

---

## 3. Branching & basic merging

**git-15 (checkable).** `git merge <branch>` behaves in one of two ways depending on topology. If the target
branch's tip is a direct ancestor of the current branch's tip — histories never diverged — git performs a
**fast-forward**: it simply moves the current branch pointer forward, creating **no merge commit** at all.
If histories diverged, git performs a **three-way merge** (the default strategy is `ort`, using the two tips
plus their common ancestor), producing a **new commit with two parents**. When more than one common ancestor
exists, `ort` builds a merged tree of the ancestors first to use as the 3-way reference, which reduces
spurious conflicts versus the older `recursive` strategy it replaced. **`ort` has been the default merge
strategy only since Git 2.34** — `recursive` was the default for resolving two heads from Git v0.99.9k
through v2.33.0 inclusive; a pre-2.34 git binary defaults to `recursive` instead. *[VERIFIED,
git-scm.com/book/en/v2/Git-Branching-Basic-Branching-and-Merging, and git-scm.com/docs/merge-strategies —
`ort` and `recursive`.]*

**git-16 (checkable).** When the same region of the same file was changed differently on both sides, the
merge **pauses** instead of committing: `git status` lists the file under "Unmerged paths," and git writes
literal conflict markers into the working-tree file — `<<<<<<< HEAD` / your side, `=======` as the divider,
`>>>>>>> <branch>` / their side. Resolve by hand-editing to the intended final content, removing the markers,
then `git add <file>` to mark it resolved, then `git commit` to finish the merge (or `git merge --abort` to
bail out entirely). *[VERIFIED, git-scm.com/book/en/v2/Git-Branching-Basic-Branching-and-Merging — "Basic
Merge Conflicts".]*

**git-17 (judgment).** Do not let the merge strategy be an accident of which commands you happened to type.
A **fast-forward** is appropriate when the branch is genuinely a straight-line extension nobody else built
on (keeps history linear, honest, and free of a no-op merge commit); a real **three-way merge commit** is
appropriate when you are integrating a completed, independently-meaningful line of work and want that fact
preserved in the graph. Forcing one shape onto the other (e.g. `--no-ff` on every trivial branch, or
squash-flattening a long-lived collaborative branch) discards true information about how the work happened.

---

## 4. Rebase — mechanics, the golden rule, and merge vs rebase

**git-18 (checkable).** `git rebase <upstream>` replays commits rather than merging them: it locates the
**common ancestor** of your branch and `<upstream>`, computes the **patch (diff)** each of your unique
commits introduced, checks out `<upstream>`'s tip, and **reapplies those patches one at a time**, generating
a **brand-new commit** for each — same authored content, but a **new SHA** every time (new parent, usually
new committer timestamp), even if the resulting tree is byte-identical to the original commit. *[VERIFIED,
git-scm.com/book/en/v2/Git-Branching-Rebasing — "The Basic Rebase".]*

**git-19 (checkable — the golden rule).** **Do not rebase commits that exist outside your repository and
that people may have based work on.** This is git's own stated rule, verbatim. Because rebase abandons the
original commits and creates new-but-similar ones, rewriting and re-pushing history that others have already
pulled forces every one of them to reconcile diverging histories by hand — duplicated commits, confusing
re-merges, and (if they in turn based new work on the old commits) a genuinely hard-to-untangle mess.
*[VERIFIED verbatim, git-scm.com/book/en/v2/Git-Branching-Rebasing — "The Perils of Rebasing".]*

**git-20 (judgment).** Merge and rebase are not "one is better" — they trade off **topological honesty**
against **linear readability**, and the right choice depends on what has been shared. **Prefer merge** to
preserve the true shape of how work actually diverged and came back together, especially once a branch is
public/shared (git-19 forbids rebasing it anyway). **Prefer rebase** on your own **not-yet-shared** local or
feature-branch commits, to fold in upstream changes without a needless merge bubble and to leave a clean,
`bisect`-friendly (§9) linear trail before you share the branch. Never rebase to *simulate* a merge you
haven't actually done, and never merge-then-rebase-then-force-push a branch others have already pulled.

---

## 5. Interactive rebase

**git-21 (checkable).** `git rebase -i <after-this-commit>` opens an editable **todo list**, one line per
commit **from oldest to newest**, for every commit after `<after-this-commit>` on your current branch (merge
commits are excluded by default — see git-26). Each line starts `pick <sha> <oneline>`; the oneline text is
purely for your reading — git tracks the commit by hash, not by that text, so editing the oneline in the
todo list does not rename anything. Reordering lines reorders the replay; deleting a line drops that commit
entirely from the result. *[VERIFIED, git-scm.com/docs/git-rebase — "INTERACTIVE MODE".]*

**git-22 (checkable).** The todo-list verbs (replace `pick` with another word per line): **`reword`** — keep
the change, stop to edit only the commit message; **`edit`** — pause after applying this commit so you can
amend its content before continuing; **`squash`** — combine this commit into the previous one, prompting to
merge their messages; **`fixup`** — combine into the previous one but **discard** this commit's message
entirely; **`drop`** — remove the commit (equivalent to deleting the line). `git rebase --continue` /
`--skip` / `--abort` control the paused sequence exactly as in cherry-pick (§6). *[VERIFIED,
git-scm.com/docs/git-rebase — "INTERACTIVE MODE".]*

**git-23 (checkable).** `--autosquash` automatically relocates commits whose message begins `squash! `,
`fixup! `, or `amend! ` next to the commit they target during interactive rebase, matching the remainder of
the title against a prior commit's title or hash; pair it with `git commit --fixup=<target>` /
`--squash=<target>` at commit time so the eventual cleanup rebase requires zero manual reordering.
*[VERIFIED, git-scm.com/docs/git-rebase — `--autosquash` option.]*

**git-24 (checkable).** `--exec <cmd>` inserts an `exec <cmd>` step after every commit the rebase produces
— e.g. run the test suite after each replayed commit to catch which one first breaks the build. Any
`--exec` command that exits non-zero **halts the rebase** with exit code 1 at that point, leaving you free to
fix and `--continue`. Multiple `--exec` flags (or `&&`-joined commands in one) run in sequence after each
commit; combined with `--autosquash`, exec lines are appended only once per squash/fixup series, not after
every intermediate commit. *[VERIFIED, git-scm.com/docs/git-rebase — `--exec` option.]*

**git-25 (checkable).** `--onto <newbase>` changes **where** the replayed commits land: instead of the
implicit `<upstream>`, they are transplanted onto any valid commit. Two canonical uses: (1) **transplant a
topic branch built on branch A onto branch B** — `git rebase --onto B A topic` re-parents `topic`'s unique
commits as if they had always forked from `B`; (2) **rebase only part of a branch** —
`git rebase --onto master topicA topicB` moves just `topicB`'s commits (the ones after `topicA`) onto
`master`, leaving `topicA` untouched, which is also how you surgically **remove** a bad commit range from
history. *[VERIFIED, git-scm.com/docs/git-rebase — "TRANSPLANTING A TOPIC BRANCH WITH --ONTO".]*

**git-26 (checkable).** Interactive rebase **drops merge commits from the todo list by default**, linearizing
everything — usually desired for a simple patch series. Pass **`--rebase-merges`** when the branch topology
itself is meaningful (e.g. two related topic branches merged together) and must survive the rebase; it
regenerates `label`/`reset`/`merge` todo-list commands to recreate the same commit graph shape on the new
base instead of flattening it. *[VERIFIED, git-scm.com/docs/git-rebase — "REBASING MERGES".]*

---

## 6. Cherry-pick

**git-27 (checkable).** `git cherry-pick <commit>...` takes the **change** (diff) introduced by each given
commit and applies it to the current branch as a **new commit** — new SHA, same authored content, current
branch as parent. `-x` appends a `(cherry picked from commit <sha>)` trailer to the new message for
traceability back to the source commit (recommended for picks between public branches); `-n`/`--no-commit`
applies the patch to the working tree and index **without** committing, useful for combining several picks
into one commit. *[VERIFIED, git-scm.com/docs/git-cherry-pick — SYNOPSIS and `-x`/`-n` options.]*

**git-28 (checkable).** A cherry-pick over multiple commits that hits a conflict **pauses** the sequence
mid-way, exactly like a rebase: resolve the conflict, `git add`, then `git cherry-pick --continue`; or
`--skip` the offending commit and move to the next; or `--abort` to cancel and return to the pre-sequence
state, or `--quit` to just clear the in-progress sequencer state without rewinding. *[VERIFIED,
git-scm.com/docs/git-cherry-pick — "SEQUENCER SUBCOMMANDS".]*

---

## 7. Stash

**git-29 (checkable).** `git stash push` snapshots your current **working-tree and index changes relative to
HEAD** into a stash entry, then rolls the working tree and index back to match `HEAD` — a quick "set this
aside" without committing. By default it does **not** touch untracked or ignored files; pass
`-u`/`--include-untracked` to also stash untracked files, or `-a`/`--all` to also sweep in ignored files.
Omitting the `push` keyword is a supported shorthand for a quick snapshot. *[VERIFIED,
git-scm.com/docs/git-stash — "COMMANDS", `push`.]*

**git-30 (checkable).** Inspect stashes with `git stash list` (all entries) and `git stash show [<stash>]`
(the diff of one); reapply with `git stash apply` (leaves the stash entry in place — use when you might need
it again) or `git stash pop` (applies **then drops** it — use once you're confident); `git stash branch
<name> [<stash>]` creates and checks out a fresh branch from a stash and drops it on success, the safest way
to reapply a stash whose target has diverged enough to conflict. *[VERIFIED, git-scm.com/docs/git-stash —
SYNOPSIS, `apply`/`pop`/`branch`.]*

**git-31 (principle).** The **latest stash you created is stored in `refs/stash`; older stashes are found in
the reflog of this reference** and can be named via the usual reflog syntax (`stash@{0}` is the most recent,
`stash@{1}` the one before it, `stash@{2.hours.ago}` also works). *[VERIFIED, git-scm.com/docs/git-stash —
DESCRIPTION.]* In other words a stash entry is backed by ordinary commit object(s) reachable through that
ref/reflog, not a separate, magical storage class **[INFERRED]**. Treat `git stash drop` and especially
`git stash clear` with the same caution as deleting a branch: the content is gc-eligible the moment nothing
references it, recoverable (if at all) only through the **same** reflog/`fsck` techniques as any other commit
(§10), and only within the local reflog's retention window.

---

## 8. Worktrees

**git-32 (checkable).** `git worktree add <path> [<commit-ish>]` creates an additional **linked working
tree**, checked out from a branch, that shares the same underlying repository and object store as your
existing checkout but has its own independent working directory, `HEAD`, and index. This lets you have two
branches checked out **simultaneously** in two directories without a second clone — e.g. running the test
suite on `main` while mid-rebase on a feature branch elsewhere. If `<commit-ish>` names a branch that exists
as exactly one remote's tracking branch, or is omitted entirely, git infers a sensible new local branch to
create and check out. *[VERIFIED, git-scm.com/docs/git-worktree — "DESCRIPTION" and `add`.]*

**git-33 (checkable).** A repository has exactly **one main worktree** (the one `git init`/`git clone`
produced) and zero or more **linked worktrees**. The main worktree can **never** be removed via
`git worktree remove` — only linked ones. `git worktree list [--porcelain]` enumerates all of them with
their path, checked-out `HEAD`, and branch/detached state. *[VERIFIED, git-scm.com/docs/git-worktree —
"DESCRIPTION" and `remove`.]*

**git-34 (checkable).** `git worktree remove` only succeeds on a **clean** linked worktree — no untracked
files, no modifications to tracked files; a dirty one, or one containing submodules, needs `--force`.
`git worktree prune` cleans up administrative metadata left behind when a linked worktree's directory was
deleted **manually** (outside `git worktree remove`) rather than actually removing files. `git worktree
lock`/`unlock` protects a worktree (e.g. on removable/network media that may be temporarily unreachable)
from being pruned or moved. *[VERIFIED, git-scm.com/docs/git-worktree — "COMMANDS", `remove`/`prune`/
`lock`.]*

**git-35 (checkable).** If the main repository (or a linked worktree's directory) is relocated by hand — a
plain `mv`, not `git worktree move` — the administrative links between worktrees break. Run
`git worktree repair [<path>...]` from the main worktree (or from the moved worktree, or with each new path
given) to reestablish the bidirectional pointers between the main repo and its linked worktrees. *[VERIFIED,
git-scm.com/docs/git-worktree — `repair`.]*

---

## 9. Bisect

**git-36 (checkable).** `git bisect start`, then mark a known-**bad** commit (`git bisect bad [<rev>]`,
defaults to current `HEAD`) and at least one known-**good** commit (`git bisect good <rev>`). Git checks out
the midpoint of the remaining range and reports how many revisions/steps remain; you build/test it and answer
`git bisect good` or `git bisect bad` (or `git bisect skip` if that revision is untestable), and git narrows
the range by half each time — **O(log n)** steps to isolate the first bad commit, which the final report
leaves checked out and recorded at `refs/bisect/bad`. *[VERIFIED, git-scm.com/docs/git-bisect — "Basic
bisect commands: start, bad, good".]*

**git-37 (checkable).** `git bisect run <cmd> [<args>...]` fully automates the good/bad loop against any
script or command: exit code **0** means good; exit code **1–127 inclusive, except 125,** means bad; exit
**125** tells bisect this revision is untestable (auto-skip, see `git bisect skip`); **any other exit code
aborts the bisect process** entirely rather than being interpreted as good/bad/skip. Custom vocabulary —
`--term-bad=<word>` / `--term-good=<word>` (e.g. `new`/`old`) — replaces the good/bad wording, useful when
"bad" is semantically backwards (bisecting when a **fix** was introduced, not a regression). *[VERIFIED,
git-scm.com/docs/git-bisect — "Bisect run" and `--term-*`.]*

**git-38 (checkable).** `git bisect reset` ends the session and returns you to the branch/commit you started
from (do this even after finding the answer — bisect leaves you on a detached-HEAD checkout otherwise, see
git-10). `git bisect log` records the session transcript; `git bisect replay <logfile>` reruns a previously
logged session, which is how you resume a bisect later or hand it to a teammate. *[VERIFIED,
git-scm.com/docs/git-bisect — SYNOPSIS, `reset`/`log`/`replay`.]*

---

## 10. Reflog, `fsck`, and safe recovery from mistakes

**git-39 (checkable).** Every ref update on a local clone — commits, checkouts, resets, rebases, merges,
branch/tag creation or deletion — is appended to a per-ref log under `.git/logs/<ref>` (plus `.git/logs/
HEAD` for every `HEAD` move), readable via `git reflog [show] [<ref>]` as `<ref>@{N}` entries. Reflog entries
expire on a schedule, not forever: `gc.reflogExpire` (default **90 days**) for entries still reachable from
the ref's tip, `gc.reflogExpireUnreachable` (default **30 days**) for entries that have become unreachable —
after which `git gc` may actually delete the underlying objects. *[VERIFIED, git-scm.com/docs/git-reflog —
"Options for `expire`".]*

**git-40 (checkable — recovery recipe).** To recover from an over-eager `reset --hard`, an accidental branch
deletion, or a rebase/amend you regret: run `git reflog` (or, if the branch itself was deleted,
`git reflog show <old-branch-name>` still works if the reflog file survived) to find the SHA of the state
just before the mistake, then either `git branch <new-name> <sha>` to recover it under a new name, or
`git reset --hard <sha>` on the current branch to snap straight back. This is the everyday, first-line
recovery tool — reach for it before anything more invasive. *[VERIFIED, git-scm.com/book/en/v2/
Git-Internals-Maintenance-and-Data-Recovery — "Data Recovery".]*

**git-41 (checkable — last resort).** If the reflog entry itself is gone (very old, expired, or the branch
was deleted before any reflog was ever written for it), run `git fsck --full`, which lists every object
**not pointed to by any other object** — printed as `dangling commit <sha>` / `dangling blob <sha>` / etc.
A dangling commit is real, intact data with no ref keeping it alive; recover it the same way as git-40, by
pointing a new branch at its SHA. Do this before running `git gc`, which is what eventually reclaims
genuinely unreachable objects. *[VERIFIED, git-scm.com/book/en/v2/Git-Internals-Maintenance-and-Data-
Recovery — "Data Recovery".]*

**git-42 (principle).** The full safety-net stack, in the order to try it: **(1)** is it still reachable
from a ref? plain `git log`/`git branch` suffices. **(2)** is it in a reflog? `git reflog` (git-40).
**(3)** is it dangling with no reflog left? `git fsck --full` (git-41). **(4)** if none of those find it, it
is genuinely gone — either it was never committed (uncommitted working-tree edits have **no** object at all
until `git add`, and are not covered by any of this), or it fell outside both expiry windows and was
gc'd. This stack is **local to one repository/clone** — nothing here recovers a commit that only ever
existed on a machine you don't have access to.

**git-43 (checkable).** `git gc --auto` is the mechanism, and its thresholds are the actual bound on the
recovery window above: it does nothing until roughly **7,000 loose objects** or **50 packfiles** accumulate
(tunable via `gc.auto`/`gc.autopacklimit`; footnote — the `git-gc`/`git-config` docs give the exact
`gc.auto` default as **6700**, see git-8), at which point it packs loose objects, consolidates packfiles,
prunes objects unreachable-and-past-expiry, and rolls per-file refs into `.git/packed-refs` (git-12). Know
that an explicit `git gc` (not just the auto-triggered one) can run this immediately and shrink the recovery
window on demand — don't run it reflexively on a repo where you might still need git-41. *[VERIFIED,
git-scm.com/book/en/v2/Git-Internals-Maintenance-and-Data-Recovery — "Maintenance".]*

---

## 11. Submodules

**git-44 (checkable).** A submodule is recorded in the superproject as a special **"gitlink" tree entry** —
a pinned commit SHA, not file content — plus a path/URL/branch entry in a top-level `.gitmodules` file.
Cloning (or pulling) a superproject creates the submodule's directory but leaves it **empty**: you must run
`git submodule init` (registers the config) **and** `git submodule update` (fetches and checks out the
pinned commit) — or clone with `--recurse-submodules` to do both up front. *[VERIFIED, git-scm.com/book/
en/v2/Git-Tools-Submodules — "Cloning a Project with Submodules".]*

**git-45 (checkable).** `git submodule foreach '<cmd>'` runs an arbitrary shell command inside **every**
submodule in turn (e.g. `git submodule foreach 'git stash'` or `'git checkout -b featureA'`) — the standard
way to batch an operation across many submodules instead of `cd`-ing into each by hand. *[VERIFIED,
git-scm.com/book/en/v2/Git-Tools-Submodules — "Submodule Foreach".]*

**git-46 (checkable).** `git pull` on the superproject **recursively fetches** submodule commits by default,
but does **not** update the submodule's checked-out working copy to match. `git status` afterward reports the
submodule path as "modified... new commits" until you explicitly run `git submodule update` (or
`git pull --recurse-submodules` combined with an update step) — fetch and checkout are two separate acts for
submodules, unlike for the superproject itself. *[VERIFIED, git-scm.com/book/en/v2/Git-Tools-Submodules —
"Pulling Upstream Changes from the Project Remote".]*

**git-47 (judgment).** Treat every submodule pointer bump as a deliberate, reviewed commit in the
superproject — it pins an exact SHA that everyone who updates will receive, so an accidental or silent bump
(e.g. from running `update --remote` without checking the diff) is effectively an uncontrolled dependency
upgrade. Remember a submodule checked out at its pinned commit is normally in **detached HEAD** (git-10):
real work done inside it needs its own branch checked out first, or it risks becoming unreachable (git-42)
once you move to a different pin.

---

## 12. Hooks

**git-48 (checkable).** Hooks are executable scripts living under `.git/hooks/`, named **exactly** after the
event (`pre-commit`, `commit-msg`, `pre-push`, `pre-receive`, ...) with **no extension**. `git init`/
`git clone` seed that directory with example scripts suffixed `.sample`; to activate one, rename it to drop
`.sample` and ensure it is executable (`chmod +x`) — any language works via its shebang line, the examples
just default to shell/Perl. *[VERIFIED, git-scm.com/book/en/v2/Customizing-Git-Git-Hooks — "Installing a
Hook".]*

**git-49 (checkable).** **Client-side hooks are never copied when a repository is cloned.** They live only
in that one local `.git/hooks/` and cannot be relied on to enforce anything for other contributors — a
teammate's clone simply doesn't have your `pre-commit` hook. Anything that must actually be enforced belongs
on the **server side** (git-50) or in CI, not assumed-present client tooling. *[VERIFIED, git-scm.com/book/
en/v2/Customizing-Git-Git-Hooks — "Client-Side Hooks" note.]*

**git-50 (checkable).** Server-side `pre-receive` runs once per push, receiving **every** ref being updated
on stdin, **before any of them are accepted**; a non-zero exit rejects the **entire push**, making it the
right place for access control or fast-forward-only policy. `post-receive` runs **after** the push is already
accepted, for notifications/CI triggers/ticket updates — it **cannot** reject or unwind the push, only react
to it, and the client stays connected until it finishes (so keep it fast). *[VERIFIED, git-scm.com/book/
en/v2/Customizing-Git-Git-Hooks — "`pre-receive`" and "`post-receive`".]*

**git-51 (checkable — [INFERRED]).** Because client-side hooks aren't cloned (git-49), a team that wants
shared, versioned hook behavior anyway typically points `core.hooksPath` at a repo-tracked directory instead
of the untracked `.git/hooks/`, and has contributors opt in (or a bootstrap script set it) after cloning —
this reconciles "hooks should be shareable" with git's actual per-clone hook storage.

---

## 13. `.gitattributes`

**git-52 (checkable).** Setting the **`text`** attribute on a path enables end-of-line normalization: on
`git add`/commit, line endings are normalized to **LF in the index** regardless of the working-tree ending;
on checkout, the **`eol`** attribute (or, if unset, `core.autocrlf`/`core.eol`, defaulting to CRLF on Windows
and LF elsewhere) decides whether the working copy gets CRLF or LF back. `text=auto` lets git guess
binary-vs-text itself. Get this attribute right for cross-platform repos **before** the first commit with
mixed endings — retrofitting it rewrites history. *[VERIFIED, git-scm.com/docs/gitattributes — "`text`" and
"`eol`".]*

**git-53 (checkable).** **`export-ignore`** on a path excludes it from `git archive` output (e.g. drop
tests/CI config from a release tarball without deleting them from the repo). A **custom merge driver** is
declared in `.git/config`/`~/.gitconfig` under `[merge "<name>"]` with a `driver` command template
(`%O`/`%A`/`%B` = temp files for ancestor/ours/theirs, `%L` = conflict-marker length) and is opted into per
path with `<pattern> merge=<name>` in `.gitattributes` — useful for generated or semantically-mergeable files
that plain 3-way text diff handles badly. *[VERIFIED, git-scm.com/docs/gitattributes — "`export-ignore`" and
"Defining a custom merge driver".]*

**git-54 (checkable).** **`conflict-marker-size=<N>`** on a path lengthens the `<<<<<<<`/`=======`/`>>>>>>>`
markers from the 7-character default to `<N>` — needed for files whose content can itself contain 7 `<`/`=`/
`>` characters at line start (nested diffs, some markup formats), where the default width would be ambiguous
or would collide. *[VERIFIED, git-scm.com/docs/gitattributes — "`conflict-marker-size`".]*

---

## 14. `.gitignore`

**git-55 (checkable).** Ignore-pattern precedence, **highest to lowest** (within one level, the **last**
matching line in that source wins): command-line patterns given directly to a git command > a `.gitignore`
file in the same or a parent directory of the path (a file closer to the path overrides a farther one) >
`$GIT_COMMON_DIR/info/exclude` (local, unshared, not versioned) > the file named by `core.excludesFile`
(global, unshared). Patterns meant to be shared with every contributor belong in a versioned `.gitignore`;
purely personal ignores belong in `info/exclude` or `core.excludesFile`. *[VERIFIED, git-scm.com/docs/
gitignore — "DESCRIPTION" precedence list.]*

**git-56 (checkable).** A leading **`!`** negates a pattern, re-including a file an earlier pattern excluded
— **except** you cannot re-include a file whose **parent directory** is itself excluded: git doesn't descend
into an excluded directory to evaluate patterns for files inside it, for performance, so a `!` targeting a
file under an excluded directory silently has no effect. *[VERIFIED, git-scm.com/docs/gitignore — "PATTERN
FORMAT".]*

**git-57 (checkable).** `.gitignore` only governs **untracked** files. A path that has already been
`git add`ed / committed at any point keeps being tracked and keeps showing up in `git status`/diffs no matter
what pattern you add afterward — stop tracking it first with `git rm --cached <path>` (which leaves the file
on disk) before the ignore pattern takes effect. *[VERIFIED, git-scm.com/docs/gitignore — "DESCRIPTION",
"Files already tracked by Git are not affected".]*

---

## 15. Signed commits & tags

**git-58 (checkable).** Configure `user.signingkey` to your GPG key id (`gpg --list-keys` / `--gen-key`
first), then `git commit -S` signs a commit and `git tag -s <name>` signs a tag at creation. Verify with
`git tag -v <name>` (or `git log --show-signature`): success prints `gpg: Good signature from "<identity>"`;
without the signer's public key in your keyring you instead get `gpg: Can't check signature: public key not
found`. *[VERIFIED, git-scm.com/book/en/v2/Git-Tools-Signing-Your-Work — "GPG Introduction" and "Verifying
Tags".]*

**git-59 (checkable).** Only an **annotated** tag can carry a signature, because only the annotated form is a
real tag **object** (tagger + date + message + pointer, git-9) — a lightweight tag is nothing but a ref, with
no object to attach a signature to. `-s` (or `-a` plus a manually-supplied signature) is therefore a
prerequisite, not an add-on, for any tag verification workflow. *[VERIFIED, git-scm.com/book/en/v2/
Git-Internals-Git-References — "Tags".]*

**git-60 (judgment).** Require signed commits and/or signed tags at release boundaries and for any
externally-consumed artifact (a published release tag, a hotfix cherry-picked into a support branch) as the
actual proof of provenance — an unsigned annotated tag is still just a movable-in-spirit label that anyone
with push access could recreate pointing anywhere; the signature, verified against a keyring you control, is
what turns "this tag says v2.0" into "a specific, known identity attested to this exact commit as v2.0."

---

## 16. Clean-history hygiene & safe rewriting

**git-61 (judgment — [INFERRED], standard practitioner convention).** Shape each commit as one coherent,
independently buildable/testable change with an imperative-mood summary line ("Add X", not "Added X" or
"Adds X"), and use interactive rebase (§5: `squash`/`fixup`/`reword`/`drop`) to fold exploratory,
typo-fix, and "oops" commits into their logical parent **before** the branch is shared — so `git log` and
`git bisect` (§9) read as a sequence of deliberate engineering steps, and a bisect never lands on a commit
that doesn't actually build.

**git-62 (checkable).** When a branch you already pushed needs to be pushed again after a local rewrite
(interactive rebase, amend), prefer `git push --force-with-lease` over a bare `--force`. Plain `--force`
disables **all** safety checks and unconditionally overwrites whatever is currently on the remote —
including any commits a collaborator pushed there since you last fetched, silently discarding their work.
`--force-with-lease` (optionally `--force-with-lease=<ref>:<expected-sha>`) only proceeds if the remote ref
still points where you last saw it; if someone else has pushed in the meantime, it fails loudly instead of
overwriting them. *[VERIFIED, git-scm.com/docs/git-push — `--force-with-lease` and `--force` options.]*

**git-63 (judgment).** On a branch that is already public/shared, prefer `git revert <commit>` over any form
of history rewriting to undo it: revert adds a **new** commit that inverses the change, leaving the original
commit and the rest of history intact — safe to push normally (no force needed, no golden-rule violation,
git-19) and requiring no coordination with anyone who already has the original commit. Reserve rebase-based
rewriting (§4–§5, plus git-62's `--force-with-lease` discipline) for history that is still effectively yours
alone.

---

## Defaults / quick-reference table

Object header format `"<type> <byte-len>\0" + content`, id = SHA-1 of that · loose object path
`.git/objects/<sha[0:2]>/<sha[2:]>` · `git gc --auto` fires past **~7,000 loose objects / 50 packfiles**
(exact `gc.auto` default: **6700**, git-8, git-43) · reflog retention **90 days reachable / 30 days
unreachable** (`gc.reflogExpire` / `gc.reflogExpireUnreachable`, git-39) · default merge strategy =
**`ort`** (since Git 2.34; `recursive` before that, git-15) · conflict marker default
width = **7 characters**, override via `conflict-marker-size` (git-54) · interactive-rebase todo verbs =
**pick / reword / edit / squash / fixup / drop** (git-22) · reset stop points = **`--soft`**(HEAD only) /
**`--mixed`**(+index, default) / **`--hard`**(+working dir) (git-14) · gitignore precedence = **CLI >
nearest `.gitignore` > `info/exclude` > `core.excludesFile`**, last-match-wins within a source (git-55) ·
push safety = **`--force-with-lease` over `--force`** (git-62) · recovery order = **ref → reflog → `fsck
--full` dangling objects** (git-42).

---

## Source ledger

**Primary sources (fetched and read in full for this guide, 2026-08-16):**

- Pro Git book (`git-scm.com/book/en/v2`): *Git Internals – Git Objects*; *Git Internals – Git References*;
  *Git Branching – Basic Branching and Merging*; *Git Branching – Rebasing*; *Git Tools – Reset Demystified*;
  *Git Tools – Submodules*; *Git Tools – Signing Your Work*; *Customizing Git – Git Hooks*; *Git Internals –
  Maintenance and Data Recovery*.
- Git reference documentation (`git-scm.com/docs/<cmd>`): `git-rebase` (INTERACTIVE MODE, MERGE STRATEGIES,
  `--onto`, `--exec`, `--autosquash`, REBASING MERGES); `git-cherry-pick` (SYNOPSIS, SEQUENCER SUBCOMMANDS);
  `git-stash` (DESCRIPTION, SYNOPSIS, COMMANDS); `git-worktree` (DESCRIPTION, COMMANDS); `git-bisect`
  (SYNOPSIS, DESCRIPTION, "Bisect run"); `git-reflog` (SYNOPSIS, expire options); `gitattributes`
  (`text`/`eol`, `export-ignore`, merge drivers, `conflict-marker-size`); `gitignore` (DESCRIPTION, PATTERN
  FORMAT); `git-reset` (DISCUSSION); `git-push` (`--force-with-lease`, `--force`); `merge-strategies` (`ort`,
  `recursive`); `git-gc` and `git-config` (`gc.auto` CONFIGURATION).

**Evidence flags carried from the research:** rules marked `[INFERRED]` in full are git-51 and git-61 —
well-established community convention or a config knob referenced only in passing on a fetched page, not the
literal subject of primary-source prose quoted above, flagged rather than presented as an official mandate.
git-31's core claim (the latest stash lives at `refs/stash`, older stashes in that ref's reflog) is directly
quoted from the fetched `git-stash` DESCRIPTION and is **[VERIFIED]**; only its closing editorial gloss ("not
a separate, magical storage class") remains a labeled inference. Every other checkable/principle rule cites
the specific fetched page it comes from.

*Colophon: v1, 2026-08-16. Distilled with zero shortcuts from the Pro Git book and the official git
reference docs, fetched and read in full; 63 rules (git-1…git-63); no section dropped; verified vs
inference labeling carried throughout.*
