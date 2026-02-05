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

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { notifications } from '@mantine/notifications';
import {
  Container,
  Title,
  Paper,
  Button,
  Text,
  Stack,
  TextInput,
  Group,
  Table,
  Badge,
  ActionIcon,
  Pagination,
  Select,
  Checkbox,
  Modal,
  Progress,
  RingProgress,
  Card,
  ThemeIcon,
  Transition,
  Box,
  Popover
} from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { IconSearch, IconFileText, IconTrash, IconEdit, IconX, IconCheck, IconChevronUp, IconChevronDown, IconPlayerPlay, IconPencil, IconBug, IconWaveSquare, IconClock, IconCircleCheck, IconAlertCircle, IconDownload, IconColumns, IconNotebook, IconUpload, IconBulb } from '@tabler/icons-react';
import { QuillEditor } from '../components/QuillEditor';
import { AudioPlayer } from '../components/AudioPlayer';
import { DraggableCard } from '../components/DraggableCard';

interface Slice {
  id: number;
  original_audio_file_name: string;
  title: string | null;
  transcribed: boolean;
  audio_file_size: number;
  audio_file_type: string;
  estimated_time_to_transcribe: number;
  audio_time_length_seconds: number | null;
  transcription: string | null;
  transcription_time_taken: number | null;
  transcription_word_count: number | null;
  transcription_model: string | null;
  recording_date: number | null; // Unix timestamp of original recording
}

interface TranscriptionProgress {
  total_slices: number;
  completed_slices: number;
  failed_slices: number;
  current_slice_id: number | null;
  current_slice_name: string | null;
  current_step: string;
  estimated_total_seconds: number;
  elapsed_seconds: number;
  is_active: boolean;
  // Per-slice progress tracking
  current_slice_elapsed_seconds: number;
  current_slice_estimated_seconds: number;
  current_slice_file_size: number;
  bytes_per_second_rate: number;
}

type SortField = keyof Slice;
type SortDirection = 'asc' | 'desc';

