# CLI

## Mode & Args

### daemon

| mode   | arg                        | description                                                                                                        |
| ------ | -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| daemon |                            | start choosme daemon mode                                                                                          |
| daemon | --set-default [0123456789] | set the default browser (only for fallbacks) 0 == the first application found in the config, 2 the first one, etc |
| daemon | --unset-default            | unset the default browser and reset to default behaviour (printing the UI on fallbacking)                          |
| daemon | --status                   | print status in JSON format -useful for bars like ironbar or waybar-                                               |

### default mode

In default mode the binary try to act as a client, and if not able to connect to the daemon, fallback to local interpretation.

| arg | description                             |
| --- | --------------------------------------- |
| %u  | url to open, typically used by xdg-open |
