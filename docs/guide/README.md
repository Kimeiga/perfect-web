# The Pleris guide

A guide to writing web apps in Pleris, for a developer who knows TypeScript
and React, Next or Svelte.

This guide is being written (track `guide`). What is planned:

- **Why Pleris**: what it refuses that other frameworks let through, and what
  it does not do yet.
- **A tour of the language**, one topic a page section.
- **A tutorial**: one small app, built end to end and run on the development
  server.
- **The diagnostic codes**: each code, its meaning and its ADR, generated from
  `compiler/pw-core/src/codes.rs`.

The plan is that every Pleris example in this guide is compiled by a test
with the real compiler, so an example that stops compiling fails `just ci`.
That test does not exist yet.