export default function Slices() {
  const [slices, setSlices] = useState<Slice[]>([]);
  const [filteredSlices, setFilteredSlices] = useState<Slice[]>([]);
  const [loading, setLoading] = useState(true);
  // Restore view state from localStorage to survive route changes
  const [searchTerm, setSearchTerm] = useState(() => {
    try { return localStorage.getItem('slices_search') || ''; } catch { return ''; }
  });
  const [selectedSlices, setSelectedSlices] = useState<number[]>(() => {
    try {
      const saved = localStorage.getItem('slices_selected');
      return saved ? JSON.parse(saved) : [];
    } catch { return []; }
  });
  const [transcribingSlices, setTranscribingSlices] = useState<number[]>([]);
  const [updatingNames, setUpdatingNames] = useState<number[]>([]);
  const [currentPage, setCurrentPage] = useState(() => {
    try { return Number(localStorage.getItem('slices_page')) || 1; } catch { return 1; }
  });
  const [pageSize, setPageSize] = useState(() => {
    try { return Number(localStorage.getItem('slices_pageSize')) || 25; } catch { return 25; }
  });
  const [sortField, setSortField] = useState<SortField>(() => {
    try { return (localStorage.getItem('slices_sortField') as SortField) || 'id'; } catch { return 'id'; }
  });
  const [sortDirection, setSortDirection] = useState<SortDirection>(() => {
    try { return (localStorage.getItem('slices_sortDir') as SortDirection) || 'asc'; } catch { return 'asc'; }
  });
  const [editingSlice, setEditingSlice] = useState<Slice | null>(null);
  const [editingTitleId, setEditingTitleId] = useState<number | null>(null);
  const [editingTitleValue, setEditingTitleValue] = useState<string>('');
  const [opened, { open, close }] = useDisclosure(false);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [currentModel, setCurrentModel] = useState<string>('');
  const [audioPlayerOpened, { open: openAudioPlayer, close: closeAudioPlayer }] = useDisclosure(false);
  const [currentAudioSrc, setCurrentAudioSrc] = useState<string>('');
  const [currentAudioSlice, setCurrentAudioSlice] = useState<Slice | null>(null);
  const [debugOpened, { open: openDebug, close: closeDebug }] = useDisclosure(false);
  const [debugSlice, setDebugSlice] = useState<Slice | null>(null);
  const [transcriptionProgress, setTranscriptionProgress] = useState<TranscriptionProgress | null>(null);

  // NLM (NotebookLM) upload tracking
  // Stored as { [sliceId]: { audio: boolean, text: boolean } }
  const [nlmUploads, setNlmUploads] = useState<Record<number, { audio: boolean; text: boolean }>>(() => {
    try {
      const saved = localStorage.getItem('nlm_uploads');
      return saved ? JSON.parse(saved) : {};
    } catch { return {}; }
  });
  const [nlmUploading, setNlmUploading] = useState(false);

  // Column visibility state - define which columns are visible by default
  const [visibleColumns, setVisibleColumns] = useState<Record<string, boolean>>({
    title: true,
    audio_file_size: true,
    audio_file_type: true,
    audio_time_length_seconds: true,
    recording_date: true,
    transcribed: true,
    nlm_status: true,
  });

  // Column configuration with display names (ordered, can be rearranged)
  const [columnOrder, setColumnOrder] = useState<{ key: string; label: string }[]>([
    { key: 'title', label: 'Title' },
    { key: 'audio_file_size', label: 'Size' },
    { key: 'audio_file_type', label: 'Type' },
    { key: 'audio_time_length_seconds', label: 'Audio Length' },
    { key: 'recording_date', label: 'Date' },
    { key: 'transcribed', label: 'Status' },
    { key: 'nlm_status', label: 'NLM' },
  ]);

  // Drag-and-drop state for column reordering
  const [draggedColumn, setDraggedColumn] = useState<string | null>(null);

  const toggleColumnVisibility = (columnKey: string) => {
    setVisibleColumns((prev) => ({
      ...prev,
      [columnKey]: !prev[columnKey],
    }));
  };

  // Column drag-and-drop handlers
  const handleDragStart = (e: React.DragEvent, columnKey: string) => {
    setDraggedColumn(columnKey);
    e.dataTransfer.effectAllowed = 'move';
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  };

  const handleDrop = (e: React.DragEvent, targetColumnKey: string) => {
    e.preventDefault();
    if (!draggedColumn || draggedColumn === targetColumnKey) {
      setDraggedColumn(null);
      return;
    }

    setColumnOrder((prevOrder) => {
      const newOrder = [...prevOrder];
      const draggedIndex = newOrder.findIndex((col) => col.key === draggedColumn);
      const targetIndex = newOrder.findIndex((col) => col.key === targetColumnKey);

      if (draggedIndex === -1 || targetIndex === -1) return prevOrder;

      // Remove dragged item and insert at new position
      const [removed] = newOrder.splice(draggedIndex, 1);
      newOrder.splice(targetIndex, 0, removed);
      return newOrder;
    });

    setDraggedColumn(null);
  };

  const handleDragEnd = () => {
    setDraggedColumn(null);
  };

  // Persist view state to localStorage so it survives route changes
  useEffect(() => { try { localStorage.setItem('slices_search', searchTerm); } catch {} }, [searchTerm]);
  useEffect(() => { try { localStorage.setItem('slices_selected', JSON.stringify(selectedSlices)); } catch {} }, [selectedSlices]);
  useEffect(() => { try { localStorage.setItem('slices_page', String(currentPage)); } catch {} }, [currentPage]);
  useEffect(() => { try { localStorage.setItem('slices_pageSize', String(pageSize)); } catch {} }, [pageSize]);
  useEffect(() => { try { localStorage.setItem('slices_sortField', sortField); } catch {} }, [sortField]);
  useEffect(() => { try { localStorage.setItem('slices_sortDir', sortDirection); } catch {} }, [sortDirection]);

  useEffect(() => {
    loadSlices();
    loadModels();
    loadCurrentConfig();
    autoPopulateTitles();
    populateAudioDurations();
    backfillRecordingDates();
  }, []);

  const populateAudioDurations = async () => {
    try {
      const count = await invoke<number>('populate_audio_durations');
      if (count > 0) {
        console.log(`Populated ${count} audio durations`);
        // Reload slices to show updated durations
        loadSlices();
      }
    } catch (error) {
      console.error('Failed to populate audio durations:', error);
      // Don't show notification - this is a background operation
    }
  };

  const backfillRecordingDates = async () => {
    try {
      const count = await invoke<number>('backfill_recording_dates');
      if (count > 0) {
        console.log(`Backfilled ${count} recording dates`);
        // Reload slices to show updated dates
        loadSlices();
      }
    } catch (error) {
      console.error('Failed to backfill recording dates:', error);
      // Don't show notification - this is a background operation
    }
  };

  const autoPopulateTitles = async () => {
    try {
      const count = await invoke<number>('auto_populate_titles');
      if (count > 0) {
        console.log(`Auto-populated ${count} titles`);
        // Reload slices to show updated titles
        loadSlices();
      }
    } catch (error) {
      console.error('Failed to auto-populate titles:', error);
      // Don't show notification - this is a background operation
    }
  };

  const loadModels = async () => {
    try {
      const models = await invoke<string[]>('get_available_models');
      setAvailableModels(models);
    } catch (error) {
      console.error('Failed to load available models:', error);
    }
  };

  const getModelDisplayName = (modelName: string) => {
    const modelDescriptions: Record<string, string> = {
      'tiny': 'Tiny (~1GB, fastest)',
      'tiny.en': 'Tiny English (~1GB, fastest)',
      'base': 'Base (~1.5GB, balanced)',
      'base.en': 'Base English (~1.5GB, balanced)',
      'small': 'Small (~2.5GB, good quality)',
      'small.en': 'Small English (~2.5GB, good quality)',
      'medium': 'Medium (~5GB, high quality)',
      'medium.en': 'Medium English (~5GB, high quality)',
      'large': 'Large (~10GB, best quality)',
      'large-v1': 'Large v1 (~10GB, best quality)',
      'large-v2': 'Large v2 (~10GB, best quality)',
      'large-v3': 'Large v3 (~10GB, best quality)',
    };
    return modelDescriptions[modelName] || modelName;
  };

  const loadCurrentConfig = async () => {
    try {
      const config = await invoke<any>('get_config');
      setCurrentModel(config.model_name);
    } catch (error) {
      console.error('Failed to load current config:', error);
    }
  };

  const handleModelChange = async (modelName: string) => {
    try {
      await invoke('update_transcription_model', { modelName: modelName });
      setCurrentModel(modelName);
      notifications.show({
        title: 'Success',
        message: `Transcription model updated to ${modelName}`,
        color: 'green',
        icon: <IconCheck size={16} />,
      });
    } catch (error) {
      console.error('Failed to update model:', error);
      let errorMessage = 'Failed to update transcription model';
      
      if (error && typeof error === 'object' && 'message' in error) {
        errorMessage = error.message as string;
      } else if (typeof error === 'string') {
        errorMessage = error;
      }
      
      notifications.show({
        title: 'Error',
        message: errorMessage,
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  // Poll for updates when there are transcribing slices or name updates
  useEffect(() => {
    if (transcribingSlices.length === 0 && updatingNames.length === 0) return;

    const interval = setInterval(async () => {
      try {
        // Get fresh data without triggering loading state
        const freshSlices = await invoke<Slice[]>('get_slice_records');

        // Check if any transcribing slices are now completed
        const completedSlices = transcribingSlices.filter(id => {
          const slice = freshSlices.find(s => s.id === id);
          return slice && slice.transcribed;
        });

        // Check if any name updates are completed by comparing filenames
        const oldSlices = slices;
        const updatedNameSlices = updatingNames.filter(id => {
          const oldSlice = oldSlices.find(s => s.id === id);
          const newSlice = freshSlices.find(s => s.id === id);
          // If the filename has changed, consider it updated
          return oldSlice && newSlice && oldSlice.original_audio_file_name !== newSlice.original_audio_file_name;
        });

        // Only update slices state if there are actual changes
        if (completedSlices.length > 0 || updatedNameSlices.length > 0) {
          setSlices(freshSlices);
          if (completedSlices.length > 0) {
            setTranscribingSlices(prev => prev.filter(id => !completedSlices.includes(id)));
          }
          if (updatedNameSlices.length > 0) {
            setUpdatingNames(prev => prev.filter(id => !updatedNameSlices.includes(id)));
          }
        }
      } catch (error) {
        console.error('Error polling for updates:', error);
      }
    }, 2000); // Poll every 2 seconds

    return () => clearInterval(interval);
  }, [transcribingSlices, updatingNames, slices]); // Added updatingNames and slices

  // Poll for transcription progress when there are transcribing slices
  useEffect(() => {
    if (transcribingSlices.length === 0) {
      // Clear progress when no slices are transcribing
      setTranscriptionProgress(null);
      return;
    }

    const pollProgress = async () => {
      try {
        const progress = await invoke<TranscriptionProgress | null>('get_transcription_progress');
        setTranscriptionProgress(progress);

        // If transcription is complete, clear the progress after a short delay
        if (progress && !progress.is_active) {
          setTimeout(() => {
            setTranscriptionProgress(null);
          }, 3000);
        }
      } catch (error) {
        console.error('Error polling transcription progress:', error);
      }
    };

    // Poll immediately
    pollProgress();

    // Then poll every 500ms for smooth progress updates
    const interval = setInterval(pollProgress, 500);

    return () => clearInterval(interval);
  }, [transcribingSlices]);

  useEffect(() => {
    // Apply filtering, sorting and pagination when data or search term changes
    let filtered = slices;
    
    if (searchTerm) {
      filtered = slices.filter(slice =>
        slice.original_audio_file_name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (slice.title && slice.title.toLowerCase().includes(searchTerm.toLowerCase())) ||
        (slice.transcription && slice.transcription.toLowerCase().includes(searchTerm.toLowerCase()))
      );
    }
    
    // Apply sorting
    filtered.sort((a, b) => {
      const aVal = a[sortField];
      const bVal = b[sortField];
      
      // Handle null values
      if (aVal === null && bVal === null) return 0;
      if (aVal === null) return sortDirection === 'asc' ? 1 : -1;
      if (bVal === null) return sortDirection === 'asc' ? -1 : 1;
      
      // Handle different data types
      let comparison = 0;
      if (typeof aVal === 'string' && typeof bVal === 'string') {
        comparison = aVal.toLowerCase().localeCompare(bVal.toLowerCase());
      } else if (typeof aVal === 'number' && typeof bVal === 'number') {
        comparison = aVal - bVal;
      } else if (typeof aVal === 'boolean' && typeof bVal === 'boolean') {
        comparison = aVal === bVal ? 0 : aVal ? 1 : -1;
      } else {
        comparison = String(aVal).localeCompare(String(bVal));
      }
      
      return sortDirection === 'asc' ? comparison : -comparison;
    });
    
    setFilteredSlices(filtered);
    setCurrentPage(1); // Reset to first page when filtering or sorting
  }, [slices, searchTerm, sortField, sortDirection]);

  const loadSlices = async () => {
    setLoading(true);
    try {
      const data = await invoke<Slice[]>('get_slice_records');
      setSlices(data);
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to load slices',
        color: 'red',
        icon: <IconX size={16} />,
      });
    } finally {
      setLoading(false);
    }
  };

  const transcribeSelected = async () => {
    if (selectedSlices.length === 0) return;

    // Set transcribing state for selected slices
    setTranscribingSlices(prev => [...prev, ...selectedSlices]);

    try {
      await invoke('transcribe_slices', { sliceIds: selectedSlices });
      notifications.show({
        title: 'Success',
        message: `Started transcription for ${selectedSlices.length} slices`,
        color: 'green',
        icon: <IconCheck size={16} />,
      });
      setSelectedSlices([]);

      // Don't clear transcribing state here - let the polling handle it
    } catch (error) {
      console.error('Transcription error:', error);
      // Clear transcribing state on error
      setTranscribingSlices(prev => prev.filter(id => !selectedSlices.includes(id)));
      notifications.show({
        title: 'Error',
        message: `Failed to start transcription: ${error}`,
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const updateSliceNames = async () => {
    if (selectedSlices.length === 0) return;

    // Set updating state for selected slices
    setUpdatingNames(prev => [...prev, ...selectedSlices]);

    try {
      await invoke('update_slice_names_from_audio', { sliceIds: selectedSlices });
      notifications.show({
        title: 'Success',
        message: `Started updating names for ${selectedSlices.length} slices`,
        color: 'green',
        icon: <IconCheck size={16} />,
      });
      setSelectedSlices([]);

      // Don't clear updating state here - let the polling handle it
    } catch (error) {
      console.error('Name update error:', error);
      // Clear updating state on error
      setUpdatingNames(prev => prev.filter(id => !selectedSlices.includes(id)));
      notifications.show({
        title: 'Error',
        message: `Failed to start name update: ${error}`,
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const exportTranscribedText = async () => {
    if (selectedSlices.length === 0) return;

    try {
      const exportPath = await invoke<string>('export_transcribed_text', { sliceIds: selectedSlices });
      notifications.show({
        title: 'Export Successful',
        message: `Transcriptions exported to: ${exportPath}`,
        color: 'green',
        icon: <IconCheck size={16} />,
        autoClose: 8000,
      });
      setSelectedSlices([]);
    } catch (error) {
      console.error('Export error:', error);
      let errorMessage = 'Failed to export transcriptions';

      if (error && typeof error === 'object' && 'message' in error) {
        errorMessage = error.message as string;
      } else if (typeof error === 'string') {
        errorMessage = error;
      }

      notifications.show({
        title: 'Export Error',
        message: errorMessage,
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const openSuggestFeature = async () => {
    try {
      const systemInfo = await invoke<{ app_version: string; macos_version: string }>('get_system_info');
      const body = `## Feature Request\n\n**Describe the feature you'd like:**\n\n\n**Why would this be useful?**\n\n\n---\n_App Version: ${systemInfo.app_version}_\n_macOS Version: ${systemInfo.macos_version}_`;
      const params = new URLSearchParams({
        title: '',
        body: body,
        labels: 'enhancement',
      });
      await invoke('open_url', { url: `https://github.com/appstart-one/ciderpress/issues/new?${params.toString()}` });
    } catch (error) {
      console.error('Failed to open feature request:', error);
      notifications.show({
        title: 'Error',
        message: 'Failed to open feature request page',
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  // NLM upload helpers
  const getSelectedNotebook = (): string | null => {
    return localStorage.getItem('nlm_selected_notebook');
  };

  const persistNlmUploads = (uploads: Record<number, { audio: boolean; text: boolean }>) => {
    setNlmUploads(uploads);
    localStorage.setItem('nlm_uploads', JSON.stringify(uploads));
  };

  const uploadAudioToNlm = async () => {
    const notebookId = getSelectedNotebook();
    if (!notebookId) {
      notifications.show({
        title: 'No Notebook Selected',
        message: 'Go to the NotebookLM page and select a notebook first.',
        color: 'yellow',
        icon: <IconNotebook size={16} />,
      });
      return;
    }
    if (selectedSlices.length === 0) return;

    setNlmUploading(true);
    let successCount = 0;
    const updatedUploads = { ...nlmUploads };

    for (const sliceId of selectedSlices) {
      try {
        await invoke<string>('nlm_add_audio', { notebookId, sliceId });
        updatedUploads[sliceId] = {
          audio: true,
          text: updatedUploads[sliceId]?.text || false,
        };
        successCount++;
      } catch (error: unknown) {
        const msg = typeof error === 'string' ? error : (error as { message?: string })?.message || JSON.stringify(error);
        console.error(`Failed to upload audio for slice ${sliceId}:`, error);
        notifications.show({
          title: 'Upload Error',
          message: `Failed to upload audio for slice ${sliceId}: ${msg}`,
          color: 'red',
          icon: <IconX size={16} />,
        });
      }
    }

    persistNlmUploads(updatedUploads);
    setNlmUploading(false);

    if (successCount > 0) {
      notifications.show({
        title: 'Upload Complete',
        message: `Uploaded ${successCount} audio file(s) to NotebookLM.`,
        color: 'green',
        icon: <IconCheck size={16} />,
      });
    }
  };

  const uploadTextToNlm = async () => {
    const notebookId = getSelectedNotebook();
    if (!notebookId) {
      notifications.show({
        title: 'No Notebook Selected',
        message: 'Go to the NotebookLM page and select a notebook first.',
        color: 'yellow',
        icon: <IconNotebook size={16} />,
      });
      return;
    }
    if (selectedSlices.length === 0) return;

    // Filter to only slices with transcriptions
    const slicesWithText = slices.filter(
      s => selectedSlices.includes(s.id) && s.transcription
    );

    if (slicesWithText.length === 0) {
      notifications.show({
        title: 'No Transcriptions',
        message: 'None of the selected slices have transcriptions to upload.',
        color: 'yellow',
        icon: <IconFileText size={16} />,
      });
      return;
    }

    setNlmUploading(true);
    let successCount = 0;
    const updatedUploads = { ...nlmUploads };

    for (const slice of slicesWithText) {
      try {
        // Strip HTML tags from transcription for plain text upload
        const plainText = (slice.transcription || '').replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
        const title = slice.title || slice.original_audio_file_name;
        await invoke<string>('nlm_add_text', {
          notebookId,
          text: plainText,
          title: `${title}.txt`,
        });
        updatedUploads[slice.id] = {
          audio: updatedUploads[slice.id]?.audio || false,
          text: true,
        };
        successCount++;
      } catch (error: unknown) {
        const msg = typeof error === 'string' ? error : (error as { message?: string })?.message || JSON.stringify(error);
        console.error(`Failed to upload text for slice ${slice.id}:`, error);
        notifications.show({
          title: 'Upload Error',
          message: `Failed to upload text for "${slice.title || slice.original_audio_file_name}": ${msg}`,
          color: 'red',
          icon: <IconX size={16} />,
        });
      }
    }

    persistNlmUploads(updatedUploads);
    setNlmUploading(false);

    if (successCount > 0) {
      notifications.show({
        title: 'Upload Complete',
        message: `Uploaded ${successCount} transcription(s) to NotebookLM.`,
        color: 'green',
        icon: <IconCheck size={16} />,
      });
    }
  };

  const saveSlice = async () => {
    if (!editingSlice) return;

    try {
      // Extract plain text from HTML for word count calculation
      const getPlainTextWordCount = (html: string): number => {
        if (!html || html.trim() === '' || html === '<p><br></p>') return 0;
        
        // Create a temporary div to parse HTML and extract text content
        const tempDiv = document.createElement('div');
        tempDiv.innerHTML = html;
        const plainText = tempDiv.textContent || tempDiv.innerText || '';
        
        // Count words in plain text, filtering out empty strings
        const words = plainText.split(/\s+/).filter(word => word.trim().length > 0);
        return words.length;
      };

      // Update word count if transcription was manually edited
      const wordCount = editingSlice.transcription ? getPlainTextWordCount(editingSlice.transcription) : 0;
      const updatedSlice = {
        ...editingSlice,
        transcription_word_count: wordCount > 0 ? wordCount : null,
        transcribed: editingSlice.transcription && wordCount > 0 ? true : editingSlice.transcribed
      };

      await invoke('update_slice', { slice: updatedSlice });
      
      notifications.show({
        title: 'Success',
        message: 'Slice updated successfully',
        color: 'green',
        icon: <IconCheck size={16} />,
      });
      
      close();
      setEditingSlice(null);
      loadSlices();
    } catch (error) {
      console.error('Error updating slice:', error);
      let errorMessage = 'Failed to update slice';
      
      // Extract error message from Tauri error
      if (error && typeof error === 'object' && 'message' in error) {
        errorMessage = error.message as string;
      } else if (typeof error === 'string') {
        errorMessage = error;
      }
      
      notifications.show({
        title: 'Error',
        message: errorMessage,
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const startEditingTitle = (slice: Slice) => {
    setEditingTitleId(slice.id);
    // If title is null, suggest a value from filename, but user can change it
    const suggestedTitle = slice.title || slice.original_audio_file_name.replace(/\.(m4a|wav|mp3)$/i, '');
    setEditingTitleValue(suggestedTitle);
  };

  const cancelEditingTitle = () => {
    setEditingTitleId(null);
    setEditingTitleValue('');
  };

  const saveSliceTitle = async (sliceId: number) => {
    if (!editingTitleValue.trim()) {
      notifications.show({
        title: 'Error',
        message: 'Title cannot be empty',
        color: 'red',
        icon: <IconX size={16} />,
      });
      return;
    }

    try {
      // Update the recording title
      await invoke('update_recording_title', {
        sliceId: sliceId,
        newTitle: editingTitleValue.trim()
      });

      // Update local state
      setSlices(prev => prev.map(slice =>
        slice.id === sliceId
          ? { ...slice, title: editingTitleValue.trim() }
          : slice
      ));

      notifications.show({
        title: 'Success',
        message: 'Title updated successfully',
        color: 'green',
        icon: <IconCheck size={16} />,
      });

      cancelEditingTitle();
    } catch (error) {
      console.error('Error updating title:', error);
      let errorMessage = 'Failed to update title';

      // Extract error message from Tauri error
      if (error && typeof error === 'object' && 'message' in error) {
        errorMessage = error.message as string;
      } else if (typeof error === 'string') {
        errorMessage = error;
      }

      notifications.show({
        title: 'Error',
        message: errorMessage,
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const handlePlayAudio = async (slice: Slice) => {
    try {
      // Get the audio file bytes from the backend
      const audioBytes = await invoke<number[]>('get_slice_audio_bytes', { sliceId: slice.id });

      // Convert the bytes array to a Uint8Array
      const uint8Array = new Uint8Array(audioBytes);

      // Create a blob from the bytes
      const blob = new Blob([uint8Array], { type: 'audio/m4a' });

      // Create a blob URL
      const audioUrl = URL.createObjectURL(blob);

      // Clean up previous blob URL if exists
      if (currentAudioSrc && currentAudioSrc.startsWith('blob:')) {
        URL.revokeObjectURL(currentAudioSrc);
      }

      setCurrentAudioSrc(audioUrl);
      setCurrentAudioSlice(slice);
      openAudioPlayer();
    } catch (error) {
      console.error('Failed to load audio:', error);
      let errorMessage = 'Failed to load audio file';

      if (error && typeof error === 'object' && 'message' in error) {
        errorMessage = error.message as string;
      } else if (typeof error === 'string') {
        errorMessage = error;
      }

      notifications.show({
        title: 'Error',
        message: errorMessage,
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const formatFileSize = (bytes: number) => {
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    if (bytes === 0) return '0 Bytes';
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return Math.round(bytes / Math.pow(1024, i) * 100) / 100 + ' ' + sizes[i];
  };

  const formatDuration = (seconds: number) => {
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
  };

  const formatDate = (timestamp: number | null) => {
    if (!timestamp) return '-';
    const date = new Date(timestamp * 1000); // Convert Unix timestamp to milliseconds
    return date.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  };

  // Format audio length as human-readable (1h 30m 5s) with zero components omitted
  const formatAudioLength = (seconds: number | null): string => {
    if (seconds === null || seconds === undefined) {
      return '-';
    }

    const totalSeconds = Math.round(seconds);
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const secs = totalSeconds % 60;

    const parts: string[] = [];

    if (hours > 0) {
      parts.push(`${hours}h`);
    }
    if (minutes > 0) {
      parts.push(`${minutes}m`);
    }
    if (secs > 0 || parts.length === 0) {
      // Always show seconds if it's the only non-zero component or if all are zero
      parts.push(`${secs}s`);
    }

    return parts.join(' ');
  };

  // Format elapsed time for progress display
  const formatElapsedTime = (seconds: number): string => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;

    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  };

  // Calculate progress percentage including partial progress on current slice
  const getProgressPercentage = (): number => {
    if (!transcriptionProgress || transcriptionProgress.total_slices === 0) return 0;

    // Calculate progress within current slice (0 to 1)
    let currentSliceProgress = 0;
    if (transcriptionProgress.current_slice_estimated_seconds > 0) {
      currentSliceProgress = Math.min(
        1,
        transcriptionProgress.current_slice_elapsed_seconds / transcriptionProgress.current_slice_estimated_seconds
      );
    }

    // Total progress = completed slices + partial current slice progress
    const totalProgress = (transcriptionProgress.completed_slices + currentSliceProgress) / transcriptionProgress.total_slices;

    return Math.min(100, Math.round(totalProgress * 100));
  };

  const getStatusColor = (slice: Slice) => {
    if (transcribingSlices.includes(slice.id)) return 'orange';
    return slice.transcribed ? 'green' : 'gray';
  };

  const getStatusText = (slice: Slice) => {
    if (transcribingSlices.includes(slice.id)) return 'Transcribing Now';
    return slice.transcribed ? 'Transcribed' : 'Audio';
  };

  const toggleSelectAll = () => {
    const currentPageSlices = getCurrentPageSlices();
    const currentPageIds = currentPageSlices.map(s => s.id);
    
    if (selectedSlices.length === currentPageIds.length && 
        currentPageIds.every(id => selectedSlices.includes(id))) {
      setSelectedSlices([]);
    } else {
      setSelectedSlices(currentPageIds);
    }
  };

  const toggleSelectSlice = (id: number) => {
    setSelectedSlices(prev => 
      prev.includes(id) 
        ? prev.filter(sliceId => sliceId !== id)
        : [...prev, id]
    );
  };

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      // Toggle direction if same field
      setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc');
    } else {
      // Set new field with ascending direction
      setSortField(field);
      setSortDirection('asc');
    }
  };

  const getSortIcon = (field: SortField) => {
    if (sortField !== field) return null;
    return sortDirection === 'asc' ? <IconChevronUp size={14} /> : <IconChevronDown size={14} />;
  };

  // Helper function to render a column header with drag support
  const renderColumnHeader = (col: { key: string; label: string }) => {
    if (!visibleColumns[col.key]) return null;

    return (
      <Table.Th
        key={col.key}
        style={{
          cursor: 'grab',
          userSelect: 'none',
          backgroundColor: draggedColumn === col.key ? 'var(--mantine-color-blue-1)' : undefined,
        }}
        draggable
        onDragStart={(e) => handleDragStart(e, col.key)}
        onDragOver={handleDragOver}
        onDrop={(e) => handleDrop(e, col.key)}
        onDragEnd={handleDragEnd}
        onClick={() => handleSort(col.key as SortField)}
      >
        <Group gap="xs" justify="space-between">
          <Text>{col.label}</Text>
          {getSortIcon(col.key as SortField)}
        </Group>
      </Table.Th>
    );
  };

  // Helper function to render a cell value based on column key
  const renderCellValue = (slice: Slice, columnKey: string) => {
    switch (columnKey) {
      case 'title':
        return editingTitleId === slice.id ? (
          <Group gap="xs" wrap="nowrap">
            <TextInput
              value={editingTitleValue}
              onChange={(e) => setEditingTitleValue(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  saveSliceTitle(slice.id);
                } else if (e.key === 'Escape') {
                  cancelEditingTitle();
                }
              }}
              size="xs"
              style={{ flex: 1 }}
              autoFocus
            />
            <ActionIcon size="xs" color="green" onClick={() => saveSliceTitle(slice.id)}>
              <IconCheck size={12} />
            </ActionIcon>
            <ActionIcon size="xs" color="red" onClick={cancelEditingTitle}>
              <IconX size={12} />
            </ActionIcon>
          </Group>
        ) : (
          <Text
            fw={500}
            style={{ cursor: 'pointer' }}
            onClick={() => startEditingTitle(slice)}
            title="Click to edit title"
            c={slice.title ? undefined : 'dimmed'}
          >
            {slice.title || '(No title)'}
          </Text>
        );
      case 'audio_file_size':
        return <Text size="sm">{formatFileSize(slice.audio_file_size)}</Text>;
      case 'audio_file_type':
        return <Text size="sm">{slice.audio_file_type}</Text>;
      case 'audio_time_length_seconds':
        return <Text size="sm">{formatAudioLength(slice.audio_time_length_seconds)}</Text>;
      case 'recording_date':
        return <Text size="sm">{formatDate(slice.recording_date)}</Text>;
      case 'transcribed':
        return (
          <Badge color={getStatusColor(slice)} variant="light">
            {getStatusText(slice)}
          </Badge>
        );
      case 'nlm_status': {
        const upload = nlmUploads[slice.id];
        if (!upload || (!upload.audio && !upload.text)) {
          return <Text size="xs" c="dimmed">-</Text>;
        }
        const parts: string[] = [];
        if (upload.audio) parts.push('Audio');
        if (upload.text) parts.push('Text');
        return (
          <Badge
            color={upload.audio && upload.text ? 'green' : 'blue'}
            variant="light"
            size="sm"
          >
            {parts.join(' + ')}
          </Badge>
        );
      }
      default:
        return null;
    }
  };

  const getSortFieldDisplayName = (field: SortField) => {
    const fieldNames: Record<SortField, string> = {
      id: 'ID',
      original_audio_file_name: 'filename',
      title: 'title',
      transcribed: 'status',
      audio_file_size: 'size',
      audio_file_type: 'type',
      estimated_time_to_transcribe: 'estimated time',
      audio_time_length_seconds: 'audio length',
      transcription: 'transcription',
      transcription_time_taken: 'transcription time',
      transcription_word_count: 'word count',
      transcription_model: 'model',
      recording_date: 'date'
    };
    return fieldNames[field] || field;
  };

  const getCurrentPageSlices = () => {
    const startIndex = (currentPage - 1) * pageSize;
    const endIndex = startIndex + pageSize;
    return filteredSlices.slice(startIndex, endIndex);
  };

  const totalPages = Math.ceil(filteredSlices.length / pageSize);
  const currentPageSlices = getCurrentPageSlices();

  return (
    <Container size="xl">
      <Stack gap="lg">
        <Group justify="space-between">
          <Title order={2}>Voice Memo Slices</Title>
          
          <Group>
            <Select
              value={currentModel}
              onChange={(value) => value && handleModelChange(value)}
              data={availableModels.map(model => ({
                value: model,
                label: getModelDisplayName(model)
              }))}
              placeholder="Select Model"
              label="Transcription Model"
              w={280}
            />
            <Select
              value={pageSize.toString()}
              onChange={(value) => setPageSize(Number(value))}
              data={['10', '25', '50', '100', '250', '500', '1000']}
              label="Per Page"
              w={100}
            />
            <TextInput
              placeholder="Search slices..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.currentTarget.value)}
              leftSection={<IconSearch size={16} />}
              label="Search"
              w={300}
            />
            <Popover position="bottom-end" withArrow shadow="md">
              <Popover.Target>
                <ActionIcon variant="light" size="lg" mt={24} title="Toggle Columns">
                  <IconColumns size={18} />
                </ActionIcon>
              </Popover.Target>
              <Popover.Dropdown>
                <Stack gap="xs">
                  <Text fw={500} size="sm">Visible Columns</Text>
                  {columnOrder.map((col) => (
                    <Checkbox
                      key={col.key}
                      label={col.label}
                      checked={visibleColumns[col.key]}
                      onChange={() => toggleColumnVisibility(col.key)}
                    />
                  ))}
                </Stack>
              </Popover.Dropdown>
            </Popover>
          </Group>
        </Group>

        <Paper p="md" withBorder>
          <Group justify="space-between">
            <Group>
              <Text size="sm">
                Showing {currentPageSlices.length} of {filteredSlices.length} slices
                {selectedSlices.length > 0 && ` (${selectedSlices.length} selected)`}
              </Text>
              <Text size="xs" c="dimmed">
                Sorted by {getSortFieldDisplayName(sortField)} ({sortDirection})
              </Text>
            </Group>
            <Group>
              <Button
                variant="outline"
                leftSection={<IconPencil size={16} />}
                onClick={updateSliceNames}
                disabled={selectedSlices.length === 0}
              >
                Update Slice Names
              </Button>
              <Button
                variant="outline"
                leftSection={<IconFileText size={16} />}
                onClick={transcribeSelected}
                disabled={selectedSlices.length === 0}
              >
                Transcribe Selected
              </Button>
              <Button
                variant="outline"
                color="teal"
                leftSection={<IconDownload size={16} />}
                onClick={exportTranscribedText}
                disabled={selectedSlices.length === 0}
              >
                Export Text
              </Button>
              <Button
                variant="outline"
                color="violet"
                leftSection={<IconUpload size={16} />}
                onClick={uploadAudioToNlm}
                loading={nlmUploading}
                disabled={selectedSlices.length === 0 || !getSelectedNotebook()}
                title={!getSelectedNotebook() ? 'Select a notebook on the NotebookLM page first' : 'Upload audio to NotebookLM'}
              >
                NLM Audio
              </Button>
              <Button
                variant="outline"
                color="violet"
                leftSection={<IconNotebook size={16} />}
                onClick={uploadTextToNlm}
                loading={nlmUploading}
                disabled={selectedSlices.length === 0 || !getSelectedNotebook()}
                title={!getSelectedNotebook() ? 'Select a notebook on the NotebookLM page first' : 'Upload text to NotebookLM'}
              >
                NLM Text
              </Button>
              <Button
                variant="outline"
                color="gray"
                leftSection={<IconBulb size={16} />}
                onClick={openSuggestFeature}
              >
                Suggest Feature
              </Button>
            </Group>
          </Group>
        </Paper>

        <Paper withBorder>
          <Table>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>
                  <Checkbox
                    checked={selectedSlices.length > 0 && currentPageSlices.every(s => selectedSlices.includes(s.id))}
                    indeterminate={selectedSlices.length > 0 && !currentPageSlices.every(s => selectedSlices.includes(s.id))}
                    onChange={toggleSelectAll}
                  />
                </Table.Th>
                {columnOrder.map((col) => renderColumnHeader(col))}
                <Table.Th>Actions</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {loading ? (
                <Table.Tr>
                  <Table.Td colSpan={Object.values(visibleColumns).filter(Boolean).length + 2}>
                    <Text ta="center" py="xl">Loading slices...</Text>
                  </Table.Td>
                </Table.Tr>
              ) : currentPageSlices.length === 0 ? (
                <Table.Tr>
                  <Table.Td colSpan={Object.values(visibleColumns).filter(Boolean).length + 2}>
                    <Text ta="center" py="xl" c="dimmed">
                      {searchTerm ? 'No slices found matching your search' : 'No slices found'}
                    </Text>
                  </Table.Td>
                </Table.Tr>
              ) : (
                currentPageSlices.map((slice) => (
                  <Table.Tr key={slice.id}>
                    <Table.Td>
                      <Checkbox
                        checked={selectedSlices.includes(slice.id)}
                        onChange={() => toggleSelectSlice(slice.id)}
                      />
                    </Table.Td>
                    {columnOrder.map((col) =>
                      visibleColumns[col.key] && (
                        <Table.Td key={col.key}>{renderCellValue(slice, col.key)}</Table.Td>
                      )
                    )}
                    <Table.Td>
                      <Group gap="xs">
                        <ActionIcon
                          variant="light"
                          color="green"
                          onClick={() => handlePlayAudio(slice)}
                          title="Play audio"
                        >
                          <IconPlayerPlay size={16} />
                        </ActionIcon>
                        <ActionIcon
                          variant="light"
                          onClick={() => {
                            setEditingSlice(slice);
                            open();
                          }}
                        >
                          <IconEdit size={16} />
                        </ActionIcon>
                        {!slice.transcribed && !transcribingSlices.includes(slice.id) && (
                          <ActionIcon
                            variant="light"
                            color="blue"
                            onClick={async () => {
                              // Set transcribing state for this slice
                              setTranscribingSlices(prev => [...prev, slice.id]);

                              try {
                                await invoke('transcribe_slices', { sliceIds: [slice.id] });
                                notifications.show({
                                  title: 'Success',
                                  message: `Started transcription for ${slice.original_audio_file_name}`,
                                  color: 'green',
                                  icon: <IconCheck size={16} />,
                                });

                                // Don't clear transcribing state here - let the polling handle it
                              } catch (error) {
                                console.error('Individual slice transcription error:', error);
                                // Clear transcribing state on error
                                setTranscribingSlices(prev => prev.filter(id => id !== slice.id));
                                notifications.show({
                                  title: 'Error',
                                  message: `Failed to start transcription: ${error}`,
                                  color: 'red',
                                  icon: <IconX size={16} />,
                                });
                              }
                            }}
                          >
                            <IconFileText size={16} />
                          </ActionIcon>
                        )}
                        {transcribingSlices.includes(slice.id) && (
                          <ActionIcon
                            variant="light"
                            color="orange"
                            loading
                          >
                            <IconFileText size={16} />
                          </ActionIcon>
                        )}
                        <ActionIcon
                          variant="light"
                          color="gray"
                          onClick={() => {
                            setDebugSlice(slice);
                            openDebug();
                          }}
                          title="Debug info"
                        >
                          <IconBug size={16} />
                        </ActionIcon>
                      </Group>
                    </Table.Td>
                  </Table.Tr>
                ))
              )}
            </Table.Tbody>
          </Table>

          {totalPages > 1 && (
            <Group justify="center" p="md">
              <Pagination
                value={currentPage}
                onChange={setCurrentPage}
                total={totalPages}
              />
            </Group>
          )}
        </Paper>

        <Modal
          opened={opened}
          onClose={close}
          title="Edit Slice"
          size="lg"
        >
          {editingSlice && (
            <Stack gap="md">
              <TextInput
                label="Filename"
                value={editingSlice.original_audio_file_name}
                onChange={(e) => setEditingSlice({
                  ...editingSlice,
                  original_audio_file_name: e.currentTarget.value
                })}
              />

              <div>
                <Text size="sm" fw={500} mb="xs">Transcription</Text>
                <QuillEditor
                  value={editingSlice.transcription || ''}
                  onChange={(value) => setEditingSlice({
                    ...editingSlice,
                    transcription: value
                  })}
                  placeholder="Enter or edit transcription..."
                  minHeight={250}
                />
              </div>

              <Group justify="flex-end">
                <Button variant="outline" onClick={close}>
                  Cancel
                </Button>
                <Button onClick={saveSlice}>
                  Save Changes
                </Button>
              </Group>
            </Stack>
          )}
        </Modal>

        <Modal
          opened={audioPlayerOpened}
          onClose={() => {
            // Clean up blob URL on close
            if (currentAudioSrc && currentAudioSrc.startsWith('blob:')) {
              URL.revokeObjectURL(currentAudioSrc);
            }
            closeAudioPlayer();
          }}
          title={currentAudioSlice ? `Playing: ${currentAudioSlice.original_audio_file_name}` : 'Audio Player'}
          size="lg"
        >
          {currentAudioSrc && (
            <AudioPlayer
              audioSrc={currentAudioSrc}
              onError={(error) => {
                notifications.show({
                  title: 'Audio Error',
                  message: error,
                  color: 'red',
                  icon: <IconX size={16} />,
                });
              }}
            />
          )}
        </Modal>

        <Modal
          opened={debugOpened}
          onClose={closeDebug}
          title="Debug Information"
          size="xl"
        >
          {debugSlice && (
            <Stack gap="md">
              <div>
                <Text fw={700} size="lg" mb="xs">Slice Table Fields</Text>
                <Table striped highlightOnHover>
                  <Table.Tbody>
                    <Table.Tr>
                      <Table.Td fw={600}>id</Table.Td>
                      <Table.Td>{debugSlice.id}</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>original_audio_file_name</Table.Td>
                      <Table.Td>{debugSlice.original_audio_file_name}</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>transcribed</Table.Td>
                      <Table.Td>{debugSlice.transcribed ? 'true' : 'false'}</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>audio_file_size</Table.Td>
                      <Table.Td>{debugSlice.audio_file_size} bytes</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>audio_file_type</Table.Td>
                      <Table.Td>{debugSlice.audio_file_type}</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>estimated_time_to_transcribe</Table.Td>
                      <Table.Td>{debugSlice.estimated_time_to_transcribe} seconds</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>audio_time_length_seconds</Table.Td>
                      <Table.Td>{debugSlice.audio_time_length_seconds !== null ? `${debugSlice.audio_time_length_seconds.toFixed(2)} seconds (${formatAudioLength(debugSlice.audio_time_length_seconds)})` : 'null'}</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>transcription</Table.Td>
                      <Table.Td style={{ maxWidth: '400px', wordWrap: 'break-word' }}>
                        {debugSlice.transcription || 'null'}
                      </Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>transcription_time_taken</Table.Td>
                      <Table.Td>{debugSlice.transcription_time_taken || 'null'} seconds</Table.Td>
                    </Table.Tr>
                    <Table.Tr>
                      <Table.Td fw={600}>transcription_word_count</Table.Td>
                      <Table.Td>{debugSlice.transcription_word_count || 'null'}</Table.Td>
                    </Table.Tr>
                  </Table.Tbody>
                </Table>
              </div>

              <div>
                <Text fw={700} size="lg" mb="xs">Recording Table Fields (via JOIN)</Text>
                <Table striped highlightOnHover>
                  <Table.Tbody>
                    <Table.Tr>
                      <Table.Td fw={600}>title</Table.Td>
                      <Table.Td>{debugSlice.title || 'null'}</Table.Td>
                    </Table.Tr>
                  </Table.Tbody>
                </Table>
                <Text size="sm" c="dimmed" mt="xs">
                  Note: Other recording fields are not currently included in the slice query.
                  The title field is joined from the recordings table where copied_path matches the slice filename.
                </Text>
              </div>
            </Stack>
          )}
        </Modal>

        {/* Fancy Transcription Progress Bar - Draggable */}
        <Transition mounted={transcriptionProgress !== null && transcriptionProgress.is_active} transition="slide-up" duration={400}>
          {(styles) => (
            <Box
              style={{
                ...styles,
                position: 'fixed',
                bottom: 24,
                left: '50%',
                transform: 'translateX(-50%)',
                zIndex: 1000,
                width: 'min(600px, calc(100vw - 48px))',
              }}
            >
              <DraggableCard shadow="xl" padding="lg" radius="lg" withBorder style={{ background: 'var(--mantine-color-body)' }}>
                <Stack gap="md">
                  {/* Header */}
                  <Group justify="space-between" align="center">
                    <Group gap="sm">
                      <ThemeIcon size="lg" radius="md" variant="light" color="blue">
                        <IconWaveSquare size={20} />
                      </ThemeIcon>
                      <div>
                        <Text fw={600} size="sm">Transcribing Audio</Text>
                        <Text size="xs" c="dimmed">
                          {transcriptionProgress?.current_step || 'Processing...'}
                        </Text>
                      </div>
                    </Group>
                    <Group gap="xs">
                      <RingProgress
                        size={48}
                        thickness={4}
                        roundCaps
                        sections={[
                          { value: getProgressPercentage(), color: 'blue' },
                          { value: transcriptionProgress ? (transcriptionProgress.failed_slices / transcriptionProgress.total_slices) * 100 : 0, color: 'red' },
                        ]}
                        label={
                          <Text size="xs" ta="center" fw={700}>
                            {getProgressPercentage()}%
                          </Text>
                        }
                      />
                    </Group>
                  </Group>

                  {/* Progress Bar */}
                  <Progress.Root size="lg" radius="md">
                    <Progress.Section
                      value={getProgressPercentage()}
                      color="blue"
                      animated
                    >
                      <Progress.Label>{getProgressPercentage()}%</Progress.Label>
                    </Progress.Section>
                    {transcriptionProgress && transcriptionProgress.failed_slices > 0 && (
                      <Progress.Section
                        value={(transcriptionProgress.failed_slices / transcriptionProgress.total_slices) * 100}
                        color="red"
                      />
                    )}
                  </Progress.Root>

                  {/* Stats Row */}
                  <Group justify="space-between" gap="xl">
                    <Group gap="lg">
                      <Group gap="xs">
                        <IconCircleCheck size={16} style={{ color: 'var(--mantine-color-green-6)' }} />
                        <Text size="sm">
                          <Text span fw={600}>{transcriptionProgress?.completed_slices || 0}</Text>
                          <Text span c="dimmed"> / {transcriptionProgress?.total_slices || 0}</Text>
                        </Text>
                      </Group>
                      {transcriptionProgress && transcriptionProgress.failed_slices > 0 && (
                        <Group gap="xs">
                          <IconAlertCircle size={16} style={{ color: 'var(--mantine-color-red-6)' }} />
                          <Text size="sm" c="red">{transcriptionProgress.failed_slices} failed</Text>
                        </Group>
                      )}
                    </Group>
                    <Group gap="xs">
                      <IconClock size={16} style={{ color: 'var(--mantine-color-dimmed)' }} />
                      <Text size="sm" c="dimmed">
                        {formatElapsedTime(transcriptionProgress?.elapsed_seconds || 0)}
                        {transcriptionProgress && transcriptionProgress.estimated_total_seconds > 0 && (
                          <Text span c="dimmed"> / {formatElapsedTime(transcriptionProgress.estimated_total_seconds)}</Text>
                        )}
                      </Text>
                    </Group>
                  </Group>

                  {/* Current File */}
                  {transcriptionProgress?.current_slice_name && (
                    <Text size="xs" c="dimmed" ta="center" truncate="end" style={{ maxWidth: '100%' }}>
                      {transcriptionProgress.current_slice_name}
                    </Text>
                  )}
                </Stack>
              </DraggableCard>
            </Box>
          )}
        </Transition>

        {/* Completion Toast - Draggable */}
        <Transition mounted={transcriptionProgress !== null && !transcriptionProgress.is_active} transition="slide-up" duration={400}>
          {(styles) => (
            <Box
              style={{
                ...styles,
                position: 'fixed',
                bottom: 24,
                left: '50%',
                transform: 'translateX(-50%)',
                zIndex: 1000,
                width: 'min(400px, calc(100vw - 48px))',
              }}
            >
              <DraggableCard shadow="xl" padding="md" radius="lg" withBorder style={{ background: 'var(--mantine-color-body)' }}>
                <Group justify="center" gap="sm">
                  <ThemeIcon size="lg" radius="xl" variant="light" color="green">
                    <IconCircleCheck size={20} />
                  </ThemeIcon>
                  <div>
                    <Text fw={600} size="sm" c="green">Transcription Complete!</Text>
                    <Text size="xs" c="dimmed">
                      {transcriptionProgress?.completed_slices || 0} slices transcribed
                      {transcriptionProgress && transcriptionProgress.failed_slices > 0 && `, ${transcriptionProgress.failed_slices} failed`}
                    </Text>
                  </div>
                </Group>
              </DraggableCard>
            </Box>
          )}
        </Transition>
      </Stack>
    </Container>
  );
} 