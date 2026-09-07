# Implementation plans

**The V1 rewrite is finished.** Its plans — the `m1`–`m11` milestones, the
`*-next` cutover work, the parity and testing programmes — are history, not
instructions, and they live in
[`../archive/implementation-plans/`](../archive/implementation-plans/).

They were moved because they were being read as current (#3154). Eighty-plus
plan documents sitting in the live architecture tree, several of them describing
milestones as `Planned` that shipped a year ago, is a trap for anyone — human or
agent — trying to work out what is true now.

## What belongs here

Plans for work that is **in progress or upcoming**. A plan is finished when its
last slice merges; move it to the archive then, in the same PR if you can.

Currently live:

- `wall-clock-as-of-s3-implementation-plan.md` — the #3112 wall-clock epic's S3
  slice plan. Kept because it documents corrections made *during*
  implementation (S2b's index behaviour, S3a's no-scan-fallback finding) that
  the design doc alone does not carry.

## Where to look instead

| Question | Read |
|---|---|
| What is the architecture now? | `../{layer}-architecture.md` |
| What does this contract say? | `../engine/<contract>.md`, `../storage/<layer>.md` |
| What is being built next? | The GitHub Projects boards, not this directory |
| How was V1 built? | `../archive/implementation-plans/` |
