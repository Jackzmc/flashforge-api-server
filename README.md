# Flashforge API

Based on [01F0/flashforge-finder-api](https://github.com/01F0/flashforge-finder-api), but written in rust.
I didn't like using flask for a "production server" and wanted to add some features.

Built with [Rocket](https://rocket.rs/).

The server by default listens on `localhost:8080`.

# Features

* Notifications on job completion (to email, or webhook such as Discord)
  * Including image of result
* API Wrappers (Info, Status, Temperature, Head Position, Progress)
* Camera Proxy
  * Allows multiple clients to view stream at once


## Tested Printers
Should work in theory on all the supported printers of flashforge-finder-api
* Flashforge Adventuer 5M Pro - Firmware v2.7.9

## API Doc

The `docs` folder includes documentation for use in [Bruno](https://www.usebruno.com/), set the `PRINTER` environment variable to that of your printers's id.

In general, for now:
* `http://localhost:8080/apis/printers` - Returns list of printer names
* `http://localhost:8080/apis/printers/:printerId/info` - Get printer info
* `http://localhost:8080/apis/printers/:printerId/status` - Get printer status
* `http://localhost:8080/apis/printers/:printerId/temperatures` - Get sensor temperatures, B for bed, T0 for main sensor
* `http://localhost:8080/apis/printers/:printerId/head-position` - Get the printer's head position
* `http://localhost:8080/apis/printers/:printerId/progress` - Get print progress
* `http://localhost:8080/apis/printers/:printerId/snapshot` - Get a single frame of printer's camera
* `http://localhost:8080/apis/printers/:printerId/camera` - See printer's camera live, supporting multiple clients viewing at once

## Getting Started

1. Build the project with `cargo build --release` or find a release
2. Copy `config.example.toml` to `config.toml` and configure it
    * All sections except [printers] are optional
3. Run target/release/flashforge-api or the binary file
    * The current directory must include the `config.toml` file

# Future Work

* [x] Notifications (email, push?, webhooks?) on completion
  * [x] Email
  * [x] Webhooks
  * [ ] Push?
  * [ ] Progress notifications
    * interval (every hour) or % based
  * [x] Image snapshots in notifications
* [ ] Simple UI that replaces need of polar3d
* [x] Write APIs
  * [x] Set temperature
  * [ ] Start / stop / pause file
  * [ ] Move head, bed

