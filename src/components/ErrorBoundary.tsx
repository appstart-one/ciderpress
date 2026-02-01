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

import { Component, ReactNode } from 'react';
import { Alert, Stack, Button, Code, Text } from '@mantine/core';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  render() {
    if (this.state.hasError) {
      return (
        <Stack p="md" gap="md">
          <Alert color="red" title="Something went wrong">
            <Text size="sm" mb="xs">An error occurred while rendering this page.</Text>
            <Code block>{this.state.error?.message || 'Unknown error'}</Code>
            {this.state.error?.stack && (
              <Code block mt="xs" style={{ fontSize: '11px', maxHeight: 200, overflow: 'auto' }}>
                {this.state.error.stack}
              </Code>
            )}
          </Alert>
          <Button
            variant="light"
            onClick={() => this.setState({ hasError: false, error: null })}
          >
            Try Again
          </Button>
        </Stack>
      );
    }

    return this.props.children;
  }
}