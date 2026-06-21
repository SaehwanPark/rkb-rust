# Lessons

Record recurring setup traps, debugging lessons, and non-obvious fixes here.
Each entry should state context, cause, resolution, and prevention concisely.

## GitHub Actions checkout runtime

- Context: The initial CI run passed but warned that `actions/checkout@v4` targets deprecated Node.js 20.
- Cause: GitHub runners had moved action execution to Node.js 24 compatibility mode.
- Resolution: Use `actions/checkout@v5`.
- Prevention: Treat successful CI annotations as actionable compatibility signals during bootstrap review.

## Float structures and Eq trait derivation

- Context: Creating configuration structures containing floating point values (`f64`).
- Cause: Deriving `Eq` on structures with floats fails compilation because `f64` does not implement `Eq` due to `NaN != NaN` behavior.
- Resolution: Derive only `Clone`, `Debug`, and `PartialEq` on configurations containing float properties.
- Prevention: Check for floating point properties before automatically deriving `Eq` on new structs.

