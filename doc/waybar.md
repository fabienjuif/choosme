# Waybar

Here's a custom configuration for waybar

```json
    "custom/choosme": {
        "format": "{icon} {}",
        "format-icons": {
            "perso": "<span foreground='orange'>󰈹 </span>",
            "work": "<span foreground='blue'> </span>",
            "no-default": "<span foreground='green'>󱞒 </span>"
        },
        "interval": 5,
        "tooltip": false,
        "return-type": "json",
        "exec": "choosme daemon --waybar",
        "on-click": "choosme daemon --set-default-next",
        "on-click-right": "choosme daemon --unset-default"
    },
```
