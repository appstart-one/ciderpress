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

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Title,
  Text,
  Button,
  Card,
  Group,
  Stack,
  Badge,
  Select,
  Alert,
  Loader,
  TextInput,
  Modal,
  Code,
} from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { IconBrandGoogle, IconRefresh, IconCheck, IconX, IconNotebook, IconPlus, IconUser, IconInfoCircle, IconExternalLink } from '@tabler/icons-react';

interface NlmStatus {
  authenticated: boolean;
  binary_available: boolean;
  binary_path: string | null;
  current_profile: string | null;
}

interface NlmNotebook {
  id: string;
  title: string;
}

interface NlmBrowserProfile {
  name: string;
  display_name: string;
}

interface NlmNotebookDetails {
  id: string;
  title: string;
  sources: string;
  notes: string;
  analytics: string;
}

export default function NotebookLM() {
  const [status, setStatus] = useState<NlmStatus | null>(null);
  const [notebooks, setNotebooks] = useState<NlmNotebook[]>([]);
  const [selectedNotebook, setSelectedNotebook] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [authLoading, setAuthLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [authMessage, setAuthMessage] = useState<string | null>(null);

  // Account/profile management
  const [profiles, setProfiles] = useState<NlmBrowserProfile[]>([]);
  const [profilesLoading, setProfilesLoading] = useState(false);
  const [switchingProfile, setSwitchingProfile] = useState(false);

  // Create notebook
  const [newNotebookTitle, setNewNotebookTitle] = useState('');
  const [creatingNotebook, setCreatingNotebook] = useState(false);

  // Notebook details modal
  const [detailsOpened, { open: openDetails, close: closeDetails }] = useDisclosure(false);
  const [notebookDetails, setNotebookDetails] = useState<NlmNotebookDetails | null>(null);
  const [detailsLoading, setDetailsLoading] = useState(false);

  const checkStatus = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const nlmStatus = await invoke<NlmStatus>('nlm_get_status');
      setStatus(nlmStatus);

      if (nlmStatus.authenticated) {
        try {
          const notebookList = await invoke<NlmNotebook[]>('nlm_list_notebooks');
          setNotebooks(notebookList || []);
          // Restore previously selected notebook from localStorage
          const saved = localStorage.getItem('nlm_selected_notebook');
          if (saved && notebookList?.some(n => n.id === saved)) {
            setSelectedNotebook(saved);
          }
        } catch (notebookErr: any) {
          console.error('Failed to list notebooks:', notebookErr);
          // Don't fail the whole status check for notebook list failure
          setNotebooks([]);
        }
      }
    } catch (e: any) {
      console.error('NLM status check failed:', e);
      const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e);
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadProfiles = useCallback(async () => {
    try {
      setProfilesLoading(true);
      const profileList = await invoke<NlmBrowserProfile[]>('nlm_list_profiles');
      setProfiles(profileList || []);
    } catch (e: any) {
      console.error('Failed to load profiles:', e);
      // Non-critical, don't show error
    } finally {
      setProfilesLoading(false);
    }
  }, []);

  useEffect(() => {
    checkStatus();
  }, [checkStatus]);

  // Load profiles separately, only when status shows binary is available
  useEffect(() => {
    if (status?.binary_available) {
      loadProfiles();
    }
  }, [status?.binary_available, loadProfiles]);

  const handleAuth = async () => {
    try {
      setAuthLoading(true);
      setError(null);
      setAuthMessage(null);
      const result = await invoke<string>('nlm_authenticate');
      setAuthMessage(result || 'Authentication initiated. Check your browser.');
      await checkStatus();
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e);
      setError(msg);
    } finally {
      setAuthLoading(false);
    }
  };

  const handleAuthWithProfile = async (profileName: string) => {
    try {
      setSwitchingProfile(true);
      setError(null);
      setAuthMessage(null);
      const result = await invoke<string>('nlm_auth_with_profile', { profileName });
      setAuthMessage(result || `Switched to profile: ${profileName}`);
      await checkStatus();
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e);
      setError(msg);
    } finally {
      setSwitchingProfile(false);
    }
  };

  const handleNotebookSelect = (value: string | null) => {
    setSelectedNotebook(value);
    if (value) {
      localStorage.setItem('nlm_selected_notebook', value);
    } else {
      localStorage.removeItem('nlm_selected_notebook');
    }
  };

  const handleRefreshNotebooks = async () => {
    try {
      setLoading(true);
      const notebookList = await invoke<NlmNotebook[]>('nlm_list_notebooks');
      setNotebooks(notebookList || []);
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e);
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleCreateNotebook = async () => {
    if (!newNotebookTitle.trim()) return;
    try {
      setCreatingNotebook(true);
      setError(null);
      await invoke<string>('nlm_create_notebook', { title: newNotebookTitle.trim() });
      setNewNotebookTitle('');
      await handleRefreshNotebooks();
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e);
      setError(msg);
    } finally {
      setCreatingNotebook(false);
    }
  };

  const handleShowDetails = async () => {
    if (!selectedNotebook) return;
    const notebook = notebooks.find(n => n.id === selectedNotebook);
    try {
      setDetailsLoading(true);
      setNotebookDetails(null);
      openDetails();
      const details = await invoke<NlmNotebookDetails>('nlm_get_notebook_details', {
        notebookId: selectedNotebook,
        title: notebook?.title || selectedNotebook,
      });
      setNotebookDetails(details);
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : e?.message || JSON.stringify(e);
      setError(msg);
      closeDetails();
    } finally {
      setDetailsLoading(false);
    }
  };

  if (loading && !status) {
    return (
      <Stack align="center" pt="xl">
        <Loader size="lg" />
        <Text c="dimmed">Checking NotebookLM status...</Text>
      </Stack>
    );
  }

  return (
    <Stack gap="lg">
      <Group justify="space-between">
        <Title order={2}>NotebookLM</Title>
        <Group gap="xs">
          <Button
            variant="subtle"
            leftSection={<IconExternalLink size={16} />}
            onClick={() => invoke('open_url', { url: 'https://notebooklm.google.com' })}
          >
            Open NotebookLM
          </Button>
          <Button
            variant="subtle"
            leftSection={<IconRefresh size={16} />}
            onClick={checkStatus}
            loading={loading}
          >
            Refresh
          </Button>
        </Group>
      </Group>

      {error && (
        <Alert color="red" title="Error" onClose={() => setError(null)} withCloseButton>
          {error}
        </Alert>
      )}

      {authMessage && (
        <Alert color="blue" title="Authentication" onClose={() => setAuthMessage(null)} withCloseButton>
          <Text size="sm" style={{ whiteSpace: 'pre-wrap' }}>{authMessage}</Text>
        </Alert>
      )}

      {/* Status Card */}
      <Card withBorder>
        <Stack gap="sm">
          <Group justify="space-between">
            <Text fw={500}>Connection Status</Text>
            <Group gap="xs">
              <Badge
                color={status?.binary_available ? 'green' : 'red'}
                leftSection={status?.binary_available ? <IconCheck size={12} /> : <IconX size={12} />}
              >
                NLM Binary
              </Badge>
              <Badge
                color={status?.authenticated ? 'green' : 'yellow'}
                leftSection={status?.authenticated ? <IconCheck size={12} /> : <IconX size={12} />}
              >
                {status?.authenticated ? 'Authenticated' : 'Not Authenticated'}
              </Badge>
            </Group>
          </Group>

          {/* Current Account/Profile Display */}
          {status?.current_profile && (
            <Group gap="xs">
              <IconUser size={16} />
              <Text size="sm" c="dimmed">
                Active profile: <strong>{status.current_profile}</strong>
              </Text>
            </Group>
          )}

          {!status?.binary_available && (
            <Alert color="red" title="NLM Binary Not Found">
              The NLM command-line tool was not found. Run scripts/build-nlm.sh to build it.
            </Alert>
          )}
        </Stack>
      </Card>

      {/* Authentication / Account Switching Card */}
      {status?.binary_available && (
        <Card withBorder>
          <Stack gap="md">
            <Text fw={500}>
              {status?.authenticated ? 'Account Management' : 'Authenticate with Google'}
            </Text>

            {!status?.authenticated && (
              <Text size="sm" c="dimmed">
                Connect your Google account to access NotebookLM. This will open your browser for authentication.
              </Text>
            )}

            <Group>
              <Button
                leftSection={<IconBrandGoogle size={16} />}
                onClick={handleAuth}
                loading={authLoading}
                variant={status?.authenticated ? 'light' : 'filled'}
              >
                {status?.authenticated ? 'Re-authenticate' : 'Authenticate'}
              </Button>
            </Group>

            {/* Profile switching */}
            {profilesLoading && <Loader size="sm" />}
            {profiles.length > 0 && (
              <Stack gap="xs">
                <Text size="sm" fw={500}>Switch Browser Profile</Text>
                <Text size="xs" c="dimmed">
                  Authenticate with a different Chrome/Chromium profile to access another Google account's notebooks.
                </Text>
                <Select
                  placeholder="Select a browser profile..."
                  data={profiles.map(p => ({ value: p.name, label: p.display_name }))}
                  onChange={(value) => value && handleAuthWithProfile(value)}
                  disabled={switchingProfile}
                  leftSection={<IconUser size={16} />}
                  searchable
                  clearable
                />
              </Stack>
            )}
          </Stack>
        </Card>
      )}

      {/* Notebook Management */}
      {status?.authenticated && (
        <Card withBorder>
          <Stack gap="md">
            <Group justify="space-between">
              <Text fw={500}>Notebooks</Text>
              <Button
                variant="subtle"
                size="xs"
                leftSection={<IconRefresh size={14} />}
                onClick={handleRefreshNotebooks}
                loading={loading}
              >
                Refresh List
              </Button>
            </Group>

            {/* Create new notebook */}
            <Group>
              <TextInput
                placeholder="New notebook title..."
                value={newNotebookTitle}
                onChange={(e) => setNewNotebookTitle(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleCreateNotebook();
                }}
                style={{ flex: 1 }}
              />
              <Button
                leftSection={<IconPlus size={16} />}
                onClick={handleCreateNotebook}
                loading={creatingNotebook}
                disabled={!newNotebookTitle.trim()}
              >
                Create
              </Button>
            </Group>

            {/* Select active notebook */}
            <Text size="sm" c="dimmed">
              Select a notebook to upload audio and transcriptions to from the Slices screen.
            </Text>
            {notebooks.length > 0 ? (
              <Select
                placeholder="Choose a notebook..."
                data={notebooks.map(n => ({ value: n.id, label: n.title }))}
                value={selectedNotebook}
                onChange={handleNotebookSelect}
                leftSection={<IconNotebook size={16} />}
                searchable
                clearable
              />
            ) : (
              <Text size="sm" c="dimmed" fs="italic">
                No notebooks found. Create one above or click Refresh.
              </Text>
            )}

            {selectedNotebook && (
              <Stack gap="xs">
                <Alert color="green" title="Active Notebook">
                  <Text size="sm">
                    Uploads from the Slices screen will go to: <strong>{notebooks.find(n => n.id === selectedNotebook)?.title || selectedNotebook}</strong>
                  </Text>
                </Alert>
                <Button
                  variant="light"
                  leftSection={<IconInfoCircle size={16} />}
                  onClick={handleShowDetails}
                  loading={detailsLoading}
                >
                  Show Notebook Info
                </Button>
              </Stack>
            )}
          </Stack>
        </Card>
      )}

      {/* Notebook Details Modal */}
      <Modal
        opened={detailsOpened}
        onClose={closeDetails}
        title={notebookDetails ? `Notebook: ${notebookDetails.title}` : 'Notebook Details'}
        size="lg"
      >
        {detailsLoading ? (
          <Stack align="center" py="xl">
            <Loader size="md" />
            <Text size="sm" c="dimmed">Fetching notebook details...</Text>
          </Stack>
        ) : notebookDetails ? (
          <Stack gap="md">
            <div>
              <Text size="sm" fw={500} mb={4}>ID</Text>
              <Code>{notebookDetails.id}</Code>
            </div>

            <div>
              <Text size="sm" fw={500} mb={4}>Sources</Text>
              <Code block style={{ whiteSpace: 'pre-wrap', maxHeight: 200, overflow: 'auto' }}>
                {notebookDetails.sources || 'No sources found.'}
              </Code>
            </div>

            <div>
              <Text size="sm" fw={500} mb={4}>Notes</Text>
              <Code block style={{ whiteSpace: 'pre-wrap', maxHeight: 200, overflow: 'auto' }}>
                {notebookDetails.notes || 'No notes found.'}
              </Code>
            </div>

            <div>
              <Text size="sm" fw={500} mb={4}>Analytics</Text>
              <Code block style={{ whiteSpace: 'pre-wrap', maxHeight: 200, overflow: 'auto' }}>
                {notebookDetails.analytics || 'No analytics available.'}
              </Code>
            </div>
          </Stack>
        ) : null}
      </Modal>
    </Stack>
  );
}