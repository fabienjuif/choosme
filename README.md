## Installation

### From sources

1. Install dependencies

   - libgtk-4-dev
   - libadwaita-1-dev

2. Install rust: https://www.rust-lang.org/tools/install
3. Compile and install the app:

```sh
cargo install --path .
```

4. Add `~/.cargo/bin/` to your PATH

## First run

1. Create a config file (see [Config](#config))
2. Copy the choosme desktop file:

```sh
cp ./choosme.desktop ~/.local/share/applications/
```

3. Set choosme as your default browser:

```sh
xdg-settings set default-web-browser choosme.desktop
```

## Shortcuts

- `Escape` to close
- `1` open the first row
- `2` open the 2nd row
- etc

## Config

`~/.config/choosme/config.toml`

```toml
# this app is never auto selected
[[application]]
path = "/usr/share/applications/firefox.desktop"

# it auto selects chrome if the URL starts with https://gmail.com
[[application]]
path = "~/.local/share/applications/chrome.desktop"
prefixes = [
    "https://gmail.com"
]

# if you click to any link that is not gmail.com, it'll open choosme UI.
# you then have to choose between Firefox and Chrome to open this link.
```

## Styling

You can override the styling creating a CSS file here: `.config/choosme/style.css`.

```css
/* main window */
.main-window {
}
/* list of items */
.boxed-list {
}
/* a row / item */
.row {
}
```

## TODOs

- [ ] Support regexp for each `[[application]]`
- [ ] Speed up start (maybe doing a daemon?)
- [ ] Change configuration logic to be
- [ ] Enter opens the last used browser

```toml
[[rule]]
prefix="http://github.com/fabienjuif"
application="Firefox"

# this rule apply only if the first one is not matching
[[rule]]
prefix="http://google.com"
application="Chrome"

# optional fallback rule if you want to avoid the UI to pop
[[rule]]
default="Firefox"

[[application]]
path=
name="Firefox"

[[application]]
path=
name="Chrome"
```

## Great to have

- [ ] Automatically choose from the source app, if discord -> this browser, if slack -> this browser

## Nice to have

- [ ] Auto set as default web browser on first run
- [ ] Auto detect browsers to init config file
- [ ] Be able to add or remove apps from the UI
- [ ] From the UI, have a drop down menu (hidden by default) where are presented the full URL (you can modify the URL to edit it) and the dns only, click on one of both, then you choose your app, it will be registred as your default app for this prefix
- [ ] Open window near cursor in Sway
