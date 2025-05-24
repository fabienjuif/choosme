# DBUS

## methods

| name        | params             | description                                                                    |
| ----------- | ------------------ | ------------------------------------------------------------------------------ |
| toggle      | -                  | hide/show the window (UI)                                                      |
| status      | -                  | return the status of choosme: window open or not, default browser set or not   |
| set-default | -1 or [1234567890] | set the default browser to use on fallback, -1 means no default -> open the UI |
| kill        | -                  | exit                                                                           |
