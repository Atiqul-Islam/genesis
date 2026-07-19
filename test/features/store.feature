# Source: test/specs/genesis-memory-server.md

Feature: Store a memory
  A memory sent to the store tool is embedded with the local ONNX model and persisted in
  the shared SQLite vector store under the caller-supplied agent_id, so that a later recall
  for that same agent finds it. Every scenario runs against a fresh temporary database.

  # Acceptance Criterion 3
  Scenario: A stored memory is recalled from a paraphrase of its text
    Given a memory server with an empty database
    And agent "alpha" has stored the calibrated source text
    And agent "alpha" has stored the calibrated dissimilar decoy texts
    When agent "alpha" recalls with the calibrated paraphrase of the source text
    Then the recall result contains an entry whose text is the calibrated source text
    And that entry is first in the recall result

  # Acceptance Criterion 9
  Scenario: Embedding the same text twice in one process yields the same vector
    Given a loaded embedder
    When the embedder embeds "a quick brown fox jumps over the lazy dog" twice in the same process
    Then the two vectors have cosine similarity of at least 0.9999

  # Acceptance Criterion 10
  Scenario: A vector whose length is not 384 is rejected with an error
    Given an open vector store
    When a 383 element vector is passed to VectorStore insert and VectorStore knn
    Then both calls return an error
    And the test process has not panicked
