# Project Guidelines

## Code Style

### Comment Style

- Comments within source code should be written in English by default
- Use clear, concise English for inline comments
- Document public APIs with doc comments (`///` in Rust)
- Keep comments up-to-date with code changes

## Commit Strategy

### Commit Conventions

- Keep commits focused and atomic
- Write clear commit messages using Conventional Commits (https://www.conventionalcommits.org/en/v1.0.0/) style
- Use clear language, primarily English
- The subject SHOULD BE written in English whenever possible
- In the description, briefly explain the motivation for the change, not a line-by-line summary

## Branch Strategy

- main: For Production. DO NOT PUSH DIRECTORY.
- feature/<short-description>: Feature Developments
- fix/<short-description>: Bug Fixes
- bugfix/<short-description>: Bug Fixes (same as fix)
- release/<version>: Release Candidates. MUST BE REQURIED PR.

Create these branches from the main branch unless otherwise instructed.  
For example, fix branches for specific feature branches do not need to be create from develop.

## PR (Pull Request) Strategy

- Write subjects using Conventional Commits (https://www.conventionalcommits.org/en/v1.0.0/) style.
- All descriptions should be written in English by default
