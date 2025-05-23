xdg-settings set default-web-browser choosme.desktop

xdg-settings set default-web-browser userapp-Firefox-OOAD52.desktop

## Shortcuts

- `1` open the first row
- `2` open the 2nd row
- etc

## TODOs

- [ ] Support regexp for each `[[application]]`
- [ ] Close on Escape
- [ ] CSS from a path (XDG)

## Great to have

- [ ] Automatically choose from the source app, if discord -> this browser, if slack -> this browser

## Nice to have

- [ ] Auto set as default web browser on first run
- [ ] Auto detect browsers to init config file
- [ ] Be able to add or remove apps from the UI
- [ ] From the UI, have a drop down menu (hidden by default) where are presented the full URL (you can modify the URL to edit it) and the dns only, click on one of both, then you choose your app, it will be registred as your default app for this prefix
- [ ] Open window near cursor in Sway

`~/.config/choosme/config.toml`

```toml
[[application]]
path = "/usr/share/applications/firefox.desktop"
prefixes = ["http://google.fr"]

[[application]]
path = "/usr/local/share/applications/chrome.desktop"
prefixes = ["https://*gmail.com"]
```
