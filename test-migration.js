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

// Test script for migration commands
// Copy and paste this into the browser console to test the migration functionality

console.log("Testing migration commands...");

// Test 1: Check if we can get initial migration stats
window.__TAURI__.core.invoke('get_migration_stats')
  .then(stats => {
    console.log("Initial migration stats:", stats);
  })
  .catch(error => {
    console.error("Error getting initial stats:", error);
  });

// Test 2: Start migration
console.log("Starting migration...");
window.__TAURI__.core.invoke('start_migration')
  .then(() => {
    console.log("Migration started successfully");
    
    // Test 3: Poll for progress
    let pollCount = 0;
    const pollInterval = setInterval(() => {
      pollCount++;
      console.log(`Polling attempt ${pollCount}`);
      
      window.__TAURI__.core.invoke('get_migration_stats')
        .then(stats => {
          console.log("Migration progress:", stats);
          
          if (!stats || pollCount > 30) { // Stop after 30 polls or when complete
            clearInterval(pollInterval);
            console.log("Migration completed or timed out");
          }
        })
        .catch(error => {
          console.error("Error polling stats:", error);
          clearInterval(pollInterval);
        });
    }, 1000);
  })
  .catch(error => {
    console.error("Error starting migration:", error);
  }); 