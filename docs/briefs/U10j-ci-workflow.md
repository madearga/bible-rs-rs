GOAL
Add the continuous integration the program claims it has, so every pull request is checked
automatically instead of only by hand.

SCOPE
You may write only inside the new repository. Create:
  .github/workflows/ci.yml        the Linux and macOS CI workflow
Do not touch anything else. No source changes, no README changes, no Cargo changes.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements are
in docs/SPEC.md, the last section, "The build". Read it, then read on your branch:
Cargo.toml (the workspace members and the shared dependency versions), rust-toolchain.toml (the
pinned toolchain), crates/core/Cargo.toml, crates/app/Cargo.toml, and scripts/bundle-macos.sh and
scripts/verify-distribution.sh (what the macOS job can reuse).

What the workflow must do.

Linux job, on ubuntu-latest:
- Install the Rust toolchain with the fmt and clippy components, matching rust-toolchain.toml.
- Cache the cargo registry and the build directory.
- Install the system packages the GUI needs: libxkbcommon-dev, libgl1-mesa-dev, libwayland-dev,
  libx11-dev, xvfb, mesa-utils, x11-utils.
- Run cargo fmt --all --check.
- Run cargo clippy --workspace --all-targets -- -D warnings.
- Run cargo test --workspace.
- Run the search smoke test against the release binary:
  cargo run --release -p bible-app -- search "love one another" --limit 3
- Run the two release benchmarks with short measurement times, the search bench and, if it exists,
  the layout or scroll bench:
  cargo bench -p bible-core --bench search -- --warm-up-time 1 --measurement-time 2
- Run the bundle script and the verification script under a virtual display:
  Xvfb :99 -screen 0 1280x800x24 &
  export DISPLAY=:99
  ./scripts/bundle-macos.sh
  ./scripts/verify-distribution.sh --launch
  The bundle script refuses to run on a machine that is not Darwin, so the Linux job must not call
  it. On Linux, build and test the app instead, and run the shot example under Xvfb to prove the
  render harness works headless:
  cargo run --release --example shot -- /tmp/ci-shot.png 1280x800
  and assert the file exists and is not empty.

macOS job, on macos-latest:
- Install the Rust toolchain with the fmt and clippy components.
- Cache the cargo registry and the build directory.
- Run cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings, and
  cargo test --workspace.
- Build the bundle: ./scripts/bundle-macos.sh
- Verify it: ./scripts/verify-distribution.sh --launch
- Render one frame through the shot example: cargo run --release --example shot -- /tmp/ci-shot.png
- Upload the shot and the bundle as an artifact, so a reviewer can look at what CI built.

Rules.
- Pin every action to a commit SHA, not a tag, and add a comment naming the version. An unpinned
  action is a supply chain hole.
- Give each job a timeout-minutes, so a hung build does not burn the runner.
- Do not run the release benchmark suite over the whole workspace; the long release build is slow
  and the benchmarks are already covered by the targeted benches.
- The workflow must fail on the first failing step, which is the default.
- The job names must say what they check, in plain words.

ACCEPTANCE
- The workflow file parses as valid YAML. Prove it with a parser:
  python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))" if PyYAML is
  available, otherwise with another YAML parser you can run, and report the exact command and its
  exit code. If no YAML parser is available, say so plainly and report it.
- The workflow references the toolchain, the format check, the clippy check, the tests, the search
  smoke test, the benchmarks, the bundle build, the bundle verification, and the shot render, so
  every claim in the spec's build section is covered.
- Every action reference is a commit SHA of 40 hex characters, verified by:
  grep -nE "uses: [^@]+@[^0-9a-f]" .github/workflows/ci.yml and the absence of any match.
  Report the exact command and its output.
- No secret is used, and no workflow_dispatch input is required.
- Every file you create starts with a comment naming the SPDX licence and the copyright holder.

VERIFY
Run exactly these and paste the outputs into your report:
    python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml ok')"
    grep -nE "uses: [^@]+@[^0-9a-f]" .github/workflows/ci.yml ; echo "unpinned actions: $?"
    grep -cE "uses: [^@]+@[0-9a-f]{40}" .github/workflows/ci.yml
    cargo fmt --all --check
    cargo test --workspace
The workflow itself cannot run in this environment, so say plainly in the report that the first
real run happens when the branch is pushed, and that the local checks only prove the file parses
and the pins are complete.

TIMEBOX
Three hours of work. Work autonomously; the operator is away. Do not ask questions. Record any
genuine product or tooling decision under BLOCKED-ITEMS.

FORBIDDEN
No source or naming from /Users/madearga/bible-rs-reference. No SPDX headers other than MIT with
madearga. No changes outside SCOPE. Do not disable a check, use continue-on-error, or weaken a
step to make the workflow pass. No force-push, rebase, or merging.

REPORT
Reply with: status, branch, head SHA, PR URL, the exact verify outputs, the list of steps each
job runs, the number of pinned actions, limitations, BLOCKED-ITEMS.
