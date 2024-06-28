# Holochain Scaffolding CLI

A command-line interface for creating and modifying a Holochain application (hApp).

```
USAGE:
    hc-scaffold [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -t, --template <template>    The template to use for the hc-scaffold commands Can either be an option from the
                                 built-in templates: "vanilla", "vue", "lit", "svelte", "react", "headless" Or a path to
                                 a custom template

SUBCOMMANDS:
    collection    Scaffold a collection of entries in an existing zome
    dna           Scaffold a DNA into an existing app
    entry-type    Scaffold an entry type and CRUD functions into an existing zome
    example       Scaffold an example hApp
    help          Prints this message or the help of the given subcommand(s)
    link-type     Scaffold a link type and its appropriate zome functions into an existing zome
    template      Manage custom templates
    web-app       Scaffold a new, empty web app
    zome          Scaffold one or multiple zomes into an existing DNA
```

See the full CLI reference [here](/guides/cli.md)

## Obtaining the scaffolding tool through holonix

The easiest way to start using the scaffolding tool is through holonix:

```bash
nix run github:holochain/holochain#hc-scaffold -- --version
```

Should print the version of the scaffolding tool.

## Usage

Refer to [the holochain developer instructions](https://developer.holochain.org/get-building/) to know how you can use the scaffolding tool to create your own apps.

These are the commands that you can run with the scaffolding tool inside of a holonix develop shell:

```bash
# Scaffold an example app
hc scaffold example

# Scaffold an empty web-app
hc scaffold web-app forum

cd forum

# Scaffold a dna inside the newly scaffolded app
hc scaffold dna forum

# Scaffold a zome inside the newly scaffolded dna
hc scaffold zome posts

# Scaffold an entry-type inside the newly scaffolded zome
hc scaffold entry-type post

# Scaffold a collection for the newly scaffolded entry-type
hc scaffold collection global all_posts

# Scaffold a new link-type
hc scaffold link-type
```

## Custom Templates

See the [docs.rs documentation](https://docs.rs/holochain_scaffolding_cli) to learn how to use and create custom templates.

## Contributing

We have a contributing [guide](guides/contributing.md) to help you get started. If you need anything else to get started please reach out on [Discord](https://discord.gg/k55DS5dmPH)!

## Maintenance

We have a maintenance [guide](guides/maintenance.md) which is mainly aimed at maintainers of the project but may be useful for some contributors or users to read.

## Manual installation

Install the CLI globally with this command.

```bash
cargo install holochain_scaffolding_cli
```
