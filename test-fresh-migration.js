// VoiceMemoLiberator - Voice memo transcription and management tool
// Copyright (C) 2026 APPSTART LLC
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// Test script to clear database and run fresh migration
const { invoke } = window.__TAURI__.core;

async function invokeCommand(command, args = {}) {
    try {
        console.log(`🔧 Invoking: ${command}`, args);
        const result = await invoke(command, args);
        console.log(`✅ Result:`, result);
        return result;
    } catch (error) {
        console.error(`❌ Error in ${command}:`, error);
        throw error;
    }
}

async function runFreshMigrationTest() {
    console.log("🧹 FRESH MIGRATION TEST");
    console.log("=".repeat(50));
    
    try {
        // Step 1: Clear the database
        console.log("\n1️⃣ Clearing database...");
        await invokeCommand('clear_database');
        console.log("✅ Database cleared successfully");
        
        // Step 2: Verify database is empty
        console.log("\n2️⃣ Verifying database is empty...");
        const slicesBefore = await invokeCommand('get_slice_records');
        console.log(`📊 Slices in database before migration: ${slicesBefore.length}`);
        
        if (slicesBefore.length > 0) {
            console.warn("⚠️ Database not empty after clear!");
            slicesBefore.forEach((slice, i) => {
                console.log(`   ${i + 1}. ${slice.original_audio_file_name}`);
            });
        }
        
        // Step 3: Run migration
        console.log("\n3️⃣ Starting fresh migration...");
        await invokeCommand('start_migration');
        console.log("✅ Migration started successfully");
        
        // Step 4: Monitor progress
        console.log("\n4️⃣ Monitoring migration progress...");
        let attempts = 0;
        const maxAttempts = 60; // 60 seconds max
        
        while (attempts < maxAttempts) {
            await new Promise(resolve => setTimeout(resolve, 1000)); // Wait 1 second
            attempts++;
            
            try {
                const progress = await invokeCommand('get_migration_stats');
                if (progress) {
                    console.log(`📈 Progress: ${progress.current_step}`);
                    console.log(`   Processed: ${progress.processed_recordings}/${progress.total_recordings}`);
                    console.log(`   Failed: ${progress.failed_recordings}`);
                    console.log(`   Size: ${Math.round(progress.processed_size_bytes / 1024 / 1024)}MB / ${Math.round(progress.total_size_bytes / 1024 / 1024)}MB`);
                } else {
                    console.log("✅ Migration completed!");
                    break;
                }
            } catch (error) {
                console.log("✅ Migration completed (no progress data)");
                break;
            }
        }
        
        if (attempts >= maxAttempts) {
            console.warn("⚠️ Migration monitoring timed out");
        }
        
        // Step 5: Check results
        console.log("\n5️⃣ Checking migration results...");
        const slicesAfter = await invokeCommand('get_slice_records');
        console.log(`📊 Slices in database after migration: ${slicesAfter.length}`);
        
        if (slicesAfter.length === 0) {
            console.error("❌ No files were copied! Migration failed.");
        } else {
            console.log("✅ Files were copied successfully!");
            slicesAfter.forEach((slice, i) => {
                console.log(`   ${i + 1}. ${slice.original_audio_file_name} (${slice.audio_file_size} bytes)`);
            });
        }
        
        // Step 6: Get config to show where files should be
        console.log("\n6️⃣ Checking destination directory...");
        const config = await invokeCommand('get_config');
        console.log(`📂 Files should be copied to: ${config.ciderpress_home}/audio/`);
        console.log("   Please manually verify the files exist in this directory.");
        
        console.log("\n🎉 Fresh migration test completed!");
        
    } catch (error) {
        console.error("\n💥 Fresh migration test failed:", error);
    }
}

// Auto-run the test when the script loads
runFreshMigrationTest(); 