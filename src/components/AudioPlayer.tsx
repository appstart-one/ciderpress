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

import { useState, useRef, useEffect } from 'react';
import { ActionIcon, Group, Slider, Text, Stack, Button, Badge } from '@mantine/core';
import { IconPlayerPlay, IconPlayerPause, IconVolume, IconVolumeOff, IconChevronUp, IconChevronDown } from '@tabler/icons-react';

interface AudioPlayerProps {
  audioSrc: string;
  onError?: (error: string) => void;
}

export function AudioPlayer({ audioSrc, onError }: AudioPlayerProps) {
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(1);
  const [isMuted, setIsMuted] = useState(false);
  const [playbackSpeed, setPlaybackSpeed] = useState(1);
  const audioRef = useRef<HTMLAudioElement>(null);

  const SPEED_OPTIONS = [0.5, 1, 1.25, 1.5, 1.75, 2];

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    const handleTimeUpdate = () => {
      setCurrentTime(audio.currentTime);
    };

    const handleDurationChange = () => {
      setDuration(audio.duration);
    };

    const handleEnded = () => {
      setIsPlaying(false);
      setCurrentTime(0);
    };

    const handleError = () => {
      setIsPlaying(false);
      onError?.('Failed to load audio file');
    };

    audio.addEventListener('timeupdate', handleTimeUpdate);
    audio.addEventListener('durationchange', handleDurationChange);
    audio.addEventListener('ended', handleEnded);
    audio.addEventListener('error', handleError);

    return () => {
      audio.removeEventListener('timeupdate', handleTimeUpdate);
      audio.removeEventListener('durationchange', handleDurationChange);
      audio.removeEventListener('ended', handleEnded);
      audio.removeEventListener('error', handleError);
    };
  }, [onError]);

  const togglePlayPause = () => {
    const audio = audioRef.current;
    if (!audio) return;

    if (isPlaying) {
      audio.pause();
    } else {
      audio.play();
    }
    setIsPlaying(!isPlaying);
  };

  const handleSeek = (value: number) => {
    const audio = audioRef.current;
    if (!audio) return;

    audio.currentTime = value;
    setCurrentTime(value);
  };

  const handleVolumeChange = (value: number) => {
    const audio = audioRef.current;
    if (!audio) return;

    audio.volume = value;
    setVolume(value);
    setIsMuted(value === 0);
  };

  const toggleMute = () => {
    const audio = audioRef.current;
    if (!audio) return;

    if (isMuted) {
      audio.volume = volume;
      setIsMuted(false);
    } else {
      audio.volume = 0;
      setIsMuted(true);
    }
  };

  const handleSpeedChange = (speed: number) => {
    const audio = audioRef.current;
    if (!audio) return;

    audio.playbackRate = speed;
    setPlaybackSpeed(speed);
  };

  const increaseSpeed = () => {
    const currentIndex = SPEED_OPTIONS.indexOf(playbackSpeed);
    if (currentIndex < SPEED_OPTIONS.length - 1) {
      handleSpeedChange(SPEED_OPTIONS[currentIndex + 1]);
    }
  };

  const decreaseSpeed = () => {
    const currentIndex = SPEED_OPTIONS.indexOf(playbackSpeed);
    if (currentIndex > 0) {
      handleSpeedChange(SPEED_OPTIONS[currentIndex - 1]);
    }
  };

  const formatTime = (seconds: number) => {
    if (!isFinite(seconds)) return '0:00';
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  return (
    <Stack gap="md">
      <audio ref={audioRef} src={audioSrc} />

      <Group gap="md" wrap="nowrap">
        <ActionIcon
          size="lg"
          variant="filled"
          onClick={togglePlayPause}
          aria-label={isPlaying ? 'Pause' : 'Play'}
        >
          {isPlaying ? <IconPlayerPause size={20} /> : <IconPlayerPlay size={20} />}
        </ActionIcon>

        <Stack gap={0} style={{ flex: 1 }}>
          <Slider
            value={currentTime}
            onChange={handleSeek}
            max={duration || 100}
            min={0}
            step={0.1}
            size="sm"
            style={{ width: '100%' }}
          />
          <Group justify="space-between">
            <Text size="xs" c="dimmed">{formatTime(currentTime)}</Text>
            <Text size="xs" c="dimmed">{formatTime(duration)}</Text>
          </Group>
        </Stack>

        <Group gap="xs" wrap="nowrap">
          <ActionIcon
            size="sm"
            variant="subtle"
            onClick={toggleMute}
            aria-label={isMuted ? 'Unmute' : 'Mute'}
          >
            {isMuted ? <IconVolumeOff size={16} /> : <IconVolume size={16} />}
          </ActionIcon>
          <Slider
            value={isMuted ? 0 : volume}
            onChange={handleVolumeChange}
            max={1}
            min={0}
            step={0.01}
            size="xs"
            style={{ width: 60 }}
          />
        </Group>
      </Group>

      <Stack gap="xs">
        <Group justify="space-between" align="center">
          <Text size="sm" fw={500}>Playback Speed</Text>
          <Group gap="xs">
            <ActionIcon
              size="sm"
              variant="light"
              onClick={decreaseSpeed}
              disabled={playbackSpeed === SPEED_OPTIONS[0]}
              aria-label="Decrease speed"
            >
              <IconChevronDown size={16} />
            </ActionIcon>
            <Badge variant="light" size="lg">
              {playbackSpeed}x
            </Badge>
            <ActionIcon
              size="sm"
              variant="light"
              onClick={increaseSpeed}
              disabled={playbackSpeed === SPEED_OPTIONS[SPEED_OPTIONS.length - 1]}
              aria-label="Increase speed"
            >
              <IconChevronUp size={16} />
            </ActionIcon>
          </Group>
        </Group>

        <Group gap="xs" justify="center">
          {SPEED_OPTIONS.map((speed) => (
            <Button
              key={speed}
              size="xs"
              variant={playbackSpeed === speed ? 'filled' : 'light'}
              onClick={() => handleSpeedChange(speed)}
            >
              {speed}x
            </Button>
          ))}
        </Group>
      </Stack>
    </Stack>
  );
}