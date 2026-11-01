# osu! radio

Listen to your osu! beatmaps outside the game. osu! radio is a desktop music
player for local osu!stable and osu!lazer libraries, built with Rust and Qt.

The project is in **alpha**. Linux and Windows are the current targets; macOS
is not supported.

## Plans

Here's what I'm planning to work on:

- Rework shuffle.
- Make it easy to self-host a server and listen from other devices.
- Add video and storyboard playback.
- Match osu!'s search behavior 1:1.
- Play hitsounds along with the song.
- Show tracks recently played in osu! (currently, this only tracks plays in osu! radio).
- Make importing faster.
- Add autoplay based on tags.

## Install

### Arch Linux

Build the development version from the AUR:

```sh
git clone https://aur.archlinux.org/osu-radio-git.git
cd osu-radio-git
makepkg -si
```

Or install `osu-radio-git`/`osu-radio-bin` with an AUR helper such as `yay` or `paru`. 

### Windows

Head to the latest [pre-release](https://github.com/Jayko321/osu-radio/releases),
download the `osu-radio-setup.exe` installer, and follow the instructions.

## Get started

1. Launch **osu! radio** from your application menu, or run `osu-radio` if you
   installed an Arch package.
2. Open **Settings**, then use the **+** button next to **osu! folders** to find
   or select an osu! installation. Choose the folder and apply the selection to
   import its library.
3. Open **Songs**, select a track and press Play. You can also create playlists
   and add tracks to the queue.

The app starts its local server automatically. Your osu! source library is read
for import; the application stores its own library data separately.

## Build from source

You need:

- Rust installed through [rustup](https://rustup.rs/); the repository selects
  the required toolchain through `rust-toolchain.toml`.
- The .NET 10 SDK and runtime for the osu!lazer Realm helper, which Cargo builds
  automatically as part of the server build.
- Qt 6.8 or newer with development tools, Qt Quick/QML modules and a compatible
  C++ toolchain. Linux also needs ALSA headers, `pkg-config` and libclang;
  Windows needs the Windows SDK, including `rc.exe`.

Clone the repository and build both executables:

```sh
git clone https://github.com/Jayko321/osu-radio.git
cd osu-radio
cargo build --locked -p osu-radio-server -p osu-radio-qt
cargo run --locked -p osu-radio-qt
```

The GUI finds the server next to its executable. Qt libraries and plugins must
also be available at runtime.

## FAQ

**Do I need osu! installed?** Duh, yes you do, silly. I'm working on making it easy to host your library
on your PC and listen from your other devices.

**Why does building require .NET?** The scanner uses a small .NET helper to read
osu!lazer's Realm database. Cargo builds it automatically. Prebuilt packages may
have different runtime requirements; check their installation instructions.

## Contributing and license

Bug reports, ideas, and contributions are welcome! If you don't have a GitHub
account, feel free to DM me on Discord at **@jayko321** or just ping on [BTMC TALENT SERVER](https://discord.gg/7aAXEGAevP). See [CONTRIBUTING.md](CONTRIBUTING.md)
for the development workflow. The project is licensed under the [MIT License](LICENSE).
