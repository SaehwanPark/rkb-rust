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

## URL Classification Priority Ordering

- Context: Classifying resource kinds using path patterns.
- Cause: A mismatch occurred where certain URLs containing specific path substrings (like `/cms-data/files/`) had to be classified as `dataset_page` even if they ended with file extensions like `.pdf`. If file extension checks were placed first, they incorrectly matched `asset` instead of `dataset_page`.
- Resolution: Prioritize substring contains matches over suffix checks when evaluating classification rules.
- Prevention: Re-verify classification ordering logic explicitly with real/mock fixtures that exercise borderline/overlapping conditions.

## Move Semantics in Test Closures

- Context: Inspecting thread/closure execution counts or delays inside test assertions.
- Cause: Rust closure capture rules require moving ownership of variables (like counter variables) when passed into pipeline runners, making them inaccessible for post-run assertions in the outer test scope.
- Resolution: Wrap shared state/counter variables in thread-safe reference counters like `std::sync::Arc` (or `Arc<AtomicUsize>`) to allow clone-based sharing and mutation.
- Prevention: Identify shared assertion counters in mock testing early and initialize them using `Arc` counters.

## Pinned Clippy component can be present but unusable

- Context: Baseline `cargo clippy` failed for the pinned Rust 1.96 toolchain even though `rustup component list` reported Clippy as installed.
- Cause: A concurrent initial toolchain bootstrap left the component registered without an applicable `cargo-clippy` binary.
- Resolution: Remove and re-add Clippy for the exact pinned toolchain before rerunning the gate.
- Prevention: Bootstrap the pinned toolchain once before launching Cargo gates in parallel.
