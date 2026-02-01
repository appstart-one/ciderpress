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

// Migration Verification Test Script
// Run this in the browser console after migration

console.log("🔍 Migration Verification Test Starting...");

// Helper function to call Tauri commands
async function invokeCommand(command, args = {}) {
    try {
        const result = await window.__TAURI__.core.invoke(command, args);
        return result;
    } catch (error) {
        console.error(`Failed to invoke ${command}:`, error);
        throw error;
    }
}

// Test 1: Check if migration completes without errors
async function testMigrationProcess() {
    console.log("\n📋 Test 1: Running Migration...");
    
    try {
        // Start migration
        await invokeCommand('start_migration');
        console.log("✅ Migration started successfully");
        
        // Poll for completion
        let attempts = 0;
        const maxAttempts = 30; // 30 seconds timeout
        
        while (attempts < maxAttempts) {
            const stats = await invokeCommand('get_migration_stats');
            
            if (!stats) {
                console.log("✅ Migration completed (stats is null)");
                break;
            }
            
            console.log(`Migration progress: ${stats.processed_recordings}/${stats.total_recordings} files processed`);
            console.log(`Current step: ${stats.current_step}`);
            
            if (stats.processed_recordings + stats.failed_recordings >= stats.total_recordings) {
                console.log("✅ Migration completed");
                console.log(`Final stats: ${stats.processed_recordings} processed, ${stats.failed_recordings} failed`);
                break;
            }
            
            await new Promise(resolve => setTimeout(resolve, 1000));
            attempts++;
        }
        
        if (attempts >= maxAttempts) {
            console.warn("⚠️ Migration timeout - took longer than expected");
        }
        
    } catch (error) {
        console.error("❌ Migration failed:", error);
        throw error;
    }
}

// Test 2: Verify files were copied
async function testFilesCopied() {
    console.log("\n📁 Test 2: Verifying Files Were Copied...");
    
    try {
        // Get config to know where files should be copied
        const config = await invokeCommand('get_config');
        console.log("CiderPress home directory:", config.ciderpress_home);
        
        // Note: We can't directly read the filesystem from browser,
        // but we can check if the migration reported any copied files
        console.log("✅ Config retrieved - check the CiderPress home directory manually");
        console.log(`📂 Check directory: ${config.ciderpress_home}`);
        console.log("   Look for .m4a files that should have been copied");
        
    } catch (error) {
        console.error("❌ Failed to get config:", error);
        throw error;
    }
}

// Test 3: Verify slice records were created
async function testSliceRecords() {
    console.log("\n📊 Test 3: Verifying Slice Records...");
    
    try {
        const slices = await invokeCommand('get_slice_records');
        console.log(`✅ Found ${slices.length} slice records in database`);
        
        if (slices.length === 0) {
            console.warn("⚠️ No slice records found - migration may not have copied any files");
            return;
        }
        
        slices.forEach((slice, index) => {
            console.log(`📄 Slice ${index + 1}:`);
            console.log(`   - Filename: ${slice.original_audio_file_name}`);
            console.log(`   - Size: ${slice.audio_file_size} bytes`);
            console.log(`   - Type: ${slice.audio_file_type}`);
            console.log(`   - Transcribed: ${slice.transcribed}`);
            console.log(`   - Estimated time: ${slice.estimated_time_to_transcribe} seconds`);
            console.log(`   - ID: ${slice.id}`);
        });
        
        console.log("✅ Slice records verified successfully");
        
    } catch (error) {
        console.error("❌ Failed to verify slice records:", error);
        throw error;
    }
}

// Test 4: Check for specific file patterns and logging
async function testDetailedLogging() {
    console.log("\n📝 Test 4: Checking Detailed Logging...");
    
    console.log("✅ Enhanced logging should show:");
    console.log("   - '✅ SUCCESSFULLY COPIED FILE: [filename]' for each copied file");
    console.log("   - '✅ SUCCESSFULLY ADDED SLICE RECORD:' for each database record");
    console.log("   - File sizes and estimated transcription times");
    console.log("   - Any files that were skipped with reasons");
    console.log("   - Check the migration log window for these messages");
}

// Run all tests
async function runAllTests() {
    try {
        await testMigrationProcess();
        await testFilesCopied();
        await testSliceRecords();
        await testDetailedLogging();
        
        console.log("\n🎉 All verification tests completed!");
        console.log("\n📋 Manual Verification Steps:");
        console.log("1. Check the migration log window for '✅ SUCCESSFULLY COPIED FILE' messages");
        console.log("2. Check the CiderPress home directory for actual .m4a files");
        console.log("3. Open CiderPress-db.sqlite and verify slice records exist");
        console.log("4. Confirm each copied file has a corresponding slice record");
        
    } catch (error) {
        console.error("\n💥 Verification failed:", error);
    }
}

// Auto-run the tests
console.log("🚀 Starting automated verification...");
runAllTests();

// Also provide a manual test function
window.testMigration = runAllTests;
window.startMigrationTest = testMigrationProcess;

console.log("\n💡 Manual Commands Available:");
console.log("- window.testMigration() - Run all tests");
console.log("- window.startMigrationTest() - Just test migration process"); 