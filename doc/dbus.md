# DBUS

## methods

| name        | params                              | description                                                                    |
| ----------- | ----------------------------------- | ------------------------------------------------------------------------------ |
| open        | realm_id, uri                       | open given uri and might fallback to UI                                        |
| status      | realm_id                            | return the status of choosme: list of applications (id,alias,icon,is_default)  |
| set-default | realm_id, index: -1 or [0123456789] | set the default browser to use on fallback, -1 means no default -> open the UI |
| kill        | -                                   | exit                                                                           |
