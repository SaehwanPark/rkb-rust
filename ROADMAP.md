# ROADMAP

Each phase is delivered as a thin test-first slice. A phase is complete only
when its behavior is represented by parity tests, implementation, SDD updates,
and QA review.

1. Foundation: repository, CLI namespace, fixtures, SDD, harness, and CI. (Complete)
2. Domain foundation: record types, configuration, paths, and typed failures. (Complete)
3. Preservation: inventory discovery, archive downloads, rate limiting, and progress. (Complete)
4. Transformation: metadata extraction (Complete), HTML/PDF/XLSX parsing, and chunking.
5. Knowledge model: variables, graph seeds, and provenance QA.
6. Retrieval: SQLite FTS5 index, exact-term behavior, search, and evaluation.
7. Agent serving: agent context, MCP, setup, and downstream integration helpers.
8. Release: compatibility report, performance evidence, and distribution workflow.

Semantic reranking is deferred until deterministic lexical retrieval matches the
Python baseline. Parser or ML dependencies are selected within their owning
slice rather than committed to in advance.
