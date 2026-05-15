# sfont-manager

A terminal UI for browsing a soundfont library, building profiles, comparing them, and exporting them.

## Build

```bash
cd sfont-manager
cargo build --release
```

## Usage

```bash
# Run against your soundfont library folder
# Defaults to ~/Documents/Lightsabers/SoundFonts
./sfont-manager /path/to/your/library
```

The program scans for **all proffie SoundFonts** in the library folder — (every soundfont has a config.ini file).
