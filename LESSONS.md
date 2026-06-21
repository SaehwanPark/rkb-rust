# Lessons

Record recurring setup traps, debugging lessons, and non-obvious fixes here.
Each entry should state context, cause, resolution, and prevention concisely.

## GitHub Actions checkout runtime

- Context: The initial CI run passed but warned that `actions/checkout@v4` targets deprecated Node.js 20.
- Cause: GitHub runners had moved action execution to Node.js 24 compatibility mode.
- Resolution: Use `actions/checkout@v5`.
- Prevention: Treat successful CI annotations as actionable compatibility signals during bootstrap review.
