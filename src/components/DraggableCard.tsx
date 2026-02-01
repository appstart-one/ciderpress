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

import { useState, useRef, useEffect, ReactNode } from 'react';
import { Card, CardProps, Box, ThemeIcon } from '@mantine/core';
import { IconGripHorizontal } from '@tabler/icons-react';

interface DraggableCardProps extends Omit<CardProps, 'children'> {
  children: ReactNode;
  initialX?: number;
  initialY?: number;
  showDragHandle?: boolean;
}

export function DraggableCard({
  children,
  initialX,
  initialY,
  showDragHandle = true,
  style,
  ...cardProps
}: DraggableCardProps) {
  // Track drag offset from initial position (using translate)
  const [dragOffset, setDragOffset] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const dragStartPos = useRef({ mouseX: 0, mouseY: 0, offsetX: 0, offsetY: 0 });
  const cardRef = useRef<HTMLDivElement>(null);

  const handleMouseDown = (e: React.MouseEvent) => {
    // Only start dragging from the drag handle area
    const target = e.target as HTMLElement;
    const isDragHandleEl = target.closest('[data-drag-handle]');

    if (!isDragHandleEl && showDragHandle) {
      return;
    }

    e.preventDefault();

    // Store the starting mouse position and current offset
    dragStartPos.current = {
      mouseX: e.clientX,
      mouseY: e.clientY,
      offsetX: dragOffset.x,
      offsetY: dragOffset.y,
    };

    setIsDragging(true);
  };

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      // Calculate how far the mouse has moved from the start
      const deltaX = e.clientX - dragStartPos.current.mouseX;
      const deltaY = e.clientY - dragStartPos.current.mouseY;

      // New offset is starting offset plus delta
      const newOffsetX = dragStartPos.current.offsetX + deltaX;
      const newOffsetY = dragStartPos.current.offsetY + deltaY;

      setDragOffset({ x: newOffsetX, y: newOffsetY });
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging]);

  // Use transform: translate for movement - this doesn't change position/layout
  const dragStyle: React.CSSProperties = {
    transform: dragOffset.x !== 0 || dragOffset.y !== 0
      ? `translate(${dragOffset.x}px, ${dragOffset.y}px)`
      : undefined,
  };

  const combinedStyle: React.CSSProperties = {
    ...(style as React.CSSProperties),
    ...dragStyle,
    cursor: isDragging ? 'grabbing' : undefined,
    userSelect: isDragging ? 'none' : undefined,
    zIndex: isDragging ? 10001 : undefined,
  };

  return (
    <Card
      ref={cardRef}
      onMouseDown={handleMouseDown}
      style={combinedStyle}
      {...cardProps}
    >
      {showDragHandle && (
        <Box
          data-drag-handle
          style={{
            cursor: 'grab',
            padding: '4px 0',
            marginBottom: 8,
            display: 'flex',
            justifyContent: 'center',
          }}
        >
          <ThemeIcon variant="subtle" size="xs" color="gray">
            <IconGripHorizontal size={14} />
          </ThemeIcon>
        </Box>
      )}
      {children}
    </Card>
  );
}