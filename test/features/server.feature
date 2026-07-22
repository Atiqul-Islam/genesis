# Source: test/specs/genesis-memory-server.md

Feature: MCP server protocol and lifecycle
  Protocol- and process-level behaviour that belongs to no single tool: the initialize
  handshake and pinned protocol version, tool advertisement, the isError-vs-JSON-RPC-error
  split, clean EOF shutdown, offline operation, and database isolation through the
  GENESIS_MEMORY_DB environment variable.

  # Acceptance Criterion 1
  Scenario: Initialize advertises the tools capability and the pinned protocol version
    Given a spawned memory server child process over stdio
    When the client sends an initialize request
    Then the initialize response advertises "tools" under capabilities
    And the initialize response protocolVersion is "2024-11-05"

  # Acceptance Criterion 2
  Scenario: A tools list response names the three memory tools
    Given an initialized memory server child process over stdio
    When the client sends a "tools/list" request
    Then the response contains the tool names "store", "recall" and "consolidate"

  # Acceptance Criterion 11
  Scenario: A failing tool call is a successful response carrying isError
    Given an initialized memory server child process over stdio
    When the client makes a tool call that triggers an internal failure
    Then the response is a JSON-RPC result whose isError field is true

  # Acceptance Criterion 12
  Scenario: A malformed request yields a protocol level JSON-RPC error
    Given a spawned memory server child process over stdio
    When a syntactically invalid JSON-RPC request is written to the server stdin
    Then the server replies with a JSON-RPC error object rather than a result

  # Acceptance Criterion 14
  Scenario: The server exits cleanly when the client closes stdin
    Given a spawned memory server child process over stdio
    When the client closes the server stdin
    Then the server child process terminates with exit status 0

  # Acceptance Criterion 15
  Scenario: Tool calls complete with outbound network access unavailable
    Given an initialized memory server child process over stdio with the model already on disk
    And outbound network access is unavailable
    When the client makes a tool call for each of "store", "recall" and "consolidate"
    Then every one of those tool calls completes successfully

  # Acceptance Criterion 17
  Scenario: Two servers configured with different database paths never share memories
    Given a memory server child process launched with GENESIS_MEMORY_DB pointing at a first temporary file
    And a second memory server child process launched with GENESIS_MEMORY_DB pointing at a different temporary file
    When agent "alpha" stores the memory "isolated note" through the first server
    And agent "alpha" recalls "isolated note" through the second server
    Then the recall result contains no entry whose text is "isolated note"
