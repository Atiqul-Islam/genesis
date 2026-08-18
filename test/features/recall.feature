# Source: test/specs/genesis-memory-server.md

Feature: Recall memories
  The recall tool embeds the query, runs the hybrid retrieval pipeline restricted to the caller's
  agent_id, and returns at most k hits most relevant first as a machine-readable JSON array of
  {id, text, score} objects (higher score = more relevant) carried inside a single text content block.

  # Acceptance Criterion 4
  Scenario: Recall returns at most k memories ordered most relevant first
    Given a memory server with an empty database
    And agent "alpha" has stored 10 distinct memories
    When agent "alpha" recalls "morning routine" with k of 3
    Then the recall result contains exactly 3 entries
    And the score values in the recall result are non-increasing

  # Acceptance Criterion 5
  Scenario: Recall of the exact stored text ranks it first with a positive score
    Given a memory server with an empty database
    And agent "alpha" has stored the memory "deploy on friday afternoons"
    When agent "alpha" recalls "deploy on friday afternoons" with k of 5
    Then the first recall entry has text "deploy on friday afternoons"
    And the first recall entry has a positive score

  # Acceptance Criterion 6
  Scenario: Recall never returns a memory belonging to another agent
    Given a memory server with an empty database
    And agent "alpha" has stored the memory "alpha private note"
    And agent "beta" has stored the memory "beta private note"
    When agent "beta" recalls "private note" with k of 5
    Then no recall entry has text "alpha private note"

  # Acceptance Criterion 7
  Scenario: Recall without k returns at most five memories
    Given a memory server with an empty database
    And agent "alpha" has stored 6 distinct memories
    When agent "alpha" recalls "morning routine" without k
    Then the recall result contains exactly 5 entries

  # Acceptance Criterion 16
  Scenario: The recall payload is a machine readable JSON array
    Given a memory server with an empty database
    And agent "alpha" has stored the memory "payload shape probe"
    When agent "alpha" recalls "payload shape probe" with k of 5
    Then the text content block of the recall result parses as a JSON array
    And every element of that array has exactly the keys id and text and score
    And in every element id is an integer, text is a string and score is a number
