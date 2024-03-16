<br>

<div align="center">
    <img src="https://raw.githubusercontent.com/daimond113/pesde/master/website/static/logo.svg" alt="pesde" width="200" />
</div>

<br>

pesde is a package manager for Roblox that is designed to be feature-rich and easy to use.
Currently, pesde is in a very early stage of development, but already supports the following features:

- Managing dependencies
- Re-exporting types
- `bin` exports (ran with Lune)
- Patching packages

## Installation

pesde can be installed from GitHub Releases. You can find the latest release [here](https://github.com/daimond113/pesde/releases).
It can also be installed by using [Aftman](https://github.com/LPGhatguy/aftman).

## Usage

pesde is designed to be easy to use. Here are some examples of how to use it:

```sh
# Initialize a new project
pesde init

# Install a package
pesde add daimond113/pesde@0.1.0

# Remove a package
pesde remove daimond113/pesde

# List outdated packages
pesde outdated

# Install all packages
pesde install

# Search for a package
pesde search pesde

# Run a binary
pesde run daimond113/pesde

# Run a binary with arguments
pesde run daimond113/pesde -- --help
```

## Preparing to publish

To publish you must first initialize a new project with `pesde init`. You can then use the other commands to manipulate dependencies, and edit the file
manually to add metadata such as authors, description, and license.

> **Warning**  
> The pesde CLI respects the `.gitignore` file and will not include files that are ignored. The `.pesdeignore` file has more power over the `.gitignore` file, so you can unignore files by prepending a `!` to the pattern.

The pesde CLI supports the `.pesdeignore` file, which is similar to `.gitignore`. It can be used to include or exclude files from the package.

## Registry

The main pesde registry is hosted on [fly.io](https://fly.io). You can find it at https://registry.pesde.daimond113.com.

### Self-hosting

You can self-host the registry by using the default implementation in the `registry` folder, or by creating your own implementation. The API
must be compatible with the default implementation, which can be found in the `main.rs` file.

## Previous art

pesde is heavily inspired by [npm](https://www.npmjs.com/), [pnpm](https://pnpm.io/), [Wally](https://wally.run), and [Cargo](https://doc.rust-lang.org/cargo/).
