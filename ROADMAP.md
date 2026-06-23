# ROADMAP

Each phase is delivered as a thin test-first slice. A phase is complete only
when its behavior is represented by parity tests, implementation, SDD updates,
and QA review.

1. Foundation: repository, CLI namespace, fixtures, SDD, harness, and CI. (Complete)
2. Domain foundation: record types, configuration, paths, and typed failures. (Complete)
3. Preservation: inventory discovery, archive downloads, rate limiting, progress logs, and progress summaries. (Complete)
4. Transformation: metadata extraction, HTML/PDF/XLSX parsing, and chunking. (Complete)
5. Knowledge model: variables, graph seeds, and provenance QA. (Complete)
6. Retrieval: SQLite FTS5 index, exact-term behavior, search, evaluation, and hybrid reranking. (Complete)
7. Agent serving: agent context, MCP, setup, and downstream integration helpers. (Complete)
8. Release: compatibility report, performance evidence, and distribution workflow. (Evidence drafted)

Model-backed semantic reranking remains deferred; current hybrid reranking uses
deterministic local embedding vectors stored in the rebuildable SQLite serving
index. Parser or ML dependencies are selected within their owning slice rather
than committed to in advance.
