# Flashforge API

Based on [01F0/flashforge-finder-api](https://github.com/01F0/flashforge-finder-api), but written in rust.
I didn't like using flask for a "production server" and wanted to add some features.

Built with rocket, this branch (barebones), will not include any other features, just a direct JSON wrapper of the printer's TCP server

## Tested Printers
Should work in theory on all the supported printers of flashforge-finder-api
* Flashforge Adventuer 5M Pro

## API Doc

The `docs` folder includes documentation for use in [Bruno](https://www.usebruno.com/), replace the ip with your server's

These are the following APIs:
* `http://localhost:8080/<printer ip>/info` - Get printer info
* `http://localhost:8080/<printer ip>/status` - Get printer status
* `http://localhost:8080/<printer ip>/temperature` - Get sensor temperatures, B for bed, T0 for main sensor
* `http://localhost:8080/<printer ip>/head-position` - Get the printer's head position
* `http://localhost:8080/<printer ip>/progress` - Get print progress

## Future Work

See the master branch if you want more features