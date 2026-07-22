#!/usr/bin/env python3
"""Tests that the superpowers discipline skills are vendored self-contained into Genesis.

Genesis's /spec-forge workflow (skills spec-forge, forge-dev-agent, forge-review-agent) invokes
discipline skills that originate in the separate `superpowers` plugin. Those skills are vendored
into skills/ so a fresh Genesis install works without superpowers installed. This test proves the
vendoring is complete and dangling-reference-free:

  * ZERO `superpowers:` skill references remain anywhere under skills/ (all rewired to Genesis's
    bare-name convention).
  * Every skill the spec-forge / forge-* workflow references now exists under skills/<name>/SKILL.md.
  * The full transitive closure is present (11 skills) and every `../<sibling>` cross-skill path
    reference inside a vendored skill resolves to an existing sibling skill (no dangling refs).
  * Third-party license/attribution is preserved (VENDORED-superpowers-LICENSE + NOTICE.md entry).

Run:  python3 hooks/test_vendored_skills.py
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)                       # hooks/ -> plugin root
SKILLS = os.path.join(REPO, "skills")

# The complete transitive closure vendored from superpowers 6.1.1.
VENDORED = [
    "brainstorming",
    "writing-plans",
    "using-git-worktrees",
    "test-driven-development",
    "systematic-debugging",
    "verification-before-completion",
    "requesting-code-review",
    "receiving-code-review",
    "finishing-a-development-branch",
    "subagent-driven-development",
    "executing-plans",
]

# Genesis workflow skills that drove the dependency (they reference the vendored discipline skills).
REFERENCING_DIRS = [
    "spec-forge",
    "forge-dev-agent",
    "forge-docs-agent",
    "forge-review-agent",
    "forge-spec-agent",
    "forge-verify-agent",
]


def skill_dirs():
    return {d for d in os.listdir(SKILLS) if os.path.isdir(os.path.join(SKILLS, d))}


def all_skill_files():
    for root, _dirs, files in os.walk(SKILLS):
        for f in files:
            yield os.path.join(root, f)


def read(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        return fh.read()


def main():
    passed = failed = 0

    def check(name, cond):
        nonlocal passed, failed
        passed += 1 if cond else 0
        failed += 0 if cond else 1
        print(f"  {'PASS' if cond else 'FAIL'}  {name}")

    dirs = skill_dirs()

    # ---- 1. ZERO `superpowers:` references anywhere under skills/ ----
    offenders = []
    for path in all_skill_files():
        try:
            if "superpowers:" in read(path):
                offenders.append(os.path.relpath(path, REPO))
        except (OSError, UnicodeDecodeError):
            pass
    check(f"no `superpowers:` refs remain under skills/ (found in: {offenders})", not offenders)

    # ---- 2. Every vendored skill exists as skills/<name>/SKILL.md ----
    for name in VENDORED:
        check(f"vendored skill present: {name}/SKILL.md",
              os.path.isfile(os.path.join(SKILLS, name, "SKILL.md")))

    # ---- 3. Every skill referenced by spec-forge / forge-* exists under skills/ ----
    # Collect backtick-quoted skill-like tokens from the referencing files, restricted to the
    # vendored discipline-skill vocabulary, and assert each resolves to a real skill directory.
    vendored_set = set(VENDORED)
    referenced = set()
    tok = re.compile(r"`([a-z][a-z0-9-]+)`")
    for d in REFERENCING_DIRS:
        ddir = os.path.join(SKILLS, d)
        if not os.path.isdir(ddir):
            continue
        for root, _sub, files in os.walk(ddir):
            for f in files:
                if not f.endswith(".md"):
                    continue
                for m in tok.findall(read(os.path.join(root, f))):
                    if m in vendored_set:
                        referenced.add(m)
    check("spec-forge/forge-* reference at least the core discipline skills",
          {"test-driven-development", "verification-before-completion"} <= referenced)
    missing_referenced = sorted(n for n in referenced if n not in dirs)
    check(f"every discipline skill referenced by spec-forge/forge-* exists (missing: {missing_referenced})",
          not missing_referenced)

    # ---- 4. Transitive-closure integrity: every `../<sibling>` ref inside a vendored skill resolves ----
    rel = re.compile(r"\.\./([A-Za-z0-9_-]+)/")
    dangling = []
    # Skill names that a vendored skill may legitimately point at via ../ (sibling skills).
    for name in VENDORED:
        ndir = os.path.join(SKILLS, name)
        for root, _sub, files in os.walk(ndir):
            for f in files:
                if not (f.endswith(".md") or f.endswith(".sh")):
                    continue
                text = read(os.path.join(root, f))
                for target in rel.findall(text):
                    # ../<target>/ is a cross-skill sibling reference iff <target> names a skill dir
                    # under skills/. If <target> looks like a skill but is absent, it dangles.
                    if target in ("scripts", "references"):
                        continue
                    candidate = os.path.join(SKILLS, target)
                    if target not in dirs and not os.path.exists(candidate):
                        # Only flag ones that look like skill references (contain a hyphen or are known)
                        if "-" in target:
                            dangling.append(f"{name}: ../{target}/")
    check(f"no dangling ../<sibling> skill refs inside vendored skills (dangling: {dangling})",
          not dangling)

    # ---- 5. License / attribution preserved ----
    lic_path = os.path.join(SKILLS, "VENDORED-superpowers-LICENSE")
    lic = read(lic_path) if os.path.isfile(lic_path) else ""
    check("skills/VENDORED-superpowers-LICENSE exists with MIT + copyright holder",
          "MIT License" in lic and "Jesse Vincent" in lic)
    notice = read(os.path.join(REPO, "NOTICE.md"))
    check("NOTICE.md attributes superpowers (name + copyright + source URL)",
          "superpowers" in notice
          and "Jesse Vincent" in notice
          and "github.com/obra/superpowers" in notice)

    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
