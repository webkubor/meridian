<!-- Keep this short; delete anything that does not apply. -->

## What this changes, and why

<!-- One or two paragraphs. Link the issue if there is one. -->

## Where it sits in the spec

<!-- Which docs/ section or build-order step this relates to — for example a
     docs/02 §17 build-order phase (P0–P12), a docs/03 command row, or a
     docs/04 screen. -->

## What was verified

<!-- Name the commands you actually ran; for the Rust gates, run them from src-tauri/. -->

- [ ] `pnpm typecheck`
- [ ] `pnpm build`
- [ ] `cargo test --all-targets`
- [ ] `cargo clippy --all-targets -- -D warnings`

## Reminder

Meridian's feature surface comes from a reverse-engineering study, but only the
mechanism is copied — never assets, code, or UI text from another product.
Keep it that way in this change.
