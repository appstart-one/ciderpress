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

// Transcription Job Status Enum
export type TranscriptionJobStatus = 
  | 'queued'
  | 'processing' 
  | 'completed'
  | 'failed'
  | 'cancelled';

// Core TranscriptionJob interface
export interface TranscriptionJob {
  id: string;
  recordingId: number;
  filename: string;
  startedAt: Date | null;
  completedAt: Date | null;
  estimatedDurationSeconds: number;
  processedSeconds: number;
  status: TranscriptionJobStatus;
  progress: number; // 0-100 percentage
  errorMessage?: string;
  modelName: string;
}

// Job creation parameters
export interface CreateTranscriptionJobParams {
  recordingId: number;
  filename: string;
  estimatedDurationSeconds: number;
  modelName: string;
}

// Job update parameters
export interface UpdateTranscriptionJobParams {
  id: string;
  status?: TranscriptionJobStatus;
  processedSeconds?: number;
  progress?: number;
  errorMessage?: string;
  completedAt?: Date;
}

// Job queue statistics
export interface TranscriptionQueueStats {
  totalJobs: number;
  queuedJobs: number;
  processingJobs: number;
  completedJobs: number;
  failedJobs: number;
}

// Event types for job updates
export type TranscriptionJobEvent = 
  | { type: 'JOB_CREATED'; job: TranscriptionJob }
  | { type: 'JOB_UPDATED'; job: TranscriptionJob }
  | { type: 'JOB_DELETED'; jobId: string }
  | { type: 'JOB_PROGRESS'; jobId: string; progress: number; processedSeconds: number }
  | { type: 'JOB_STATUS_CHANGED'; jobId: string; status: TranscriptionJobStatus }; 