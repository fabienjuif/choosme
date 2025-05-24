# CLI

## Mode & Args

### daemon

| mode   | arg                        | description                                                                                                        |
| ------ | -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| daemon | --start,--restart          | start or restart choosme daemon mode                                                                               |
| daemon | --show                     | ask to show the window                                                                                             |
| daemon | --hide                     | ask to hide the window                                                                                             |
| daemon | --toggle                   | ask to toggle the window between hide and show                                                                     |
| daemon | --set-default [1234567890] | set the default browser (only for fallbacks) 1 == the first application found in the config, 2 the second one, etc |
| daemon | --unset-default            | unset the default browser and reset to default behaviour (printing the UI on fallbacking)                          |
| daemon | --status                   | print status in JSON format -useful for bars like ironbar or waybar-                                               |

### default mode

In default mode the binary try to act as a client, and if not able to connect to the daemon, fallback to local interpretation.

| arg | description                            |
| --- | -------------------------------------- |
| %u  | url to open                            |
| -   | used by xdg-open, otherwise print help |
