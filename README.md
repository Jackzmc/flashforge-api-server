# Flashforge API

Based on [01F0/flashforge-finder-api](https://github.com/01F0/flashforge-finder-api), but written in rust.
I didn't like using flask for a "production server" and wanted to add some features.

Built with [Rocket](https://rocket.rs/).

The server by default listens on `localhost:8080`.

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

## Future Work

* [ ] Built in mjpeg proxy 
  * So multiple clients can view at once
* [x] Notifications (email, push?, webhooks?) on completion
  * [x] Email
  * [ ] Webhooks
  * [ ] Push?
  * Progress notifications
    * interval (every hour) or % based
  * Image snapshots in notifications
* [ ] Simple UI that replaces need of polar3d
* [x] Use config file for printer ips, instead of manually putting IP

# Usage

1. Build the project with `cargo build --release` or find a release
2. Copy `config.example.toml` to `config.toml` and configure it
   * All sections except [printers] are optional
3. Run target/release/flashforge-api or the binary file
   * The current directory must include the `config.toml` file
