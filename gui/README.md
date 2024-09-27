# Liana GUI

The Liana graphical interface.

## Dependencies

You will need a few dependencies in order to run correctly this software. For Linux systems, those
are:
- [`udev`](https://www.freedesktop.org/software/systemd/man/udev.html) (On Debian/Ubuntu `apt install udev`)
- [`fontconfig`](https://www.freedesktop.org/wiki/Software/fontconfig/) for access to fonts (On Debian/Ubuntu `apt install libfontconfig1-dev`)
- [`libudev-dev`](https://www.freedesktop.org/software/systemd/man/libudev.html) to communicate with devices through USB (On Debian/Ubuntu `apt install libudev-dev`)

In addition, if you want to build the project from source, you will need:
- [`pkg-config`](https://www.freedesktop.org/wiki/Software/pkg-config/) (On Debian/Ubuntu `apt install pkg-config`)
- `cmake` (On Debian/Ubuntu apt install cmake)


## Usage

*For a quick guide to try out the software see [../doc/TRY.md](../doc/TRY.md).*

```
liana-gui --datadir <datadir> --<network>
```

The default `datadir` is the same as for `lianad` (`~/.liana` for Linux). The default network is
Bitcoin mainnet, but testnet signet and regtest are supported.

If the software is started with no parameter and no data directory is detected, a Liana installer
will be spawned that will guide you in the processing of configuring Liana.

If the software is started and a reachable `lianad` is running, it will plug to it via `lianad`'s
JSONRPC interface.

The environment variable `LOG_LEVEL` with values `error`, `warn`, `info`, `debug`, `trace`, overrides the log settings from the config file.

### Troubleshooting

- If you encounter layout issue on `X11`, try to start the GUI with `WINIT_X11_SCALE_FACTOR`
  manually set to 1
