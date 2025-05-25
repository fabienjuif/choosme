# DBUS

## methods

| name        | params             | description                                                                                       |
| ----------- | ------------------ | ------------------------------------------------------------------------------------------------- |
| open        | uri                | open given uri and might fallback to UI                                                           |
| status      | -                  | return the status of choosme: default browser set or not and list of applications (id,alias,icon) |
| set-default | -1 or [0123456789] | set the default browser to use on fallback, -1 means no default -> open the UI                    |
| kill        | -                  | exit                                                                                              |
