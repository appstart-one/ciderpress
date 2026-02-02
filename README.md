# CiderPress - Voice Memo Liberator

A desktop application built with Tauri and React that liberates your Apple Voice Memos by copying them to a local database, enabling transcription, search, and export capabilities.

## Features

- **Settings**: Configure paths to Apple's Voice Memos and CiderPress data directory
- **Migration**: Copy voice memos from Apple's database to CiderPress
- **Statistics**: View analytics about your voice memo collection
- **Slices**: Browse, search, transcribe, and export your voice memos

## Tech Stack

- **Backend**: Rust with Tauri v2
- **Frontend**: React with TypeScript and Tailwind CSS
- **Database**: SQLite 3
- **Build Tool**: Vite
- **Transcription**: Simple-whisper (placeholder implementation)
- **Audio Processing**: FFmpeg (statically linked — no external installation required)

## Project Structure

```
ciderpress/
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── backend/     # Core business logic
│   │   │   ├── config.rs    # Configuration management
│   │   │   ├── database.rs  # SQLite operations
│   │   │   ├── migrate.rs   # Apple DB migration
│   │   │   ├── transcribe.rs # Transcription engine
│   │   │   ├── stats.rs     # Statistics collection
│   │   │   └── models.rs    # Data structures
│   │   ├── lib.rs       # Tauri commands and app state
│   │   └── main.rs      # Entry point
│   └── Cargo.toml       # Rust dependencies
├── src/                 # React frontend
│   ├── pages/           # Page components
│   │   ├── Settings.tsx
│   │   ├── Migrate.tsx
│   │   ├── Stats.tsx
│   │   └── Slices.tsx
│   ├── App.tsx          # Main app component
│   ├── main.tsx         # React entry point
│   └── index.css        # Tailwind styles
├── package.json         # Node.js dependencies
├── vite.config.ts       # Vite configuration
├── tailwind.config.js   # Tailwind configuration
└── tsconfig.json        # TypeScript configuration
```

## Requirements

- macOS 11.0+ (Big Sur or later)

## Development Setup

### Prerequisites

- macOS 11.0+ (Big Sur or later)
- Rust 1.78+
- Node.js 18+
- npm or pnpm

### Installation

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd ciderpress
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Install Rust dependencies:
   ```bash
   cd src-tauri
   cargo build
   cd ..
   ```

### Development

Run the development server:
```bash
npm run tauri dev
```

This will start both the Vite dev server and the Tauri application.

### Building

Build for production:
```bash
npm run tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

## Configuration

CiderPress stores its configuration in `~/.ciderpress/ciderpress-settings.toml`:

```toml
voice_memo_root = "/Users/username/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings"
ciderpress_home = "/Users/username/.ciderpress"
model_name = "base.en"
first_run_complete = true
```

## Database Schema

### Recordings Table
Stores metadata about copied voice memos:
- `id`: Primary key
- `apple_id`: Original Apple database ID
- `created_at`: Unix timestamp
- `duration_sec`: Duration in seconds
- `title`: Recording title (optional)
- `original_path`: Path to original file
- `copied_path`: Path to copied file
- `file_size`: File size in bytes
- `mime_type`: MIME type (default: audio/m4a)
- `year`: Year for indexing

### Transcripts Table
Stores transcription results:
- `id`: Primary key
- `recording_id`: Foreign key to recordings
- `model`: Whisper model used
- `started_at`: Transcription start time
- `finished_at`: Transcription end time
- `word_count`: Number of words
- `text_path`: Path to transcript file
- `success`: Success flag
- `error_message`: Error details (if any)

## API Commands

The Rust backend exposes these Tauri commands:

- `get_config()`: Get current configuration
- `update_config(config)`: Update configuration
- `validate_paths()`: Validate Apple Voice Memo paths
- `migrate()`: Copy files from Apple's database
- `get_stats()`: Get collection statistics
- `list_recordings(limit, offset)`: List recordings with pagination
- `search_recordings(query, limit, offset)`: Search recordings
- `transcribe_many(recording_ids)`: Transcribe selected recordings
- `export_audio(recording_ids, dest_dir, reencode)`: Export audio files
- `pick_directory()`: Open directory picker dialog

## Roadmap

- [ ] **M1**: Project bootstrap ✅
- [ ] **M2**: Apple VM root detection ✅
- [ ] **M3**: Migration engine ✅
- [ ] **M4**: Stats service ✅
- [ ] **M5**: Slices grid ✅
- [ ] **M6**: Transcription backend (placeholder implemented)
- [ ] **M7**: Bulk transcription UI ✅
- [ ] **M8**: Export audio ✅
- [ ] **M9**: NotebookLM prototype (planned)

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License
## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

You are free to use, modify, and distribute this software under the terms of the GPL v3.0. Any derivative works must also be licensed under GPL v3.0.

## Security & Privacy

- Only copies data; never deletes or edits Apple originals
- All data stored locally in user's home directory
- No network calls except optional model downloads
- Proper file permissions (0o700 for directories, 0o600 for files) 