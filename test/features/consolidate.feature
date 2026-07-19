# Source: test/specs/genesis-memory-server.md

Feature: Consolidate memories
  The consolidate tool is the only place dedup/merge runs (deliberate deviation D8 from
  the source's "dedup on insert" phrasing). Two memories at or above tau_merge collapse
  into the higher-scored survivor and the loser is retired via superseded_by, after which
  recall never returns it again.

  # Acceptance Criterion 13
  Scenario: A near duplicate is retired only by an explicit consolidate call
    Given a memory server with an empty database
    And agent "alpha" has stored two memories whose cosine similarity is at or above tau_merge
    When agent "alpha" recalls the shared subject of those two memories with k of 5
    Then the recall result contains exactly 2 entries
    When agent "alpha" consolidates
    And agent "alpha" recalls the shared subject of those two memories with k of 5
    Then the recall result contains exactly 1 entry
    And the recall result does not contain the superseded memory

  # Acceptance Criterion 8
  Scenario: Recall never returns a row that a previous consolidate superseded
    Given a memory server with an empty database
    And agent "alpha" has two memories that a consolidate call has merged
    When agent "alpha" recalls the shared subject of those two memories with k of 5
    Then no recall entry has the id of the row whose superseded_by is non-null
