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
  TextInput,
  Button,
  Group,
  Text,
  Stack,
  Alert,
  Table,
  ColorInput,
  ActionIcon,
  Loader
} from '@mantine/core';
import { IconCheck, IconX, IconPlus, IconTrash, IconTags } from '@tabler/icons-react';

interface Label {
  id: number | null;
  name: string;
  color: string;
  keywords: string;
}

export default function LabelSettings() {
  const [labels, setLabels] = useState<Label[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<number | null>(null);

  useEffect(() => {
    loadLabels();
  }, []);

  const loadLabels = async () => {
    setLoading(true);
    try {
      const loadedLabels = await invoke<Label[]>('list_labels');
      setLabels(loadedLabels);
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to load labels',
        color: 'red',
        icon: <IconX size={16} />,
      });
    } finally {
      setLoading(false);
    }
  };

  const addLabel = async () => {
    const newLabel: Label = {
      id: null,
      name: 'New Label',
      color: '#228be6',
      keywords: '',
    };

    try {
      const newId = await invoke<number>('create_label', { label: newLabel });
      newLabel.id = newId;
      setLabels([...labels, newLabel]);
      notifications.show({
        title: 'Success',
        message: 'Label created',
        color: 'green',
        icon: <IconCheck size={16} />,
      });
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to create label',
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const updateLabel = async (index: number, field: keyof Label, value: string) => {
    const updatedLabels = [...labels];
    updatedLabels[index] = { ...updatedLabels[index], [field]: value };
    setLabels(updatedLabels);
  };

  const saveLabel = async (index: number) => {
    const label = labels[index];
    if (!label.id) return;

    setSaving(label.id);
    try {
      await invoke('update_label', { id: label.id, label });
      notifications.show({
        title: 'Saved',
        message: `Label "${label.name}" saved`,
        color: 'green',
        icon: <IconCheck size={16} />,
      });
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to save label',
        color: 'red',
        icon: <IconX size={16} />,
      });
    } finally {
      setSaving(null);
    }
  };

  const deleteLabel = async (index: number) => {
    const label = labels[index];
    if (!label.id) return;

    try {
      await invoke('delete_label', { id: label.id });
      setLabels(labels.filter((_, i) => i !== index));
      notifications.show({
        title: 'Deleted',
        message: `Label "${label.name}" deleted`,
        color: 'orange',
        icon: <IconTrash size={16} />,
      });
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to delete label',
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  if (loading) {
    return (
      <Container size="md">
        <Group justify="center" p="xl">
          <Loader size="md" />
          <Text>Loading labels...</Text>
        </Group>
      </Container>
    );
  }

  return (
    <Container size="lg">
      <Stack gap="lg">
        <Title order={2}>Label Settings</Title>

        <Alert icon={<IconTags size={16} />} title="About Labels" color="blue">
          Define labels with keywords that will automatically match transcriptions.
          Add comma-separated keywords to match text in your voice memo transcriptions.
        </Alert>

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Group justify="space-between">
              <Title order={3}>Labels</Title>
              <Button
                leftSection={<IconPlus size={16} />}
                onClick={addLabel}
              >
                Add Label
              </Button>
            </Group>

            {labels.length === 0 ? (
              <Text c="dimmed" ta="center" py="xl">
                No labels defined yet. Click "Add Label" to create one.
              </Text>
            ) : (
              <Table striped highlightOnHover>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th style={{ width: '200px' }}>Label Name</Table.Th>
                    <Table.Th style={{ width: '120px' }}>Color</Table.Th>
                    <Table.Th>Keywords (comma-separated)</Table.Th>
                    <Table.Th style={{ width: '120px' }}>Actions</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {labels.map((label, index) => (
                    <Table.Tr key={label.id || index}>
                      <Table.Td>
                        <TextInput
                          value={label.name}
                          onChange={(e) => updateLabel(index, 'name', e.target.value)}
                          onBlur={() => saveLabel(index)}
                          size="sm"
                          placeholder="Label name"
                        />
                      </Table.Td>
                      <Table.Td>
                        <ColorInput
                          value={label.color}
                          onChange={(value) => updateLabel(index, 'color', value)}
                          onBlur={() => saveLabel(index)}
                          size="sm"
                          swatches={[
                            '#228be6', '#40c057', '#fab005', '#fd7e14',
                            '#fa5252', '#be4bdb', '#7950f2', '#15aabf',
                            '#12b886', '#82c91e', '#e64980', '#495057'
                          ]}
                        />
                      </Table.Td>
                      <Table.Td>
                        <TextInput
                          value={label.keywords}
                          onChange={(e) => updateLabel(index, 'keywords', e.target.value)}
                          onBlur={() => saveLabel(index)}
                          size="sm"
                          placeholder="keyword1, keyword2, keyword3"
                        />
                      </Table.Td>
                      <Table.Td>
                        <Group gap="xs">
                          <ActionIcon
                            variant="light"
                            color="green"
                            onClick={() => saveLabel(index)}
                            loading={saving === label.id}
                            title="Save"
                          >
                            <IconCheck size={16} />
                          </ActionIcon>
                          <ActionIcon
                            variant="light"
                            color="red"
                            onClick={() => deleteLabel(index)}
                            title="Delete"
                          >
                            <IconTrash size={16} />
                          </ActionIcon>
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            )}
          </Stack>
        </Paper>

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={4}>How Keywords Work</Title>
            <Stack gap="xs">
              <Text size="sm">Keywords are matched against transcript text (case-insensitive).</Text>
              <Text size="sm">Separate multiple keywords with commas.</Text>
              <Text size="sm">Example: "meeting, standup, weekly" will match any transcription containing those words.</Text>
            </Stack>
          </Stack>
        </Paper>
      </Stack>
    </Container>
  );
}