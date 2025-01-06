# Flashforge API

Based on [01F0/flashforge-finder-api](https://github.com/01F0/flashforge-finder-api), but written in rust.
I didn't like using flask for a "production server" and wanted to add some features.

Built with rocket, but that might change

## Tested Printers
Should work in theory on all the supported printers of flashforge-finder-api
* Flashforge Adventuer 5M Pro

## API Doc

The `docs` folder includes documentation for use in [Bruno](https://www.usebruno.com/), replace the ip with your server's

In general, for now:
* `http://localhost:8080/apis/printers/:printerId/info` - Get printer info
* `http://localhost:8080/apis/printers/:printerId/status` - Get printer status
* `http://localhost:8080/apis/printers/:printerId/temperatures` - Get sensor temperatures, B for bed, T0 for main sensor
* `http://localhost:8080/apis/printers/:printerId/head-position` - Get the printer's head position
* `http://localhost:8080/apis/printers/:printerId/progress` - Get print progress

## Future Work

* [ ] Built in mjpeg proxy 
  * So multiple clients can view at once
* [ ] Notifications (email, push?, webhooks?) on completion
* [ ] Simple UI that replaces need of polar3d
* [x] Use config file for printer ips, instead of manually putting IP