# Rust project template

[![CI](https://github.com/clechasseur/rust-template/actions/workflows/ci.yml/badge.svg?branch=main&event=push)](https://github.com/clechasseur/rust-template/actions/workflows/ci.yml) [![codecov](https://codecov.io/gh/clechasseur/rust-template/branch/main/graph/badge.svg?token=qSFdAkbb8U)](https://codecov.io/gh/clechasseur/rust-template) [![Security audit](https://github.com/clechasseur/rust-template/actions/workflows/audit-check.yml/badge.svg?branch=main)](https://github.com/clechasseur/rust-template/actions/workflows/audit-check.yml) [![crates.io](https://img.shields.io/crates/v/rust-template-clp.svg)](https://crates.io/crates/rust-template-clp) [![downloads](https://img.shields.io/crates/d/rust-template-clp.svg)](https://crates.io/crates/rust-template-clp) [![docs.rs](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/rust-template-clp) [![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)

This is a simple template repository for Rust projects that includes some default workflows and configuration files.

## TL;DR

1. Create a new repository using this template (_Note_: do not include all branches, unless you want to end up with the test branch)
2. Clone your new repository
3. Run `cargo init` to create a Rust project at the repository root<br/>
   OR<br/>
   Run `cargo new <project>` from the repository root to create a new Rust project, then create a root `Cargo.toml` to setup a [workspace](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
4. Adjust workflows as required
5. Adjust/add/remove status badges in this README
6. Adjust links in [CONTRIBUTING.md](./CONTRIBUTING.md), [DEVELOPMENT.md](./DEVELOPMENT.md), [SECURITY.md](./SECURITY.md) and [PULL_REQUEST_TEMPLATE.md](./.github/PULL_REQUEST_TEMPLATE.md)
7. ???
8. Profit!

## More info

This template is of course opinionated, but the following sections present how it is meant to be used.
(This documentation assumes that you are familiar with Rust development; if not, you can refer to the [Cargo book](https://doc.rust-lang.org/cargo/) for more information on how to setup and manage Rust projects.)

### Required tools

In addition to a stable Rust toolchain, the template uses the following tools:

* A Nightly Rust toolchain
* [`just`](https://github.com/casey/just) (command runner, a `make` of sorts)
* [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) (code coverage tool)
* [`cargo-hack`](https://github.com/taiki-e/cargo-hack) (CI helper, used by other tools)
* [`cargo-minimal-versions`](https://github.com/taiki-e/cargo-minimal-versions) (MSRV helper tool)
* [`cargo-msrv-prep`](https://github.com/clechasseur/msrv-prep) (MSRV helper tool)
* [`cargo-msrv`](https://github.com/foresterre/cargo-msrv) (tool to determine MSRV)

### `just`

The template uses the [`just`](https://github.com/casey/just) command runner to define some build commands.
This makes it easier to run common commands without having to remember any project-specific flags that might need to be passed.
`just` commands are stored in a [`justfile`](./justfile) and are called _recipes_.

Running `just` without argument will print the list of available recipes.
The following table lists the most interesting ones.

| Command              | Purpose                                                                                                    |
|----------------------|------------------------------------------------------------------------------------------------------------|
| `just tidy`          | Run `clippy` and `rustfmt` on the project's code<sup>1</sup>                                               |
| `just check`         | Run `cargo check` on all workspace projects                                                                |
| `just build`         | Run `cargo build` on all workspace projects                                                                |
| `just test`          | Run `cargo test` for all tests in workspace                                                                |
| `just llvm-cov`      | Run `cargo llvm-cov` to execute tests with code coverage (see below)<sup>1</sup>                           |
| `just update`        | Run `cargo update`                                                                                         |
| `just doc`           | Generate documentation with `rustdoc`; when run locally, opens the resulting doc when done (with `--open`) |
| `just doc-coverage`  | Produce a doc coverage report (an experimental `rustdoc` feature)<sup>1</sup>                              |
| `just msrv`          | Determine the MSRV of `lib` and `bin` crates only using `cargo-msrv` (see below)                           |
| `just msrv-full`     | Determine the entire project's MSRV using `cargo-msrv` (see below)                                         |
| `just check-minimal` | Validate the minimal MSRV determined with `just msrv-minimal` (see below)                                  |
| `just test-package`  | Run `cargo publish --dry-run` to test package publication                                                  |

<sup>1</sup> : these commands use Nightly Rust.

The `justfile` also uses variables to determine what to run and what arguments to pass.
These can be overridden when calling `just`, however.
For example, you can override `toolchain` to run a command with another Rust toolchain:

```sh
just toolchain=nightly test
```

There are other variables as well; check out the beginning of the [`justfile`](./justfile) for details.

### Workflows

The template includes some GitHub workflows to perform common CI/CD tasks.

| File                                                     | Triggers on...                                             | Purpose                                                                                       |
|----------------------------------------------------------|------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| [`audit-check.yml`](./.github/workflows/audit-check.yml) | `push` (to `main`), `pull_request`, `schedule` (every day) | Run security audits on the project's dependencies using [`cargo-audit`](https://rustsec.org/) |
| [`ci.yml`](./.github/workflows/ci.yml)                   | `push` (to `main`), `pull_request`                         | All CI-related tasks: linting, running tests, checking MSRV, etc.                             |
| [`publish.yml`](./.github/workflows/publish.yml)         | `push` (a version tag)                                     | Publishes workspace crates using `cargo publish`                                              |
| [`release.yml`](./.github/workflows/release.yml)         | `release` (`created` only)                                 | Build release binaries and attach them to a GitHub release                                    |

By default, workflows are disabled (except for manual triggering).
To enable them, edit the corresponding file to uncomment the appropriate event at the top of the file.
There are also places where you will need to edit the files depending on your project's Rust version, etc.

If you don't need one of the workflow (such as `release.yml` if your project does not publish binaries), you can simply delete the file.

### Renovate

The template includes a [Renovate config file](./renovate.json) that can be used with the [Renovate bot](https://github.com/marketplace/renovate) to check your project's dependencies for updates.
By default, the config file points to a [shared config preset](https://github.com/clechasseur/renovate-config/blob/main/renovate.json), but you can change it for your needs if required (or delete the file if you don't use Renovate).

### `rustfmt`

The template includes a [`rustfmt.toml`](./rustfmt.toml) file to configure the `rustfmt` tool.
This tool is a Rust code formatter; it can be executed via

```sh
just fmt
```

or in combination with `clippy` via

```sh
just tidy
```

The file contains configuration values that deviate from the defaults, but they require the use of a Nightly Rust toolchain to use them.
If you do not use `rustfmt`, you can simply delete the config file.

### Code coverage

The template includes support for running tests with coverage using [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov).
It's possible to run tests with coverage locally using

```sh
just llvm-cov
```

The [`ci.yml`](./.github/workflows/ci.yml) workflow also includes support for uploading coverage results to [codecov.io](https://codecov.io/).
Coverage settings are controlled through the [`codecov.yml`](./codecov.yml) file (the coverage target, for example).
In order to use this, you will need to link Codecov with your GitHub account; for more information, see Codecov's [GitHub tutorial](https://docs.codecov.com/docs/github-tutorial).
(Also see [this warning](https://github.com/codecov/codecov-action?tab=readme-ov-file#dependabot) in order to allow proper coverage checks in Dependabot-created PRs.)

### MSRV

MSRV stands for _Minimum Supported Rust Version_.
Lots of crates advertise their MSRV so that users can determine whether they can use the dependency in their own projects.
It's also possible to specify the project's MSRV in your `Cargo.toml` file through the [`rust-version` field](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field).

The template includes support for determining and validating the project's MSRV.

#### Determining the MSRV for crate users

In order to determine the minimal Rust version needed to build your crate from a user perspective, you need to check `lib` and `bin` crates only.
If you installed all required tools, this can be determined by running

```sh
just msrv
```

This Rust version can then be configured in the CI workflow (see [`ci.yml`](./.github/workflows/ci.yml)'s `msrv-check` job).

#### Determining the MSRV for the project iself

In order to determine the minimal Rust version that can be used to build the project _itself_ (including any tests, examples, etc.), you can use

```sh
just msrv-full
```

This Rust version can then be configured in the CI workflow (see [`ci.yml`](./.github/workflows/ci.yml)'s `build` job).

#### Validating the MSRV locally

To validate the project's MSRV locally, you can use the `check-minimal` recipe.
Assuming your project's MSRV is Rust 1.63.0, run

```sh
just toolchain=1.63.0 check-minimal
```

### Other stuff

The template comes with a few more files to set you up for publishing an open-source Rust project, including:

* [Issue templates](./.github/ISSUE_TEMPLATE) ([bug report](./.github/ISSUE_TEMPLATE/bug_report.md), [feature request](./.github/ISSUE_TEMPLATE/feature_request.md))
* [Pull request template](./.github/PULL_REQUEST_TEMPLATE.md)
* [`CODEOWNERS` file](./.github/CODEOWNERS) (see [About code owners](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners) on GitHub)
* [Code of Conduct](./CODE_OF_CONDUCT.md) (uses the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/))
* [Contributing guidelines](./CONTRIBUTING.md)
* [Development guide](./DEVELOPMENT.md) (for people who want to contribute to the project)
* [License](./LICENSE) (MIT license)
* [Security policy](./SECURITY.md) (including a list of versions that are still supported and for which updates might be released to fix vulnerabilities)
* [`.gitignore` file](./.gitignore) (with some Rust-oriented ignores)
* [`.dockerignore` skeleton file](./.dockerignore) (in case you need to build Docker files for your project)

You can remove any file you do not need; those that you keep might need to be adapted, especially those containing project links.

## Questions? Comments?

If you notice a problem in the template or would like to suggest an improvement, you can create an [issue](https://github.com/clechasseur/rust-template/issues/new).
