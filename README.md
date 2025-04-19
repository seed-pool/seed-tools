# seed-tools (Linux)

## 📦 Usage

### 🔧 Setup

1. Copy the `seed-tools` executable somewhere on your system.
2. In the **same folder**, create a `config` directory.
3. Copy your `config.yaml` file into that `config` directory.
4. In the `config` directory create a `trackers` folder
5. Copy `seedpool.yaml` and `torrentlecch.yaml`files into that `trackers` folder.

### 🚀 Running the Tool

#### Video Uploads:
This assumes a Movie or TV show upload. It will fetch tmdb/imdb/tvdb ids, create a sample, create screenshots, generate a description with all these goodies linked, and upload to the requiered tracker(s) 
```bash
./seed-tools <input_path> -SP

./seed-tools <input_path> -SP -TL
```
#### 📤 Non-Video Upload (Skip All Checks)
Non-video upload, skip all filechecks and processing. The -0000 argument will be used as category id and type id for upload. i.e pass -1614 to uploads a PC game.
```
./seed-tools <input_path> -SP -0000
```

#### 🔄 Sync Mode (Cross-seeding)
Scans all torrents currently seeding in qBittorrent, checks Seedpool for matches, and adds any matches back to qBittorrent for cross-seeding.
```
./seed-tools -sync
```