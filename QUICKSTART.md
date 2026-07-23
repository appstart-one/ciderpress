# CiderPress Quick Start Guide

## System Requirements

- **macOS 11.0+** (Big Sur or later)
- **Apple Silicon** (M1, M2, M3, or M4)

---

## Step 1: Install CiderPress

1. Open the `.dmg` file you downloaded.
2. Drag **CiderPress.app** into your **Applications** folder.

---

## Step 2: Allow CiderPress to Run

Because CiderPress is not yet signed with an Apple Developer certificate, macOS will block it the first time you try to open it. Use **one** of the two methods below.

### Method A: System Settings (Recommended)

1. Double-click **CiderPress** in your Applications folder. macOS will show a warning and refuse to open it.
2. Open **System Settings** > **Privacy & Security**.
3. Scroll down. You will see a message: *"CiderPress was blocked from use because it is not from an identified developer."*
4. Click **Open Anyway**.
5. When prompted again, click **Open**.

### Method B: Terminal Command

1. Open **Terminal** (found in Applications > Utilities).
2. Paste the following command and press Enter:
   ```
   xattr -cr /Applications/CiderPress.app
   ```
3. Open CiderPress normally from your Applications folder.

> This command removes the macOS quarantine flag that gets applied to all apps downloaded from the internet. It is safe and only affects the CiderPress app.

---

## Step 3: Grant Access to Your Voice Memos

CiderPress needs to read Apple's Voice Memos database, which is stored in a protected system directory. Choose **one** of the two options below.

### Option A: Select the Voice Memos Folder (Recommended)

This lets CiderPress read your Voice Memos directly from where Apple stores them. Approving the folder in the selection dialog is what grants CiderPress read access to just that folder — nothing else on your disk.

1. Open **CiderPress**. On first launch it takes you to **Settings** and prompts you to choose your Voice Memos folder. (You can also click **Choose Folder…** on the Settings page at any time.)
2. The folder dialog opens at Apple's Voice Memos location. Click **Open**.
3. Done — the access grant persists across restarts.

### Option B: Manually Copy Your Voice Memos

If you prefer, you can copy the files yourself instead.

1. Open **Finder**.
2. Press **Cmd + Shift + G** and paste this path:
   ```
   ~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings
   ```
3. Copy the contents of that folder to a location of your choice (e.g., `~/VoiceMemos`).
4. In CiderPress, open the **Settings** page and set the Voice Memo path to the folder where you copied the files.

---

## Step 4: Start Using CiderPress

1. Open **CiderPress** from your Applications folder.
2. Go to **Settings** and choose a Whisper transcription model (start with `base.en` if unsure).
3. Go to **Migrate** and click **Start Migration** to copy your voice memos into CiderPress.
4. Go to **Slices** to browse, transcribe, search, and export your recordings.
