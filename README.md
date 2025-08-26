# azsync

Quickly synchronize local secrets with Azure.

`azsync` compares secrets stored locally and in Azure, and synchronizes them so
that they match. It supports pushing and pulling, and can compare when they were
modified to decide which direction to synchronize in per-secret.

To get started, run `azsync --help`.

## Installation

Download prebuilt binaries from the [releases] page.

To build from source:

```shell
cargo install --git https://github.com/chkinney/azsync
```

### Completions

For shell completions, run `azsync completions [shell]`. Completions are
supported for the following shells:

- Bash (`bash`, default on Linux)
- Powershell (`pwsh`, default on Windows)
- Zsh (`zsh`, default on MacOS)
- Fish (`fish`)
- Elvish (`elvish`)
- Nushell (`nushell`)

The completions script will be output to stdout. Save it to a location
appropriate for your shell.

## Synchronizing dotenv files

`azsync dotenv` synchronizes your local dotenv file with secrets stored in
Azure. By default, it looks for a Key Vault instance configured via the
`KEY_VAULT_URL` environment variable.

> [!TIP]
> You can save `KEY_VAULT_URL` in your dotenv file and `azsync` will use it.

Run `azsync dotenv` to synchronize your secrets automatically.

Can't write secrets in Key Vault? Configure which direction values are
synchronized in with `-m`. Use `azsync dotenv -m pull` if you only want to pull
newer values from Azure without pushing any values. Use `pull-always` instead to
always pull the latest values from Azure even if they're older than your local
values.

If you have a `.env.example` file, `azsync` will read that file to determine
which variables to synchronize instead. This way, you can control which
variables are synchronized to avoid pushing/pulling values you don't want
affected. You can even have `azsync` generate a dotenv file for you
automatically based on it!

## Synchronizing other files

`azsync file` synchronizes any file with a blob stored in an Azure storage
account container. By default, it looks for the following environment variables:

- `STORAGE_ACCOUNT_URL`: blob storage endpoint
  - This can be found under Settings -> Endpoint in Azure Portal
- `STORAGE_ACCOUNT_CONTAINER`: container name

The blob name, by default, is the name of the file being synchronized.

> [!TIP]
> Similar to `azsync dotenv`, these variables can be loaded from a dotenv file!

`azsync file` ensures that whichever version is newer (local vs. remote) is
synchronized to both locations. This can be used to quickly share a file with
another person.

Note that the contents of the files will not be compared, only the modified
times. When pulling or pushing, `azsync` makes sure that the correct modified
time is stored. This ensures that the files can be compared quickly without
needing to save them both to disk or load either of them fully in memory.

## License

This code is licensed under your choice of [MIT License](./LICENSE-MIT) or
[Apache License, Version 2.0](./LICENSE-APACHE).

[releases]: https://github.com/chkinney/azsync/releases
