# seed-tools linux

Usage:

Copy seed-tool executable somewhere and create a config directory in the same folder for it.

Copy config.toml to the config directory

Run:

./seedtool <input_path> -SP -TL

Or only one or the other.

If paths are not defined in config file, screenshots/torrents folders will be created in the working dir.

If no qbit category is supplied in the config file, input_path will be used as save path.

./seedtool <input_path> -SP -0000

Non-video upload, skip all filechecks and processing. The -0000 argument will be used as category id and type id for upload. i.e pass -1614
to uploads a PC game.
