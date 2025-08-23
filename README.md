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

## Synchronizing dotenv files

`azsync` synchronizes your local dotenv file with secrets stored in Azure. By
default, it looks for a Key Vault instance configured via the `KEY_VAULT_URL`
environment variable.

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

## License

This code is licensed under your choice of [MIT License](./LICENSE-MIT) or
[Apache License, Version 2.0](./LICENSE-APACHE).

[releases]: https://github.com/chkinney/azsync/releases
