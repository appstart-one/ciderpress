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

import React from 'react';
import ReactQuill from 'react-quill';
import 'react-quill/dist/quill.snow.css';
import { Box } from '@mantine/core';

interface QuillEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  readOnly?: boolean;
  minHeight?: number;
}

export const QuillEditor: React.FC<QuillEditorProps> = ({
  value,
  onChange,
  placeholder = "Enter transcription...",
  readOnly = false,
  minHeight = 200
}) => {
  // Handle Quill onChange - it sometimes returns '<p><br></p>' for empty content
  const handleChange = (content: string) => {
    // Convert empty Quill content to empty string
    if (content === '<p><br></p>' || content.trim() === '') {
      onChange('');
    } else {
      onChange(content);
    }
  };

  // Custom toolbar configuration for transcription editing
  const modules = {
    toolbar: [
      [{ 'header': [1, 2, 3, false] }],
      ['bold', 'italic', 'underline', 'strike'],
      [{ 'list': 'ordered'}, { 'list': 'bullet' }],
      [{ 'indent': '-1'}, { 'indent': '+1' }],
      ['blockquote', 'code-block'],
      ['link'],
      ['clean'] // remove formatting button
    ],
  };

  const formats = [
    'header',
    'bold', 'italic', 'underline', 'strike',
    'list', 'bullet', 'indent',
    'blockquote', 'code-block',
    'link'
  ];

  return (
    <Box
      style={{
        '& .ql-editor': {
          minHeight: `${minHeight}px`,
          fontSize: '14px',
          lineHeight: '1.5'
        },
        '& .ql-toolbar': {
          borderTop: '1px solid #ccc',
          borderLeft: '1px solid #ccc',
          borderRight: '1px solid #ccc',
          borderRadius: '4px 4px 0 0'
        },
        '& .ql-container': {
          borderBottom: '1px solid #ccc',
          borderLeft: '1px solid #ccc',
          borderRight: '1px solid #ccc',
          borderRadius: '0 0 4px 4px'
        }
      }}
    >
      <ReactQuill
        value={value}
        onChange={handleChange}
        modules={modules}
        formats={formats}
        placeholder={placeholder}
        readOnly={readOnly}
        theme="snow"
      />
    </Box>
  );
};