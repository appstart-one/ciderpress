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

import { ActionIcon, Tooltip, Menu, Group, Box, Text } from '@mantine/core';
import { IconSun, IconMoon, IconCheck } from '@tabler/icons-react';
import { useTheme } from '../contexts/ThemeContext';
import { themeList } from '../themes';

export const ThemeToggle = () => {
  const { colorScheme, themeId, setThemeId } = useTheme();

  return (
    <Menu shadow="md" width={220} position="bottom-end" withinPortal>
      <Menu.Target>
        <Tooltip label="Change theme">
          <ActionIcon variant="outline" size="lg" aria-label="Change theme">
            {colorScheme === 'dark' ? <IconSun size={18} /> : <IconMoon size={18} />}
          </ActionIcon>
        </Tooltip>
      </Menu.Target>

      <Menu.Dropdown>
        <Menu.Label>Theme</Menu.Label>
        {themeList.map((t) => (
          <Menu.Item
            key={t.id}
            onClick={() => setThemeId(t.id)}
            leftSection={
              <Group gap={4} wrap="nowrap">
                <Box
                  style={{
                    width: 12,
                    height: 12,
                    borderRadius: 3,
                    backgroundColor: t.swatch.surface,
                    border: '1px solid var(--mantine-color-default-border)',
                  }}
                />
                <Box
                  style={{
                    width: 12,
                    height: 12,
                    borderRadius: 3,
                    backgroundColor: t.swatch.accent,
                  }}
                />
              </Group>
            }
            rightSection={
              t.id === themeId ? <IconCheck size={16} /> : undefined
            }
          >
            <Group justify="space-between" gap="xs" wrap="nowrap">
              <Text size="sm">{t.label}</Text>
              <Text size="xs" c="dimmed" tt="capitalize">{t.base}</Text>
            </Group>
          </Menu.Item>
        ))}
      </Menu.Dropdown>
    </Menu>
  );
}; 