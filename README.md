# mail-todo

Yet another to-do list, backed by an IMAP email account.

It will monitor an specific folder of the provided IMAP email account, and show an entry for each email in that folder. Those emails (from now on, tasks), can be deleted by marking them in the graphical interface and clicking "Delete". It's up to you to make the emails get to that folder (manually moving them, an automated rule, ...).

The `--config` option is mandatory, and it's expected to point to a file in the "mutt" format. That is:
```sh
set imap_user=USER
set imap_pass=PASS
set folder=imaps://whatever.server:993
```

The `--folder` option is optional and defaults to `ToDo`. That's the folder in the IMAP account that will be monitored and modified by `mail-todo`.

It uses `env_logger`, which means you can set the logging level via the `RUST_LOG` environment variable:
```sh
RUST_LOG=debug mail-todo --config .path/to/config
```

A systemd service unit file is included in case you want to use it as a user service. In such case you can run the following commands to enable it:
```sh
mkdir -p ~/.config/systemd/user
cp mail-todo.service ~/.config/systemd/user
systemctl --user daemon-reload
systemctl --user enable mail-todo
systemctl --user start mail-todo
```

## To build from Ubuntu 18.04
```sh
apt install build-essential libssl-dev libgtk-3-dev libdbus-1-dev
```
