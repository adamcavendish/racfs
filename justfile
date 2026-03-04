# RACFS Justfile
# Modular command runner for development tasks

mod build 'just/build.just'
mod test 'just/test.just'
mod lint 'just/lint.just'
mod run 'just/run.just'
mod dev 'just/dev.just'
mod doc 'just/doc.just'

# Default recipe - show available commands
default:
    @just --list --list-submodules

# Coverage shortcuts (delegate to test module)
coverage:
    just test coverage

coverage-html:
    just test coverage-html
