## Installation

### From sources

1. Install dependencies

   - libgtk-4-dev
   - libadwaita-1-dev
   - libdbus-1-dev

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

# it auto selects chrome if
# - the URL starts with https://gmail.com
# - or we click on a google maps link
[[application]]
path = "~/.local/share/applications/chrome.desktop"
alias="Google" # this will be the row title instead of the .desktop Name
prefixes = [
    "https://gmail.com"
]
regexps = [
    "^https?://(www.)?google.(?:com|fr)/maps.*"
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

## Daemon mode

If you want to have a faster and/or having control over your fallback browser for your session, you can use the daemon mode.
Then you are still using the app as usual. Choosme will try to connect to the daemon, and if it fails run as a |standalone application.

Example for sway:

```
exec {
    choosme daemon
}
```

## Nice to have

- [ ] Auto set as default web browser on first run
- [ ] Auto detect browsers to init config file
- [ ] Be able to add or remove apps from the UI
- [ ] From the UI, have a drop down menu (hidden by default) where are presented the full URL (you can modify the URL to edit it) and the dns only, click on one of both, then you choose your app, it will be registred as your default app for this prefix
- [ ] Open window near cursor in Sway
- [ ] Enter opens the last used browser
- [ ] Change configuration logic to be

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
